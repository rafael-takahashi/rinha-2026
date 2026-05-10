use axum::{Router, http::StatusCode, routing::get};

mod config;
mod dataset;
mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dataset::Dataset::load()?;

    let app = Router::new().route("/ready", get(|| async { StatusCode::OK }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
