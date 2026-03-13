mod data;
mod inflation;
mod live;

use askama::Template;
use axum::{
    extract::{Query, State},
    response::{
        sse::{Event, Sse},
        Html,
    },
    routing::get,
    Router,
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;

struct AppState {
    cached_events: RwLock<Vec<data::EraPaid>>,
    block_tx: broadcast::Sender<live::BlockInfo>,
    latest_block: live::LatestBlock,
    http_client: reqwest::Client,
}

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    chart_data_json: String,
    years: u32,
}

#[derive(serde::Deserialize)]
struct IndexQuery {
    years: Option<u32>,
}

async fn index_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IndexQuery>,
) -> Html<String> {
    let years = query.years.unwrap_or(5);
    let events = state.cached_events.read().await;
    let chart_data = inflation::compute_chart_data(&events, years as f64);
    let chart_data_json = serde_json::to_string(&chart_data).unwrap_or_default();

    let template = IndexTemplate {
        chart_data_json,
        years,
    };
    Html(template.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

async fn sse_route(State(state): State<Arc<AppState>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    live::sse_handler(state.block_tx.clone(), state.latest_block.clone()).await
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

    // Fetch initial data
    tracing::info!("Fetching EraPaid events from Subsquid...");
    let events = data::fetch_era_paid_events(&http_client, None)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to fetch initial data: {e}");
            Vec::new()
        });
    tracing::info!("Loaded {} EraPaid events", events.len());
    if let Some(last) = events.last() {
        tracing::info!("Latest EraPaid: block {} at {}", last.block_number, last.timestamp);
    }

    // Dump to JSON for manual editing / reuse
    let json_path = "era_paid_events.json";
    match serde_json::to_string_pretty(&events) {
        Ok(json) => {
            std::fs::write(json_path, &json).unwrap_or_else(|e| {
                tracing::error!("Failed to write {json_path}: {e}");
            });
            tracing::info!("Wrote {json_path} ({} events)", events.len());
        }
        Err(e) => tracing::error!("Failed to serialize events: {e}"),
    }

    let (block_tx, _) = broadcast::channel::<live::BlockInfo>(64);
    let latest_block: live::LatestBlock = Arc::new(RwLock::new(None));

    let state = Arc::new(AppState {
        cached_events: RwLock::new(events),
        block_tx: block_tx.clone(),
        latest_block: latest_block.clone(),
        http_client,
    });

    // Spawn background tasks
    live::spawn_block_subscriber(block_tx, latest_block).await;
    spawn_data_refresher(state.clone());

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/events", get(sse_route))
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
            let last_block = state
                .cached_events
                .read()
                .await
                .last()
                .map(|e| e.block_number);
            tracing::info!("Fetching EraPaid events after block {last_block:?}...");
            match data::fetch_era_paid_events(&state.http_client, last_block).await {
                Ok(new_events) => {
                    let count = new_events.len();
                    if count > 0 {
                        state.cached_events.write().await.extend(new_events);
                        tracing::info!("Appended {count} new EraPaid events");
                    } else {
                        tracing::info!("No new EraPaid events");
                    }
                }
                Err(e) => tracing::error!("Failed to refresh data: {e}"),
            }
        }
    });
}
