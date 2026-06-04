//! Distance kernels for the IVF search hot path.
//!
//! Two layouts, both Structure-of-Arrays so we process 8 lanes per FMA with no
//! horizontal sums in the inner loop:
//!   - coarse: centroids stored [D][nlist] -> distance to all cells at once.
//!   - cell scan: each cell's f16 vectors stored [D][m] -> distance to 8 points
//!     at once, decoding f16->f32 with a single F16C instruction.
//!
//! x86_64 uses AVX2+FMA+F16C (guaranteed present under the project's
//! `target-cpu=native` build, and double-checked once at runtime). A scalar
//! fallback keeps non-x86 dev/test builds correct.

// SoA kernels index by dimension into computed offsets (d*nlist+c, d*m+i), so
// the range loops are intrinsic, not "needless".
#![allow(clippy::needless_range_loop)]

use crate::config::D;
use crate::topk::TopK;

#[cfg(not(target_endian = "little"))]
compile_error!("IVF index reinterpretation and f16 decoding assume little-endian");

#[cfg(target_arch = "x86_64")]
fn simd_ok() -> bool {
    use std::sync::OnceLock;
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        is_x86_feature_detected!("avx2")
            && is_x86_feature_detected!("fma")
            && is_x86_feature_detected!("f16c")
    })
}

/// Squared-L2 distance from `q` to every centroid. `centroids` is SoA `[D][nlist]`
/// (element (d,c) at `d*nlist + c`); `out` (len `nlist`) receives the distances.
#[inline]
pub fn coarse_distances(q: &[f32; D], centroids: &[f32], nlist: usize, out: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    {
        if simd_ok() {
            unsafe { coarse_avx2(q, centroids, nlist, out) };
            return;
        }
    }
    coarse_scalar(q, centroids, nlist, out);
}

/// Scan one cell's f16 SoA block (`[D][m]`, f16 bits), offering each point's
/// squared-L2 distance + label to `top`. `base` is the cell's global start so
/// labels line up.
#[inline]
pub fn scan_cell(
    q: &[f32; D],
    block: &[u16],
    m: usize,
    base: usize,
    labels: &[u8],
    top: &mut TopK,
) {
    #[cfg(target_arch = "x86_64")]
    {
        if simd_ok() {
            unsafe { scan_cell_avx2(q, block, m, base, labels, top) };
            return;
        }
    }
    scan_cell_scalar(q, block, m, base, labels, top);
}

// --- scalar fallbacks ---------------------------------------------------------

fn coarse_scalar(q: &[f32; D], centroids: &[f32], nlist: usize, out: &mut [f32]) {
    for (c, slot) in out.iter_mut().enumerate().take(nlist) {
        let mut s = 0.0f32;
        for d in 0..D {
            let diff = q[d] - centroids[d * nlist + c];
            s += diff * diff;
        }
        *slot = s;
    }
}

fn scan_cell_scalar(q: &[f32; D], block: &[u16], m: usize, base: usize, labels: &[u8], top: &mut TopK) {
    for i in 0..m {
        let mut s = 0.0f32;
        for d in 0..D {
            let v = half::f16::from_bits(block[d * m + i]).to_f32();
            let diff = q[d] - v;
            s += diff * diff;
        }
        top.offer(s, labels[base + i]);
    }
}

// --- x86_64 AVX2 + FMA + F16C -------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn coarse_avx2(q: &[f32; D], centroids: &[f32], nlist: usize, out: &mut [f32]) {
    use std::arch::x86_64::*;
    unsafe {
        let mut c = 0usize;
        while c + 8 <= nlist {
            let mut acc = _mm256_setzero_ps();
            for d in 0..D {
                let qd = _mm256_set1_ps(q[d]);
                let cv = _mm256_loadu_ps(centroids.as_ptr().add(d * nlist + c));
                let diff = _mm256_sub_ps(qd, cv);
                acc = _mm256_fmadd_ps(diff, diff, acc);
            }
            _mm256_storeu_ps(out.as_mut_ptr().add(c), acc);
            c += 8;
        }
        // tail (nlist is a multiple of 8 for the default config, but stay safe)
        while c < nlist {
            let mut s = 0.0f32;
            for d in 0..D {
                let diff = q[d] - *centroids.get_unchecked(d * nlist + c);
                s += diff * diff;
            }
            *out.get_unchecked_mut(c) = s;
            c += 1;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma,f16c")]
unsafe fn scan_cell_avx2(q: &[f32; D], block: &[u16], m: usize, base: usize, labels: &[u8], top: &mut TopK) {
    use std::arch::x86_64::*;
    unsafe {
        let mut i = 0usize;
        while i + 8 <= m {
            let mut acc = _mm256_setzero_ps();
            for d in 0..D {
                let halfs = _mm_loadu_si128(block.as_ptr().add(d * m + i) as *const __m128i);
                let cv = _mm256_cvtph_ps(halfs);
                let qd = _mm256_set1_ps(q[d]);
                let diff = _mm256_sub_ps(qd, cv);
                acc = _mm256_fmadd_ps(diff, diff, acc);
            }
            let mut dists = [0.0f32; 8];
            _mm256_storeu_ps(dists.as_mut_ptr(), acc);
            for (j, &dist) in dists.iter().enumerate() {
                top.offer(dist, *labels.get_unchecked(base + i + j));
            }
            i += 8;
        }
        while i < m {
            let mut s = 0.0f32;
            for d in 0..D {
                let v = half::f16::from_bits(*block.get_unchecked(d * m + i)).to_f32();
                let diff = q[d] - v;
                s += diff * diff;
            }
            top.offer(s, *labels.get_unchecked(base + i));
            i += 1;
        }
    }
}
