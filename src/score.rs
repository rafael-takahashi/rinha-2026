use crate::config::{K, THRESHOLD};
use crate::dataset::Dataset;
use crate::ivf::{self, ArchivedIvfIndex};
use crate::knn::knn;
use crate::models::{FraudResponse, TransactionPayload};
use crate::vectorize::vectorize;

/// A scored transaction: the response we'd return, plus the raw fraud count
/// (0..=K) so callers (e.g. the oracle harness) can inspect how close a verdict
/// is to the decision boundary.
pub struct Scored {
    pub response: FraudResponse,
    pub fraud_count: u8,
}

/// Exact K-NN scoring over the immutable KD-tree. This is the shared scoring
/// path used by both the HTTP handler and the offline oracle harness, so the
/// two can never drift.
pub fn exact_score(payload: &TransactionPayload<'_>, dataset: &Dataset) -> Scored {
    let v = vectorize(payload);
    let fraud_count: u8 = knn(&v, dataset.kd_tree(), dataset.labels()).iter().sum();
    let fraud_score = fraud_count as f32 / K as f32;

    Scored {
        response: FraudResponse {
            approved: fraud_score < THRESHOLD,
            fraud_score,
        },
        fraud_count,
    }
}

/// Approximate IVF scoring, the production path. Same verdict shape as
/// [`exact_score`] so the two can be compared directly by the agreement harness.
pub fn ivf_score(payload: &TransactionPayload<'_>, idx: &ArchivedIvfIndex, nprobe: usize) -> Scored {
    let v = vectorize(payload);
    let fraud_count = ivf::search(idx, &v, nprobe);
    let fraud_score = fraud_count as f32 / K as f32;

    Scored {
        response: FraudResponse {
            approved: fraud_score < THRESHOLD,
            fraud_score,
        },
        fraud_count,
    }
}
