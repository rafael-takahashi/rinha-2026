//! IVF (inverted file) index: coarse k-means cells + f16 vectors stored
//! Structure-of-Arrays per cell. Built in `preprocess`, serialized with rkyv,
//! mmap'd and queried zero-copy at runtime.
//!
//! Query: squared-L2 to all `nlist` centroids -> probe the `nprobe` nearest
//! cells -> scan their f16 blocks with the AVX2+F16C kernel -> top-K labels.

use crate::config::{D, MAX_NPROBE, NLIST};
use crate::simd::{coarse_distances, scan_cell};
use crate::topk::TopK;
use half::f16;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Serialize, Deserialize)]
pub struct IvfIndex {
    pub nlist: u32,
    /// Centroids, SoA `[D][nlist]` (element (d,c) at `d*nlist + c`), f32 for an
    /// accurate coarse step.
    pub centroids_soa: Vec<f32>,
    /// Prefix-sum cell directory, len `nlist+1`. Cell c spans points
    /// `[cell_offsets[c], cell_offsets[c+1])`.
    pub cell_offsets: Vec<u32>,
    /// Labels (0/1), reordered to group points by cell. Len N.
    pub labels: Vec<u8>,
    /// f16 bits, per-cell SoA: cell c's block starts at `cell_offsets[c]*D` and
    /// is laid out `[D][m]` where m is the cell size. Len N*D.
    pub codes: Vec<u16>,
}

impl IvfIndex {
    /// Pack points into the per-cell SoA layout given precomputed k-means
    /// centroids (row-major `[nlist][D]`) and per-point cell `assignments`.
    pub fn build(
        points: &[[f32; D]],
        labels: &[u8],
        centroids_rowmajor: &[f32],
        assignments: &[u32],
    ) -> Self {
        let nlist = centroids_rowmajor.len() / D;
        let n = points.len();

        let mut counts = vec![0u32; nlist];
        for &a in assignments {
            counts[a as usize] += 1;
        }
        let mut cell_offsets = vec![0u32; nlist + 1];
        for c in 0..nlist {
            cell_offsets[c + 1] = cell_offsets[c] + counts[c];
        }

        let mut codes = vec![0u16; n * D];
        let mut labels_out = vec![0u8; n];
        let mut cursor: Vec<u32> = cell_offsets[..nlist].to_vec();
        for p in 0..n {
            let c = assignments[p] as usize;
            let base = cell_offsets[c] as usize;
            let m = counts[c] as usize;
            let pos = cursor[c] as usize;
            cursor[c] += 1;
            let local = pos - base;
            for d in 0..D {
                codes[base * D + d * m + local] = f16::from_f32(points[p][d]).to_bits();
            }
            labels_out[pos] = labels[p];
        }

        let mut centroids_soa = vec![0.0f32; nlist * D];
        for c in 0..nlist {
            for d in 0..D {
                centroids_soa[d * nlist + c] = centroids_rowmajor[c * D + d];
            }
        }

        IvfIndex {
            nlist: nlist as u32,
            centroids_soa,
            cell_offsets,
            labels: labels_out,
            codes,
        }
    }
}

/// Reinterpret a slice of rkyv-archived scalar primitives as their native type.
/// Sound on little-endian, where rkyv's archived scalars are layout-identical to
/// the native primitive (enforced by the `compile_error!` in `simd.rs`).
#[inline]
fn as_native<A, T>(s: &[A]) -> &[T] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const T, s.len()) }
}

