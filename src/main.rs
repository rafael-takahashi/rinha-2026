use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};

mod config;
mod dataset;
mod handlers;
mod knn;
mod models;
mod vectorize;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = dataset::Dataset::load()?;
    let shared_state = Arc::new(dataset);

    let app = Router::new()
        .route("/ready", get(|| async { StatusCode::OK }))
        .route("/fraud-score", post(handlers::fraud_score))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
