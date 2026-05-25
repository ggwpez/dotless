//! In-process subxt ingestor for `Staking::EraPaid` events on Asset Hub Polkadot.
//!
//! Instead of scanning every block (block time is variable: ~6.3s in Nov 2025,
//! ~2s by May 2026), we binary-search era boundaries. `Staking::ActiveEra.index`
//! is monotonically non-decreasing in block height and increments at exactly the
//! block that emits `EraPaid` for the era that just ended, so we locate each
//! boundary block with a binary search over `ActiveEra.index` and read the event
//! plus `Timestamp::Now` / `Balances::TotalIssuance` there.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use subxt::rpcs::RpcClient;
use subxt::{OnlineClient, PolkadotConfig};

use crate::data;

/// Static codegen against the committed Asset Hub metadata. Accessors are by
/// pallet name (`staking`, `balances`, `timestamp`); the internal crate path
/// (`pallet_staking_async`) is irrelevant here.
#[subxt::subxt(runtime_metadata_path = "metadata/asset-hub-polkadot.scale")]
pub mod ah {}

const DEFAULT_RPC: &str = "wss://asset-hub-polkadot.ibp.network";
const DEFAULT_INGEST_START: &str = "2026-01-01";
const DEFAULT_REFRESH_SECS: u64 = 3600 * 12; // every 10 hours
const DEFAULT_LIVE_PATH: &str = "live_events.json";
const DEFAULT_RPC_INTERVAL_MS: u64 = 50;

/// A decoded `EraPaid` boundary, before conversion to the downstream string type.
pub struct EraPaidRaw {
    pub block_number: u64,
    pub era_index: u32,
    pub timestamp: DateTime<Utc>,
    pub amount_paid: u128,
    pub total_issuance: u128,
}

impl EraPaidRaw {
    pub fn into_event(self) -> data::EraPaid {
        data::EraPaid {
            timestamp: self.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            amount_paid: self.amount_paid.to_string(),
            total_issuance: self.total_issuance.to_string(),
            block_number: Some(self.block_number),
        }
    }
}

pub struct Ingestor {
    api: OnlineClient<PolkadotConfig>,
    /// Minimum spacing between RPC round-trips (0 = unthrottled).
    min_interval: Duration,
    /// Instant of the last gated request; `None` until the first call.
    last_request: tokio::sync::Mutex<Option<tokio::time::Instant>>,
}

