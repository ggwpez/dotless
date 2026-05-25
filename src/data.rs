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
