use crate::config::{D, K};
use ordered_float::OrderedFloat;
use std::collections::BinaryHeap;

fn squared_euclidean(a: &[f32; D], b: &[f32; D]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum()
}

pub fn knn(query: &[f32; D], vectors: &[[f32; D]], labels: &[u8]) -> [u8; K] {
    let mut heap: BinaryHeap<(OrderedFloat<f32>, u8)> = BinaryHeap::new();

    for (v, &l) in vectors.iter().zip(labels.iter()) {
        let dist = OrderedFloat(squared_euclidean(query, v));
        heap.push((dist, l));
        if heap.len() > K {
            heap.pop();
        }
    }

    let mut result = [0u8; K];
    for (i, (_, label)) in heap.into_iter().enumerate() {
        result[i] = label;
    }
    result
}
