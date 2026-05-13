use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};
use tokio::net::UnixListener;

mod config;
mod dataset;
mod handlers;
mod json;
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

    let socket_path = std::env::var("SOCKET_PATH").expect("SOCKET_PATH must be set");
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
    std::fs::set_permissions(&socket_path, PermissionsExt::from_mode(0o777))?;
    axum::serve(listener, app).await?;
    Ok(())
}
