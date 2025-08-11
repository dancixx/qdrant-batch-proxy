FROM rust:latest AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang llvm-dev libclang-dev pkg-config cmake make git \
    build-essential ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/qdrant-batch-proxy

COPY .. .
RUN cargo build --release

FROM ubuntu:22.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libstdc++6 libgomp1 \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

COPY --from=build /usr/src/qdrant-batch-proxy/target/release/qdrant-batch-proxy /usr/local/bin/qdrant-batch-proxy
COPY --from=build /usr/src/qdrant-batch-proxy/.env /usr/local/bin/.env

CMD ["qdrant-batch-proxy"]
