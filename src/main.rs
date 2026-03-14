mod data;
mod inflation;

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

struct AppState {
    cached_events: RwLock<Vec<data::EraPaid>>,
    chart_cache: RwLock<inflation::ChartCache>,
    http_client: reqwest::Client,
}

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    all_chart_data_json: String,
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

    let template = IndexTemplate { all_chart_data_json };
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

    let http_client = reqwest::Client::new();

    // Load historical data from JSON, then fetch newer events from GraphQL
    let mut events = data::load_events_from_json("era_paid_events.json");
    tracing::info!("Loaded {} events from era_paid_events.json", events.len());

    let last_ts = events.last().map(|e| e.timestamp.as_str());
    tracing::info!("Fetching EraPaid events after {last_ts:?}...");
    match data::fetch_era_paid_events(&http_client, last_ts).await {
        Ok(new) => {
            tracing::info!("Fetched {} new events from GraphQL", new.len());
            events.extend(new);
        }
        Err(e) => tracing::error!("Failed to fetch from GraphQL: {e}"),
    }
    tracing::info!("Total: {} EraPaid events", events.len());
    if let Some(last) = events.last() {
        tracing::info!("Latest EraPaid: {}", last.timestamp);
    }

    // Build chart cache once on startup
    let chart_cache = inflation::ChartCache::new(&events);
    tracing::info!("Chart cache built for {:?}", inflation::SUPPORTED_YEARS);

    let state = Arc::new(AppState {
        cached_events: RwLock::new(events),
        chart_cache: RwLock::new(chart_cache),
        http_client,
    });

    spawn_data_refresher(state.clone());

    let app = Router::new()
        .route("/", get(index_handler))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Refresh cached EraPaid events every 10 minutes.
fn spawn_data_refresher(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
        interval.tick().await; // skip immediate first tick
        loop {
            interval.tick().await;
            let last_ts = state
                .cached_events
                .read()
                .await
                .last()
                .map(|e| e.timestamp.clone());
            tracing::info!("Fetching EraPaid events after {last_ts:?}...");
            match data::fetch_era_paid_events(&state.http_client, last_ts.as_deref()).await {
                Ok(new_events) => {
                    let count = new_events.len();
                    if count > 0 {
                        let mut events = state.cached_events.write().await;
                        events.extend(new_events);
                        state.chart_cache.write().await.append(&events);
                        tracing::info!("Appended {count} new EraPaid events, chart cache updated");
                    } else {
                        tracing::info!("No new EraPaid events");
                    }
                }
                Err(e) => tracing::error!("Failed to refresh data: {e}"),
            }
        }
    });
}
