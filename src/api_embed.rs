use serde::{Deserialize, Serialize};
use tako::{
    extractors::{FromRequest, simdjson::SimdJson},
    responder::Responder,
    state::get_state,
    types::Request,
};

use crate::AppState;

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

    let app_state = get_state::<AppState>().unwrap();
    let embs = app_state.embedder.lock().await.embed(body.inputs, Some(32));

    match embs {
        Ok(outputs) => SimdJson(ResponseBody { outputs: outputs }).into_response(),
        Err(_) => SimdJson(ResponseBody { outputs: vec![] }).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
    use http::Method;
    use tako::router::Router;
    use tokio::{
        net::{TcpListener, TcpStream},
        sync::Mutex,
    };

    use crate::{AppState, MAX_CHANNEL_SIZE};

    const REQUEST: usize = 100;

    #[tokio::test]
    async fn test_handler() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();

        let embedder = Arc::new(Mutex::new(TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
        )?));

        let (tx, ..) = tokio::sync::mpsc::channel(MAX_CHANNEL_SIZE);
        let state = AppState { tx, embedder };
        let listener = TcpListener::bind("0.0.0.0:8082").await?;
        let mut router = Router::new();
        router.state(state);
        router.route_with_tsr(Method::POST, "/embed", super::handler);
        tokio::spawn(async move { tako::serve(listener, router).await });

        let client = reqwest::Client::new();
        let response = client
            .post("http://localhost:8082/embed")
            .json(&super::RequestBody {
                inputs: vec![
                    String::from("What is Vector Search?"),
                    String::from("Hello, world!"),
                ],
            })
            .send()
            .await?;

        assert_eq!(response.status(), 200);
        Ok(())
    }

    #[tokio::test]
    async fn test_benchmark() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();

        let embedder = Arc::new(Mutex::new(TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
        )?));

        let (tx, ..) = tokio::sync::mpsc::channel(MAX_CHANNEL_SIZE);
        let state = AppState { tx, embedder };
        let listener = TcpListener::bind("0.0.0.0:8084").await?;
        let mut router = Router::new();
        router.state(state);
        router.route_with_tsr(Method::POST, "/embed", super::handler);
        tokio::spawn(async move { tako::serve(listener, router).await });
        for _ in 0..50 {
            if TcpStream::connect("127.0.0.1:8084").await.is_ok() {
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
                .post("http://localhost:8084/embed")
                .json(&super::RequestBody {
                    inputs: vec![
                        String::from("What is Vector Search?"),
                        String::from("Hello, world!"),
                    ],
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
        println!("Elapsed time: {:?}", elapsed);

        assert_eq!(count, REQUEST);
        Ok(())
    }
}
