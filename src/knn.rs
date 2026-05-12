use crate::config::{D, K};
use kiddo::SquaredEuclidean;
use kiddo::immutable::float::kdtree::ArchivedR8ImmutableKdTree;
use std::num::NonZero;

pub fn knn(
    query: &[f32; D],
    tree: &ArchivedR8ImmutableKdTree<f32, u32, D, 32>,
    labels: &[u8],
) -> [u8; K] {
    let neighbours = tree.nearest_n::<SquaredEuclidean>(query, NonZero::new(K).unwrap());

    let mut result = [0u8; K];
    for (i, n) in neighbours.iter().enumerate() {
        result[i] = labels[n.item as usize];
    }
    result
}
