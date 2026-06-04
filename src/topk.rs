use crate::config::K;

/// Fixed-size top-K (K=5) nearest accumulator. Cheaper than a heap at this size:
/// most candidates are rejected by a single `d >= worst` compare, and we only
/// rescan the K slots to find the new worst on an actual insert (rare after the
/// first K candidates).
pub struct TopK {
    dist: [f32; K],
    label: [u8; K],
    worst: f32,
    worst_idx: usize,
}

impl TopK {
    pub fn new() -> Self {
        TopK {
            dist: [f32::INFINITY; K],
            label: [0; K],
            worst: f32::INFINITY,
            worst_idx: 0,
        }
    }

    #[inline]
    pub fn offer(&mut self, d: f32, label: u8) {
        if d >= self.worst {
            return;
        }
        self.dist[self.worst_idx] = d;
        self.label[self.worst_idx] = label;
        // Recompute the worst-of-K. Only runs on an accept.
        let mut wi = 0;
        let mut wv = self.dist[0];
        for k in 1..K {
            if self.dist[k] > wv {
                wv = self.dist[k];
                wi = k;
            }
        }
        self.worst = wv;
        self.worst_idx = wi;
    }

    /// Number of fraud labels among the K nearest (labels are 0/1).
    #[inline]
    pub fn fraud_count(&self) -> u8 {
        self.label.iter().sum()
    }
}

impl Default for TopK {
    fn default() -> Self {
        Self::new()
    }
}
