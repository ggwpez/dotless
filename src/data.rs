use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EraPaid {
    pub timestamp: String,
    pub amount_paid: String,
    pub total_issuance: String,
    /// None for frozen relay-chain history, Some for ingested Asset Hub events.
    /// Used as the ingestor cursor and to select the live subset for persistence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    /// Total balance (plancks, free + reserved) of the DAP buffer account at the
    /// boundary block — the post-2.3.0 treasury-equivalent share sink
    /// (`13UVJyLkAxdQn6zM3GrLGvrFgN9jWneRX2NGvf3u92iunZq1`). Checked once per era
    /// (≈daily). None for frozen history / events ingested before buffer monitoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_balance: Option<String>,
    /// Total balance (plancks, free + reserved) held by the configured burn
    /// address at this era boundary. Retained on every daily record so it can
    /// be graphed later as well as used for the current burn counter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burn_balance: Option<String>,
}

impl EraPaid {
    /// Amount paid in DOT (divided by 1e10)
    pub fn amount_paid_dot(&self) -> f64 {
        self.amount_paid
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1e10
    }

    /// Total issuance in DOT (divided by 1e10)
    pub fn total_issuance_dot(&self) -> f64 {
        self.total_issuance
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1e10
    }

    /// DAP buffer account total balance in DOT (divided by 1e10), if recorded.
    pub fn buffer_balance_dot(&self) -> Option<f64> {
        self.buffer_balance
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v / 1e10)
    }

    /// Configured burn-address total balance in DOT (divided by 1e10), if recorded.
    pub fn burn_balance_dot(&self) -> Option<f64> {
        self.burn_balance
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v / 1e10)
    }
}

pub fn load_events_from_json(path: &str) -> Vec<EraPaid> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::error!("Failed to parse {path}: {e}");
            Vec::new()
        }),
        Err(_) => {
            tracing::warn!("{path} not found, starting with empty history");
            Vec::new()
        }
    }
}

/// Persist the ingested Asset Hub events (those carrying a `block_number`).
pub fn save_live_events(path: &str, events: &[EraPaid]) -> anyhow::Result<()> {
    let live: Vec<&EraPaid> = events.iter().filter(|e| e.block_number.is_some()).collect();
    let json = serde_json::to_string_pretty(&live)?;
    std::fs::write(path, json)?;
    Ok(())
}
