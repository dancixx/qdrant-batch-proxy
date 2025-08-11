use tako::{responder::Responder, types::Request};

pub async fn handler(_: Request) -> impl Responder {
    tracing::info!("Health check request received");

    "OK"
}

#[cfg(test)]
mod tests {
    use http::Method;
    use tako::router::Router;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_handler() {
        let listener = TcpListener::bind("0.0.0.0:8081").await.unwrap();
        let mut router = Router::new();
        router.route_with_tsr(Method::GET, "/healthz", super::handler);
        tokio::spawn(async move { tako::serve(listener, router).await });

        let client = reqwest::Client::new();
        let response = client
            .get("http://localhost:8081/healthz")
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "OK");
    }
}