impl Ingestor {
    pub async fn connect(rpc_url: &str) -> Result<Self> {
        let rpc_client = RpcClient::from_url(rpc_url)
            .await
            .with_context(|| format!("connecting to {rpc_url}"))?;
        let api = OnlineClient::<PolkadotConfig>::from_rpc_client(rpc_client).await?;

        let interval_ms = std::env::var("INGEST_RPC_INTERVAL_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RPC_INTERVAL_MS);

        Ok(Self {
            api,
            min_interval: Duration::from_millis(interval_ms),
            last_request: tokio::sync::Mutex::new(None),
        })
    }

    /// Block until at least `min_interval` has elapsed since the previous gated
    /// request. Holding the lock across the sleep serializes RPC round-trips,
    /// which is fine since the binary searches are sequential anyway.
    async fn gate(&self) {
        if self.min_interval.is_zero() {
            return;
        }
        let mut last = self.last_request.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        *last = Some(tokio::time::Instant::now());
    }

    pub async fn finalized_head_number(&self) -> Result<u64> {
        self.gate().await;
        Ok(self.api.at_current_block().await?.block_number())
    }

    async fn timestamp_ms_at(&self, block: u64) -> Result<u64> {
        self.gate().await;
        let at = self.api.at_block(block).await?;
        self.gate().await;
        let now = at
            .storage()
            .fetch(ah::storage().timestamp().now(), ())
            .await?
            .decode()?;
        Ok(now)
    }

    async fn active_era_index_at(&self, block: u64) -> Result<u32> {
        self.gate().await;
        let at = self.api.at_block(block).await?;
        self.gate().await;
        let ae = at
            .storage()
            .try_fetch(ah::storage().staking().active_era(), ())
            .await?;
        Ok(match ae {
            Some(v) => v.decode()?.index,
            None => 0,
        })
    }

    /// First finalized block whose `Timestamp::Now >= ts` (binary search over `[1, hi]`).
    pub async fn block_at_or_after(&self, ts: DateTime<Utc>, hi: u64) -> Result<u64> {
        let target_ms = ts.timestamp_millis().max(0) as u64;
        let mut lo = 1u64;
        let mut hi = hi;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.timestamp_ms_at(mid).await? >= target_ms {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        Ok(lo)
    }

    /// First block in `(lo, hi]` with `ActiveEra.index >= target`.
    ///
    /// Era index is (near-)linear in block height across many eras but a step
    /// function within one era, so we interpolate while the window still spans
    /// multiple eras and bisect once it doesn't. The interpolation gets us to
    /// within ~one era of the answer in a probe or two; bisection then pins the
    /// exact boundary in ~log2(era_length) probes (~16 for typical Polkadot
    /// eras of ~40k blocks). Without the bisection fallback, `e_hi - e_lo == 1`
    /// degenerates to `frac = 1.0` and the window shrinks one block per probe.
    ///
    /// `e_lo`/`e_hi` are the already-known era indices at `lo`/`hi`, so the
    /// endpoints are never re-read. Requires `e_lo < target <= e_hi`. The
    /// returned block has era exactly `target` (era increments by 1 per boundary).
    async fn boundary_block_for_era(
        &self,
        target: u32,
        mut lo: u64,
        mut e_lo: u32,
        mut hi: u64,
        mut e_hi: u32,
    ) -> Result<u64> {
        // Invariant: e_lo < target <= e_hi, so the boundary lies in (lo, hi].
        while hi - lo > 1 {
            let mid = if e_hi - e_lo >= 2 {
                let frac = (target - e_lo) as f64 / (e_hi - e_lo) as f64;
                (lo + ((hi - lo) as f64 * frac) as u64).clamp(lo + 1, hi - 1)
            } else {
                lo + (hi - lo) / 2
            };
            let e_mid = self.active_era_index_at(mid).await?;
            if e_mid >= target {
                hi = mid;
                e_hi = e_mid;
            } else {
                lo = mid;
                e_lo = e_mid;
            }
        }
        Ok(hi)
    }

    async fn era_paid_at_block(&self, block: u64) -> Result<EraPaidRaw> {
        self.gate().await;
        let at = self.api.at_block(block).await?;

        self.gate().await;
        let ep = at
            .events()
            .fetch()
            .await?
            .find_first::<ah::staking::events::EraPaid>()
            .ok_or_else(|| anyhow!("no EraPaid event at block #{block}"))??;

        self.gate().await;
        let ts_ms: u64 = at
            .storage()
            .fetch(ah::storage().timestamp().now(), ())
            .await?
            .decode()?;

        self.gate().await;
        let total_issuance: u128 = at
            .storage()
            .fetch(ah::storage().balances().total_issuance(), ())
            .await?
            .decode()?;

        let timestamp = DateTime::from_timestamp_millis(ts_ms as i64)
            .ok_or_else(|| anyhow!("invalid timestamp {ts_ms} at block #{block}"))?;

        Ok(EraPaidRaw {
            block_number: block,
            era_index: ep.era_index,
            timestamp,
            amount_paid: ep.validator_payout.saturating_add(ep.remainder),
            total_issuance,
        })
    }

    /// The active era index at `after_block` and `to_block`. Each era in
    /// `(lo, hi]` corresponds to one `EraPaid` boundary in `(after_block, to_block]`.
    async fn era_span(&self, after_block: u64, to_block: u64) -> Result<(u32, u32)> {
        let lo = self.active_era_index_at(after_block).await?;
        let hi = self.active_era_index_at(to_block).await?;
        Ok((lo, hi))
    }
}

fn ingest_start() -> Result<DateTime<Utc>> {
    let s = std::env::var("INGEST_START").unwrap_or_else(|_| DEFAULT_INGEST_START.into());
    let date = NaiveDate::parse_from_str(&s, "%Y-%m-%d")
        .with_context(|| format!("parsing INGEST_START={s:?} (expected YYYY-MM-DD)"))?;
    Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
}

fn live_path() -> String {
    std::env::var("LIVE_EVENTS_PATH").unwrap_or_else(|_| DEFAULT_LIVE_PATH.into())
}

/// One sync cycle: connect, find the cursor, ingest the delta, update caches + persist.
async fn run_cycle(state: &Arc<crate::AppState>) -> Result<()> {
    let rpc_url = std::env::var("ASSET_HUB_RPC").unwrap_or_else(|_| DEFAULT_RPC.into());
    let ingestor = Ingestor::connect(&rpc_url).await?;

    let head = ingestor.finalized_head_number().await?;

    // Cursor = highest ingested boundary block (None on cold start).
    let cursor = state
        .cached_events
        .read()
        .await
        .iter()
        .filter_map(|e| e.block_number)
        .max();

    let after_block = match cursor {
        Some(c) => c,
        None => {
            let start = ingest_start()?;
            tracing::info!("ingest: cold start, resolving {start:?} over [1, #{head}]...");
            let first = ingestor.block_at_or_after(start, head).await?;
            tracing::info!("ingest: cold start, INGEST_START maps to block #{first}");
            first.saturating_sub(1)
        }
    };

    if after_block >= head {
        tracing::info!("ingest: up to date (cursor #{after_block} >= head #{head})");
        return Ok(());
    }

    // Each new era activated in (after_block, head] is one EraPaid boundary block.
    let (lo, hi) = ingestor.era_span(after_block, head).await?;
    let total = hi.saturating_sub(lo);
    if total == 0 {
        tracing::info!("ingest: no new eras");
        return Ok(());
    }

    let path = live_path();
    tracing::info!("ingest: syncing {total} era(s) in (#{after_block}, #{head}] -> {path}");

    // Interpolation-search each boundary, then append + persist incrementally so
    // the file grows as we go and a mid-sync disconnect leaves the work so far
    // recoverable. We carry the lower endpoint across eras: the boundary found
    // for era `v` has era exactly `v`, so it becomes the lower bound for `v + 1`
    // (and `head`/`hi` stay the constant upper bound), avoiding endpoint re-reads.
    let mut lo_block = after_block;
    let mut lo_era = lo;
    let mut done = 0u32;
    for v in (lo + 1)..=hi {
        let tv = ingestor
            .boundary_block_for_era(v, lo_block, lo_era, head, hi)
            .await?;
        lo_block = tv;
        lo_era = v;
        let raw = ingestor.era_paid_at_block(tv).await?;
        done += 1;
        let era_index = raw.era_index;

        let mut events = state.cached_events.write().await;
        events.push(raw.into_event());
        state.chart_cache.write().await.append(&events);
        if let Err(e) = data::save_live_events(&path, &events) {
            tracing::error!("ingest: failed to persist live events: {e:#}");
        }
        let persisted = events.len();
        drop(events);

        tracing::info!(
            "ingest: era {era_index} @ #{tv} ({done}/{total}) -> {path} ({persisted} events)"
        );
    }

    tracing::info!("ingest: sync complete, {done} new era(s) (head #{head})");
    Ok(())
}

/// Spawn the background ingestor: cold sync from the cursor (or `INGEST_START`),
/// then re-sync every `INGEST_REFRESH_SECS` (default 600). Reconnects each cycle.
pub fn spawn(state: Arc<crate::AppState>) {
    let refresh = std::env::var("INGEST_REFRESH_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_REFRESH_SECS);

    tokio::spawn(async move {
        loop {
            if let Err(e) = run_cycle(&state).await {
                tracing::error!("ingest: cycle failed (retry in {refresh}s): {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(refresh)).await;
        }
    });
}
