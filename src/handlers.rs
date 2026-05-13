use std::sync::Arc;

use axum::extract::State;

use crate::json::Json;

use crate::config::{K, THRESHOLD};
use crate::dataset::Dataset;
use crate::knn::knn;
use crate::models::{FraudResponse, TransactionPayload};
use crate::vectorize::vectorize;

pub async fn fraud_score(
    State(dataset): State<Arc<Dataset>>,
    Json(payload): Json<TransactionPayload>,
) -> Json<FraudResponse> {
    let v = vectorize(&payload);
    let number_of_frauds: u8 = knn(&v, dataset.kd_tree(), dataset.labels()).iter().sum();

    let fraud_score = number_of_frauds as f32 / K as f32;

    Json(FraudResponse {
        approved: fraud_score < THRESHOLD,
        fraud_score,
    })
}
