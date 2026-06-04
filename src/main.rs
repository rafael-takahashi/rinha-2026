use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};
use tokio::net::UnixListener;

use fraud_detection::config::NPROBE_DEFAULT;
use fraud_detection::dataset::IvfData;
use fraud_detection::handlers::{self, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ivf = IvfData::load()?;
    let nprobe = std::env::var("NPROBE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(NPROBE_DEFAULT);
    // Fault the whole index into RSS before we accept any traffic, so the
    // first requests don't pay cold-start page-fault latency.
    ivf.warm();

    let shared_state = Arc::new(AppState { ivf, nprobe });

    let app = Router::new()
        .route("/ready", get(|| async { StatusCode::OK }))
        .route("/fraud-score", post(handlers::fraud_score))
        .with_state(shared_state);

    let socket_path = std::env::var("SOCKET_PATH").expect("SOCKET_PATH must be set");
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
    std::fs::set_permissions(&socket_path, PermissionsExt::from_mode(0o777))?;

    // Marker the compose healthcheck polls: only written once the index is warm
    // and the socket is bound, so nginx won't route to this instance until it's
    // ready to serve fast.
    std::fs::File::create("/tmp/ready")?;

    axum::serve(listener, app).await?;
    Ok(())
}
