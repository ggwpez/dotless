use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use serde::Serialize;
use std::collections::HashMap;

use crate::data::EraPaid;

const DAYS_PER_YEAR: f64 = 365.25;

/// Pre-2026-03-14: 120M DOT/year
const OLD_DAILY_ISSUANCE: f64 = 120_000_000.0 / DAYS_PER_YEAR;

/// Hard cap target: 21B DOT
const TARGET_TOTAL_ISSUANCE: f64 = 2_100_000_000.0;

/// Approximate total issuance at March 14, 2026 (to be updated with real value)
const MARCH_2026_TI: f64 = 1_674_342_153.476060356;

/// 26.28% closer to target every 2-year step (Perbill::from_parts(262_800_000))
const BI_ANNUAL_RATE: f64 = 0.2628;

/// Step period: 2 years in days
const STEP_PERIOD_DAYS: f64 = 2.0 * DAYS_PER_YEAR;

/// The date the new issuance model takes effect
fn new_model_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 14).unwrap()
}

/// Display cutoff — only show data from this date onwards
fn display_cutoff() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
}

/// Evaluate the stepped curve total issuance at a given number of periods.
///
/// Formula: target - (target - initial) * (1 - rate)^num_periods
///
/// Ported from polkadot-sdk `SteppedCurve::evaluate` with `RemainingPct`.
fn stepped_curve_ti(num_periods: u32) -> f64 {
    let diff = TARGET_TOTAL_ISSUANCE - MARCH_2026_TI;
    let scale = (1.0 - BI_ANNUAL_RATE).powi(num_periods as i32);
    TARGET_TOTAL_ISSUANCE - diff * scale
}

