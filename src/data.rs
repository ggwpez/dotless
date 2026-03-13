use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EraPaid {
    pub timestamp: String,
    pub amount_paid: String,
    pub total_issuance: String,
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

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: GraphQLData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLData {
    era_paids: Vec<EraPaid>,
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

const DEFAULT_GRAPHQL_URL: &str =
    "https://dotburned.squids.live/polkadot-issuance-sqd-v2@v1/api/graphql";

pub async fn fetch_era_paid_events(
    client: &reqwest::Client,
    after_timestamp: Option<&str>,
) -> anyhow::Result<Vec<EraPaid>> {
    let url = std::env::var("SUBSQUID_GRAPHQL_URL").unwrap_or_else(|_| DEFAULT_GRAPHQL_URL.into());

    let where_clause = match after_timestamp {
        Some(ts) => format!(r#", where: {{ timestamp_gt: "{ts}" }}"#),
        None => String::new(),
    };

    let query = serde_json::json!({
        "query": format!(r#"
            query {{
                eraPaids(orderBy: timestamp_ASC{where_clause}) {{
                    id
                    timestamp
                    amountPaid
                    totalIssuance
                }}
            }}
        "#)
    });

    let resp: GraphQLResponse = client.post(&url).json(&query).send().await?.json().await?;
    Ok(resp.data.era_paids)
}
