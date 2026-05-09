use axum::{Router, http::StatusCode, routing::get};
mod models;
mod config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let app = Router::new()
        .route("/ready", get(|| async { StatusCode::OK }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