fn daily_issuance_for(date: NaiveDateTime) -> f64 {
    if date.date() >= new_model_date() {
        let curve_start = new_model_date()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            - TimeDelta::days(STEP_PERIOD_DAYS as i64);
        let days_since_start = (date - curve_start).num_days().max(0) as f64;
        let num_periods = (days_since_start / STEP_PERIOD_DAYS).floor() as u32;

        let ti_now = stepped_curve_ti(num_periods);
        let ti_prev = if num_periods > 0 {
            stepped_curve_ti(num_periods - 1)
        } else {
            MARCH_2026_TI
        };
        let step_emission = ti_now - ti_prev;
        step_emission / STEP_PERIOD_DAYS
    } else {
        OLD_DAILY_ISSUANCE
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartPoint {
    pub timestamp: String,
    pub inflation_rate: f64,
    pub supply: f64,
    pub daily_issuance: f64,
    pub is_projected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartData {
    pub points: Vec<ChartPoint>,
    pub y_min_inflation: f64,
    pub y_max_inflation: f64,
    pub y_min_supply: f64,
    pub y_max_supply: f64,
}

pub const SUPPORTED_YEARS: &[u32] = &[5, 10, 50];

/// Precomputed chart cache: historical points + per-years ChartData.
pub struct ChartCache {
    historical: Vec<ChartPoint>,
    /// Number of raw events already processed (so we can append incrementally).
    events_processed: usize,
    charts: HashMap<u32, ChartData>,
}

impl ChartCache {
    /// Build from scratch from all events.
    pub fn new(events: &[EraPaid]) -> Self {
        let historical = Self::historical_from_events(events);
        let mut cache = ChartCache {
            historical,
            events_processed: events.len(),
            charts: HashMap::new(),
        };
        for &years in SUPPORTED_YEARS {
            cache.charts.insert(years, build_chart_data(&cache.historical, years as f64));
        }
        cache
    }

    /// Append new events and recompute projections.
    ///
    /// `historical_from_events` derives each post-migration point's daily issuance
    /// from the previous event's total issuance, so it must see the full ordered
    /// list (not just the new tail). The series is ~one event/day, so rebuilding
    /// it wholesale here is cheap and keeps the delta correct across the boundary.
    pub fn append(&mut self, all_events: &[EraPaid]) {
        if all_events.len() <= self.events_processed {
            return;
        }
        self.events_processed = all_events.len();
        self.historical = Self::historical_from_events(all_events);
        for &years in SUPPORTED_YEARS {
            self.charts.insert(years, build_chart_data(&self.historical, years as f64));
        }
    }

    pub fn get(&self, years: u32) -> Option<&ChartData> {
        self.charts.get(&years)
    }

    fn historical_from_events(events: &[EraPaid]) -> Vec<ChartPoint> {
        let cutoff = display_cutoff();
        let mut points = Vec::new();
        for (i, event) in events.iter().enumerate() {
            let ts = parse_timestamp(&event.timestamp);
            if ts.date() < cutoff {
                continue;
            }
            let issuance = event.total_issuance_dot();

            // Daily new issuance = staker share + treasury-equivalent share.
            //
            // `amount_paid` (`validator_payout + remainder`) always carries the
            // staker share, and pre-2.3.0 it carried the treasury share too (via
            // `remainder`). From the 2.3.0 DAP migration `remainder` is 0 and the
            // treasury share instead drips into the DAP buffer account, so we add
            // the per-era growth of the buffer balance back in.
            //
            // This is both correct and smooth: deriving issuance from the per-era
            // delta of *total* issuance instead is noisy (it also moves with fee
            // burns and other flows), whereas `validator_payout` and the buffer are
            // clean snapshots. When the split visibly switched on (validator_payout
            // shrank to just the staker pot), the buffer began absorbing the rest by
            // an equal amount, so the sum stays flat across the boundary.
            // Pre-buffer-tracking events have `buffer_balance == None`, making the
            // delta 0 and leaving `amount_paid` unchanged. See
            // issuance-monitoring-migration.md.
            let buffer_delta = match (
                event.buffer_balance_dot(),
                i.checked_sub(1).and_then(|j| events[j].buffer_balance_dot()),
            ) {
                // `max(0)` guards against the buffer being drained/spent (a treasury
                // outflow is not negative inflation).
                (Some(cur), Some(prev)) => (cur - prev).max(0.0),
                _ => 0.0,
            };
            let daily_increase = event.amount_paid_dot() + buffer_delta;

            let inflation_rate = (daily_increase / issuance) * DAYS_PER_YEAR * 100.0;
            points.push(ChartPoint {
                timestamp: event.timestamp.clone(),
                inflation_rate,
                supply: issuance,
                daily_issuance: daily_increase,
                is_projected: false,
            });
        }
        points
    }
}

/// Build full ChartData from historical points + projections for a given timeframe.
fn build_chart_data(historical: &[ChartPoint], projection_years: f64) -> ChartData {
    let mut points = historical.to_vec();

    if let Some(last) = historical.last() {
        let projection_days = (projection_years * DAYS_PER_YEAR) as i64;
        let base_ts = parse_timestamp(&last.timestamp);
        let mut supply = last.supply;

        // Adaptive downsampling: longer projections get sparser points
        let sample_interval = if projection_years > 50.0 {
            365
        } else if projection_years > 10.0 {
            90
        } else {
            30
        };

        for day in 1..=projection_days {
            let ts = base_ts + TimeDelta::days(day);
            let daily = daily_issuance_for(ts);
            supply += daily;

            if day % sample_interval == 0 || day == projection_days {
                let inflation_rate = (daily * DAYS_PER_YEAR / supply) * 100.0;
                points.push(ChartPoint {
                    timestamp: ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    inflation_rate,
                    supply,
                    daily_issuance: daily,
                    is_projected: true,
                });
            }
        }
    }

    let (mut min_inf, mut max_inf) = (f64::MAX, f64::MIN);
    let (mut min_sup, mut max_sup) = (f64::MAX, f64::MIN);
    for p in &points {
        min_inf = min_inf.min(p.inflation_rate);
        max_inf = max_inf.max(p.inflation_rate);
        min_sup = min_sup.min(p.supply);
        max_sup = max_sup.max(p.supply);
    }

    ChartData {
        points,
        y_min_inflation: min_inf,
        y_max_inflation: max_inf,
        y_min_supply: min_sup,
        y_max_supply: max_sup,
    }
}

fn parse_timestamp(ts: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.fZ")
        .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f"))
        .unwrap_or_default()
}
