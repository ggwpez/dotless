use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Serialize)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: String,
}

/// Shared latest block, updated by the subscriber.
pub type LatestBlock = Arc<RwLock<Option<BlockInfo>>>;

/// Spawn a background task that subscribes to finalized Polkadot blocks via subxt
/// and broadcasts them through the channel.
pub async fn spawn_block_subscriber(tx: broadcast::Sender<BlockInfo>, latest: LatestBlock) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = subscribe_blocks(tx.clone(), latest.clone()).await {
                tracing::error!("Block subscription error: {e}, reconnecting in 5s...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });
}

async fn subscribe_blocks(
    tx: broadcast::Sender<BlockInfo>,
    latest: LatestBlock,
) -> anyhow::Result<()> {
    let rpc_url =
        std::env::var("POLKADOT_RPC_URL").unwrap_or_else(|_| "wss://rpc.polkadot.io".into());

    let api = subxt::OnlineClient::<subxt::PolkadotConfig>::from_url(&rpc_url).await?;
    tracing::info!("Connected to Polkadot RPC at {rpc_url}");

    // Fetch the latest block immediately so clients don't wait
    let current = api.blocks().at_latest().await?;
    let info = BlockInfo {
        number: current.number().into(),
        hash: format!("{:?}", current.hash()),
    };
    tracing::info!("Latest block #{}", info.number);
    *latest.write().await = Some(info.clone());
    let _ = tx.send(info);

    let mut blocks = api.blocks().subscribe_best().await?;

    while let Some(block) = blocks.next().await {
        let block = block?;
        let info = BlockInfo {
            number: block.number().into(),
            hash: format!("{:?}", block.hash()),
        };
        tracing::debug!("Best block #{}", info.number);
        *latest.write().await = Some(info.clone());
        let _ = tx.send(info);
    }

    Ok(())
}

/// SSE handler that streams finalized blocks to the client.
/// Immediately sends the latest known block, then streams new ones.
pub async fn sse_handler(
    tx: broadcast::Sender<BlockInfo>,
    latest: LatestBlock,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Snapshot the latest block before subscribing to avoid missing one
    let initial = latest.read().await.clone();
    let rx = tx.subscribe();

    let initial_stream = futures::stream::iter(initial.map(|info| {
        Ok(Event::default()
            .event("block")
            .json_data(&info)
            .unwrap())
    }));

    let live_stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(info) => Some(Ok(Event::default()
            .event("block")
            .json_data(&info)
            .unwrap())),
        Err(_) => None,
    });

    Sse::new(initial_stream.chain(live_stream)).keep_alive(KeepAlive::default())
}
