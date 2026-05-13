FROM lukemathwalker/cargo-chef:latest-rust-slim AS base

FROM base AS planner
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef prepare --recipe-path recipe.json

FROM base AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --bin fraud-detection && \
    cargo run --release --bin preprocess

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/fraud-detection ./
COPY --from=builder /app/data ./data
EXPOSE 9999
CMD ["./fraud-detection"]
