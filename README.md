# Qdrant-Batch-Proxy

A high-performance batching proxy for embedding generation, built with Rust.

**Stack:**

* **[Tokio](https://tokio.rs/)** – async runtime
* **[tako-rs](https://github.com/rust-dd/tako)** – custom high-performance web framework (in production should be used Axum or Actix)
* **[FastEmbed](https://github.com/Anush008/fastembed-rs)** – Rust wrapper for ONNX Runtime, used for embedding generation ([chosen due to limitations on Mac M1 hardware](https://github.com/huggingface/text-embeddings-inference?tab=readme-ov-file#apple-m1m2-arm64-architectures))


## 🚀 Usage

Clone the repository:

```bash
git clone https://github.com/youruser/qdrant-batch-proxy.git
cd qdrant-batch-proxy
```

### Run with Cargo

```bash
cargo run --release
```

### Run with Docker

```bash
docker build . -t qdrant-batch-proxy:latest
docker run -p 8080:8080 qdrant-batch-proxy:latest
```

### Run with Docker Compose

```bash
docker compose up
```


## 🧪 Testing

### Unit & Integration Tests

```bash
cargo test
```

### Manual Test with `curl`

```bash
curl 127.0.0.1:8080/embed \
  -X POST \
  -d '{"inputs":["What is Vector Search?", "Hello, world!"]}' \
  -H 'Content-Type: application/json'
```

### Load Testing with `wrk`

First, create `embed.lua`:

```lua
wrk.method = "POST"
wrk.body   = '{"inputs":["What is Vector Search?", "Hello, world!"]}'
wrk.headers["Content-Type"] = "application/json"
```

Then run:

```bash
wrk -t4 -c64 -d30s -s embed.lua http://127.0.0.1:8080/embed
```

## 📊 Benchmark

### Commands

```bash
# /embed_batch endpoint
time seq 1000 | xargs -n1 -P1000 curl -s \
  -X POST http://127.0.0.1:8080/embed_batch \
  -H 'Content-Type: application/json' \
  -d '{"inputs":["What is Vector Search?", "Hello, world!"]}' > /dev/null

# /embed endpoint
time seq 1000 | xargs -n1 -P1000 curl -s \
  -X POST http://127.0.0.1:8080/embed \
  -H 'Content-Type: application/json' \
  -d '{"inputs":["What is Vector Search?", "Hello, world!"]}' > /dev/null
```


### Results

| Endpoint       | User Time | System Time | CPU Usage | Total Time (s) |
| -------------- | --------- | ----------- | --------- | -------------- |
| `/embed_batch` | 3.64s     | 7.50s       | 86%       | **12.944**     |
| `/embed`       | 4.05s     | 8.31s       | 42%       | **28.869**     |
