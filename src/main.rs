use std::sync::Arc;

use anyhow::Result;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tako::{Method, router::Router};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::batch_engine::{BatchEngine, BatchItem};

mod api_embed;
mod api_embed_batch;
mod api_healhtz;
mod batch_engine;

const MAX_CHANNEL_SIZE: usize = 10_000;

#[derive(Clone)]
struct AppState {
    pub tx: tokio::sync::mpsc::Sender<BatchItem>,
    pub embedder: Arc<Mutex<TextEmbedding>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let embedder = Arc::new(Mutex::new(TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
    )?));

    let batcher = BatchEngine::new()?;
    let (tx, rx) = tokio::sync::mpsc::channel(MAX_CHANNEL_SIZE);
    tokio::spawn({
        let embedder = embedder.clone();
        async move { batcher.run(rx, embedder.clone()).await }
    });

    let state = AppState { tx, embedder };
    let mut router = Router::new();
    router.state(state);
    router.route_with_tsr(Method::POST, "/embed", api_embed::handler);
    router.route_with_tsr(Method::POST, "/embed_batch", api_embed_batch::handler);
    router.route_with_tsr(Method::GET, "/healthz", api_healhtz::handler);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tako::serve(listener, router).await;

    Ok(())
}
