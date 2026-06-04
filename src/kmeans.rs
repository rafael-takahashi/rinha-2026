//! Minimal parallel k-means for building the IVF coarse quantizer. Runs only in
//! `preprocess` (at docker-build time, full cores), so it favors clarity over
//! squeezing the last few percent. std threads only — no extra deps.

use crate::config::D;
use std::thread;

#[inline]
fn dist2(p: &[f32; D], c: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for d in 0..D {
        let x = p[d] - c[d];
        s += x * x;
    }
    s
}

#[inline]
fn nearest(p: &[f32; D], centroids: &[f32], nlist: usize) -> u32 {
    let mut best = 0u32;
    let mut bd = f32::INFINITY;
    for c in 0..nlist {
        let s = dist2(p, &centroids[c * D..c * D + D]);
        if s < bd {
            bd = s;
            best = c as u32;
        }
    }
    best
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn nthreads() -> usize {
    thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

/// One Lloyd assignment+accumulate step over `sample`, parallelized. Returns
/// (per-cluster coordinate sums `[nlist*D]`, per-cluster counts `[nlist]`).
fn accumulate(sample: &[[f32; D]], centroids: &[f32], nlist: usize) -> (Vec<f32>, Vec<u64>) {
    let nt = nthreads();
    let chunk = sample.len().div_ceil(nt).max(1);

    let partials: Vec<(Vec<f32>, Vec<u64>)> = thread::scope(|scope| {
        let mut handles = Vec::new();
        for pchunk in sample.chunks(chunk) {
            handles.push(scope.spawn(move || {
                let mut sums = vec![0.0f32; nlist * D];
                let mut counts = vec![0u64; nlist];
                for p in pchunk {
                    let c = nearest(p, centroids, nlist) as usize;
                    for d in 0..D {
                        sums[c * D + d] += p[d];
                    }
                    counts[c] += 1;
                }
                (sums, counts)
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut sums = vec![0.0f32; nlist * D];
    let mut counts = vec![0u64; nlist];
    for (ps, pc) in partials {
        for i in 0..nlist * D {
            sums[i] += ps[i];
        }
        for i in 0..nlist {
            counts[i] += pc[i];
        }
    }
    (sums, counts)
}

/// Train `nlist` centroids on a strided subsample of `points`. Returns row-major
/// centroids `[nlist][D]`.
pub fn train(points: &[[f32; D]], nlist: usize, sample_size: usize, iters: usize) -> Vec<f32> {
    let n = points.len();
    let stride = (n / sample_size.max(1)).max(1);
    let sample: Vec<[f32; D]> = points.iter().step_by(stride).copied().collect();
    let s = sample.len().max(1);
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);

    // k-means++ initialization.
    let mut centroids = vec![0.0f32; nlist * D];
    let first = (rng.next_u64() as usize) % s;
    centroids[0..D].copy_from_slice(&sample[first]);
    let mut min_d = vec![0.0f32; s];
    for i in 0..s {
        min_d[i] = dist2(&sample[i], &centroids[0..D]);
    }
    for c in 1..nlist {
        let total: f64 = min_d.iter().map(|&x| x as f64).sum();
        let mut target = rng.next_f32() as f64 * total;
        let mut pick = s - 1;
        for (i, &md) in min_d.iter().enumerate() {
            target -= md as f64;
            if target <= 0.0 {
                pick = i;
                break;
            }
        }
        centroids[c * D..c * D + D].copy_from_slice(&sample[pick]);
        for i in 0..s {
            let dd = dist2(&sample[i], &centroids[c * D..c * D + D]);
            if dd < min_d[i] {
                min_d[i] = dd;
            }
        }
    }

    // Lloyd iterations.
    for _ in 0..iters {
        let (sums, counts) = accumulate(&sample, &centroids, nlist);
        for c in 0..nlist {
            if counts[c] > 0 {
                let inv = 1.0 / counts[c] as f32;
                for d in 0..D {
                    centroids[c * D + d] = sums[c * D + d] * inv;
                }
            } else {
                // Reseed an empty cluster onto a random sample point.
                let r = (rng.next_u64() as usize) % s;
                centroids[c * D..c * D + D].copy_from_slice(&sample[r]);
            }
        }
    }

    centroids
}

/// Assign every point to its nearest centroid, parallelized. Returns one cell id
/// per point.
pub fn assign_all(points: &[[f32; D]], centroids: &[f32], nlist: usize) -> Vec<u32> {
    let mut out = vec![0u32; points.len()];
    let nt = nthreads();
    let chunk = points.len().div_ceil(nt).max(1);

    thread::scope(|scope| {
        for (pchunk, ochunk) in points.chunks(chunk).zip(out.chunks_mut(chunk)) {
            scope.spawn(move || {
                for (o, p) in ochunk.iter_mut().zip(pchunk.iter()) {
                    *o = nearest(p, centroids, nlist);
                }
            });
        }
    });

    out
}