/// Pick the `nprobe` nearest cells (smallest distances) into `out`, returning the
/// count. Order within `out` is irrelevant since every probed cell is scanned.
#[allow(clippy::needless_range_loop)] // index tracks worst_i alongside the value
fn select_nprobe(dists: &[f32], nprobe: usize, out: &mut [u32; MAX_NPROBE]) -> usize {
    let k = nprobe.min(MAX_NPROBE).min(dists.len());
    if k == 0 {
        return 0;
    }
    let mut best = [f32::INFINITY; MAX_NPROBE];
    let mut filled = 0usize;
    let mut worst = f32::INFINITY;
    let mut worst_i = 0usize;
    for (c, &d) in dists.iter().enumerate() {
        if filled < k {
            best[filled] = d;
            out[filled] = c as u32;
            filled += 1;
            if filled == k {
                worst = best[0];
                worst_i = 0;
                for j in 1..k {
                    if best[j] > worst {
                        worst = best[j];
                        worst_i = j;
                    }
                }
            }
        } else if d < worst {
            best[worst_i] = d;
            out[worst_i] = c as u32;
            worst = best[0];
            worst_i = 0;
            for j in 1..k {
                if best[j] > worst {
                    worst = best[j];
                    worst_i = j;
                }
            }
        }
    }
    k
}

/// Approximate K-NN fraud count for `q`: probes `nprobe` cells and returns the
/// number of fraud labels among the K nearest f16 points found.
pub fn search(idx: &ArchivedIvfIndex, q: &[f32; D], nprobe: usize) -> u8 {
    let nlist = idx.nlist.to_native() as usize;
    debug_assert!(nlist <= NLIST);
    let centroids: &[f32] = as_native(idx.centroids_soa.as_slice());
    let offsets: &[u32] = as_native(idx.cell_offsets.as_slice());
    let labels: &[u8] = idx.labels.as_slice();
    let codes: &[u16] = as_native(idx.codes.as_slice());

    let mut dists = [0.0f32; NLIST];
    coarse_distances(q, centroids, nlist, &mut dists[..nlist]);

    let mut cells = [0u32; MAX_NPROBE];
    let k = select_nprobe(&dists[..nlist], nprobe, &mut cells);

    let mut top = TopK::new();
    for &cell in &cells[..k] {
        let c = cell as usize;
        let base = offsets[c] as usize;
        let end = offsets[c + 1] as usize;
        let m = end - base;
        let blk = &codes[base * D..end * D];
        scan_cell(q, blk, m, base, labels, &mut top);
    }
    top.fraud_count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::K;
    use crate::kmeans;

    fn gen_points(n: usize) -> (Vec<[f32; D]>, Vec<u8>) {
        // deterministic pseudo-random points in [0,1)
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut points = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        for _ in 0..n {
            let mut p = [0.0f32; D];
            for v in &mut p {
                *v = next();
            }
            points.push(p);
            labels.push((next() < 0.3) as u8);
        }
        (points, labels)
    }

    /// Brute-force exact K-NN fraud count over f16-rounded points, so any
    /// mismatch with `search` at nprobe == nlist is a layout/kernel bug, not an
    /// f16-rounding difference.
    fn exact_f16(points: &[[f32; D]], labels: &[u8], q: &[f32; D]) -> u8 {
        let mut d: Vec<(f32, u8)> = points
            .iter()
            .zip(labels)
            .map(|(p, &l)| {
                let mut s = 0.0f32;
                for k in 0..D {
                    let v = f16::from_f32(p[k]).to_f32();
                    let x = q[k] - v;
                    s += x * x;
                }
                (s, l)
            })
            .collect();
        d.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        d[..K].iter().map(|x| x.1).sum()
    }

    #[test]
    fn search_at_full_nprobe_matches_bruteforce() {
        let nlist = 16;
        let (points, labels) = gen_points(400);
        let centroids = kmeans::train(&points, nlist, points.len(), 5);
        let assignments = kmeans::assign_all(&points, &centroids, nlist);
        let ivf = IvfIndex::build(&points, &labels, &centroids, &assignments);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&ivf).unwrap();
        let archived = unsafe { rkyv::access_unchecked::<ArchivedIvfIndex>(&bytes) };

        // exhaustive probing (nprobe == nlist) must reproduce brute force.
        let (queries, _) = gen_points(50);
        for q in &queries {
            let got = search(archived, q, nlist);
            let want = exact_f16(&points, &labels, q);
            assert_eq!(got, want, "fraud count mismatch for query {q:?}");
        }
    }
}
