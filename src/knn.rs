use crate::config::{D, K};
use ordered_float::OrderedFloat;

fn squared_euclidean(a: &[f32; D], b: &[f32; D]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum()
}

pub fn knn(query: &[f32; D], vectors: &[[f32; D]], labels: &[u8]) -> [u8; K] {
    let mut candidates: [(OrderedFloat<f32>, u8); 5] = [(OrderedFloat(f32::INFINITY), 0); K];

    let mut max_idx: usize = 0;
    let mut max_dist = OrderedFloat(f32::INFINITY);

    for (v, &l) in vectors.iter().zip(labels.iter()) {
        let dist = OrderedFloat(squared_euclidean(query, v));
        if dist < max_dist {
            candidates[max_idx] = (dist, l);
            let (new_max_idx, &(new_max_dist, _)) = candidates.iter().enumerate().max().unwrap();
            max_idx = new_max_idx;
            max_dist = new_max_dist;
        }
    }

    candidates.map(|t| t.1)
}
