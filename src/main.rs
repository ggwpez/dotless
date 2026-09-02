mod data;
mod inflation;
mod ingest;

use askama::Template;
use axum::{
    extract::State,
    response::Html,
    routing::get,
    Router,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

pub(crate) struct AppState {
    pub(crate) cached_events: RwLock<Vec<data::EraPaid>>,
    pub(crate) chart_cache: RwLock<inflation::ChartCache>,
}

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    all_chart_data_json: String,
    last_event_ts: String,
    /// Latest recorded DAP buffer balance in DOT (plain number), or empty.
    dap_buffer_dot: String,
    /// Latest recorded balance of the configured burn address in DOT.
    burn_balance_dot: String,
}

async fn index_handler(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let cache = state.chart_cache.read().await;
    let all: HashMap<u32, &inflation::ChartData> = inflation::SUPPORTED_YEARS
        .iter()
        .filter_map(|&y| cache.get(y).map(|d| (y, d)))
        .collect();
    let all_chart_data_json = serde_json::to_string(&all).unwrap_or_else(|_| "{}".into());

    let (last_event_ts, dap_buffer_dot, burn_balance_dot) = {
        let events = state.cached_events.read().await;
        let ts = events.last().map(|e| e.timestamp.clone()).unwrap_or_default();
        let buffer = events
            .iter()
            .rev()
            .find_map(|e| e.buffer_balance_dot())
            .map(|v| format!("{v}"))
            .unwrap_or_default();
        let burn = events
            .iter()
            .rev()
            .find_map(|e| e.burn_balance_dot())
            .map(|v| format!("{v}"))
            .unwrap_or_default();
        (ts, buffer, burn)
    };

    let template = IndexTemplate {
        all_chart_data_json,
        last_event_ts,
        dap_buffer_dot,
        burn_balance_dot,
    };
    Html(template.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dotless=info".into()),
        )
        .init();

    // Previously ingested Asset Hub events; the background ingestor syncs the
    // rest from chain. Everything the chart shows is >= 2026-01-01 (the display
    // cutoff), so the pre-2026 relay-chain history is not loaded.
    let live_path = std::env::var("LIVE_EVENTS_PATH").unwrap_or_else(|_| "live_events.json".into());
    let events = data::load_events_from_json(&live_path);
    tracing::info!("Loaded {} live events from {live_path}", events.len());
    if let Some(last) = events.last() {
        tracing::info!("Latest EraPaid: {}", last.timestamp);
    }

    // Build chart cache once on startup; the ingestor appends to it as it syncs.
    let chart_cache = inflation::ChartCache::new(&events);
    tracing::info!("Chart cache built for {:?}", inflation::SUPPORTED_YEARS);

    let state = Arc::new(AppState {
        cached_events: RwLock::new(events),
        chart_cache: RwLock::new(chart_cache),
    });

    ingest::spawn(state.clone());

    let app = Router::new()
        .route("/", get(index_handler))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
