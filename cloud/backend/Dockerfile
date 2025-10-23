FROM rust:1.84-bullseye as builder
WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/codex
COPY --from=builder /usr/src/app/target/release/codex-cloud-backend /usr/local/bin/codex-cloud-backend
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/codex-cloud-backend"]
CMD ["serve", "--addr", "0.0.0.0:8000"]
