use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::dataset::IvfData;
use crate::json::{self, Json};
use crate::models::TransactionPayload;
use crate::score::ivf_score;

/// Shared serving state: the mmap'd IVF index plus the number of cells to probe
/// per query (tuned against the agreement harness, overridable via `NPROBE`).
pub struct AppState {
    pub ivf: IvfData,
    pub nprobe: usize,
}

pub async fn fraud_score(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    json::capture(&body);
    // Parse here (not in an extractor) so the borrowed &str fields can reference
    // `body`, which stays alive for the whole handler.
    let payload: TransactionPayload<'_> = match sonic_rs::from_slice(&body) {
        Ok(p) => p,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    Json(ivf_score(&payload, state.ivf.index(), state.nprobe).response).into_response()
}
