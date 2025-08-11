use serde::{Deserialize, Serialize};
use tako::{
    extractors::{FromRequest, simdjson::SimdJson},
    responder::Responder,
    state::get_state,
    types::Request,
};

use crate::{AppState, batch_engine::BatchItem};

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestBody {
    pub inputs: Vec<String>,
}

#[derive(Serialize)]
pub struct ResponseBody {
    pub outputs: Vec<Vec<f32>>,
}

pub async fn handler(mut req: Request) -> impl Responder {
    let SimdJson(body) = SimdJson::<RequestBody>::from_request(&mut req)
        .await
        .unwrap();
    tracing::info!("Embed request body: {:?}", body);

    let (tx, rx) = tokio::sync::oneshot::channel();
    let app_state = get_state::<AppState>().unwrap();
    std::mem::drop(
        app_state
            .tx
            .send(BatchItem {
                input: body.inputs,
                tx: tx,
            })
            .await,
    );

    match rx.await {
        Ok(outputs) => SimdJson(ResponseBody {
            outputs: outputs.unwrap(),
        })
        .into_response(),
        Err(_) => SimdJson(ResponseBody { outputs: vec![] }).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use fake::Fake;
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
    use http::Method;
    use tako::router::Router;
    use tokio::{
        net::{TcpListener, TcpStream},
        sync::Mutex,
    };

    use crate::{AppState, MAX_CHANNEL_SIZE, batch_engine::BatchEngine};

    const REQUEST: usize = 100;

    #[tokio::test]
    async fn test_handler() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();

        let embedder = Arc::new(Mutex::new(TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
        )?));

        let batcher = BatchEngine::new()?;
        let (tx, rx) = tokio::sync::mpsc::channel(MAX_CHANNEL_SIZE);
        tokio::spawn({
            let embedder = embedder.clone();
            async move { batcher.run(rx, embedder).await }
        });

        let state = AppState { tx, embedder };
        let listener = TcpListener::bind("0.0.0.0:8083").await?;
        let mut router = Router::new();
        router.state(state);
        router.route_with_tsr(Method::POST, "/embed_batch", super::handler);
        tokio::spawn(async move { tako::serve(listener, router).await });

        let client = reqwest::Client::new();
        let mut count = 0;

        for _ in 0..REQUEST {
            let random = rand::random_range(1..10);
            let mut inputs = Vec::with_capacity(random);
            for _ in 0..random {
                let words = fake::faker::lorem::en::Words(3..8).fake::<Vec<String>>();
                inputs.push(words.join(" "));
            }

            let response = client
                .post("http://localhost:8083/embed_batch")
                .json(&super::RequestBody { inputs })
                .send()
                .await?;

            assert_eq!(response.status(), 200);
            count += 1;
        }

        assert_eq!(count, REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn test_benchmark() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();

        let embedder = Arc::new(Mutex::new(TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
        )?));

        let batcher = BatchEngine::new()?;
        let (tx, rx) = tokio::sync::mpsc::channel(MAX_CHANNEL_SIZE);
        tokio::spawn({
            let embedder = embedder.clone();
            async move { batcher.run(rx, embedder).await }
        });

        let state = AppState { tx, embedder };
        let listener = TcpListener::bind("0.0.0.0:8085").await?;
        let mut router = Router::new();
        router.state(state);
        router.route_with_tsr(Method::POST, "/embed_batch", super::handler);
        tokio::spawn(async move { tako::serve(listener, router).await });
        for _ in 0..50 {
            if TcpStream::connect("127.0.0.1:8085").await.is_ok() {
                println!("Server started successfully");
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let client = reqwest::Client::new();
        let mut count = 0;

        let now = Instant::now();
        let futures = (0..REQUEST).map(|_| {
            client
                .post("http://localhost:8085/embed_batch")
                .json(&super::RequestBody {
                    inputs: vec!["What is Vector Search?".into(), "Hello, world!".into()],
                })
                .send()
        });

        let responses = futures::future::join_all(futures).await;
        for response in responses {
            let response = response?;
            assert_eq!(response.status(), 200);
            count += 1;
        }
        let elapsed = now.elapsed();
        println!("Request took: {:?}", elapsed);

        assert_eq!(count, REQUEST);
        Ok(())
    }
}
