pub const D: usize = 14;
pub const K: usize = 5;
pub const THRESHOLD: f32 = 0.6;

/// Number of IVF coarse cells (k-means centroids). ~sqrt(N) for N=3M balances
/// coarse-quantizer cost against per-cell scan cost.
pub const NLIST: usize = 2048;

/// Default cells probed per query. Tunable at runtime via the `NPROBE` env var
/// and swept against the agreement harness. nprobe=8 is the knee of the curve:
/// verdict agreement plateaus at 99.994% vs the exact oracle (measured on the
/// real 3M set), so higher values only cost throughput for no accuracy gain.
pub const NPROBE_DEFAULT: usize = 8;

/// Hard cap on nprobe so the coarse-selection scratch buffers can be fixed-size.
pub const MAX_NPROBE: usize = 64;

/// k-means build parameters (preprocess only).
pub const KMEANS_SAMPLE: usize = 256_000;
pub const KMEANS_ITERS: usize = 12;

pub const MAX_AMOUNT: f32 = 10_000.0;
pub const MAX_INSTALLMENTS: f32 = 12.0;
pub const AMOUNT_VS_AVG_RATIO: f32 = 10.0;
pub const MAX_MINUTES: f32 = 1440.0;
pub const MAX_KM: f32 = 1000.0;
pub const MAX_TX_COUNT_24H: f32 = 20.0;
pub const MAX_MERCHANT_AVG_AMOUNT: f32 = 10_000.0;

pub fn mcc_risk(mcc: &str) -> f32 {
    match mcc {
        "5411" => 0.15,
        "5812" => 0.30,
        "5912" => 0.20,
        "5944" => 0.45,
        "7801" => 0.80,
        "7802" => 0.75,
        "7995" => 0.85,
        "4511" => 0.35,
        "5311" => 0.25,
        "5999" => 0.50,
        _ => 0.50,
    }
}
