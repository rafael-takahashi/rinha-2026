//! Scale validation + nprobe sweep on the real reference set.
//!
//! Samples query vectors from the dataset (with small noise so they aren't exact
//! DB points), computes the exact KD-tree fraud count once per query as the
//! oracle, then sweeps nprobe through the IVF index reporting verdict agreement,
//! directional flips, and throughput. This is the curve we use to pick nprobe.
//!
//! Usage: validate   (reads resources/references.json.gz + data/{tree.rkyv,ivf.rkyv})

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

use flate2::read::GzDecoder;
use serde::Deserialize;

use fraud_detection::config::{D, K, THRESHOLD};
use fraud_detection::dataset::{Dataset, IvfData};
use fraud_detection::ivf;
use fraud_detection::knn::knn;

#[derive(Deserialize)]
struct Record {
    vector: [f32; D],
    #[allow(dead_code)]
    label: String,
}

const SAMPLE: usize = 50_000;
const NPROBES: [usize; 7] = [1, 2, 4, 8, 16, 32, 64];

fn rss_mib() -> f64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|f| f.parse().ok())
        .unwrap_or(0);
    (pages * 4096) as f64 / (1024.0 * 1024.0)
}

fn percentile(sorted_ns: &[u64], p: f64) -> f64 {
    if sorted_ns.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * (sorted_ns.len() - 1) as f64).round() as usize;
    sorted_ns[rank] as f64 / 1000.0
}

fn approved(fraud_count: u8) -> bool {
    (fraud_count as f32 / K as f32) < THRESHOLD
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("loading reference vectors...");
    let f = File::open("resources/references.json.gz")?;
    let records: Vec<Record> = serde_json::from_reader(BufReader::new(GzDecoder::new(f)))?;
    let n = records.len();
    println!("  {n} reference vectors");

    let dataset = Dataset::load()?;
    let ivf = IvfData::load()?;
    let idx = ivf.index();

    // Build a noisy query set sampled across the dataset.
    let stride = (n / SAMPLE).max(1);
    let mut state = 0xDEAD_BEEF_1234_5678u64;
    let mut noise = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        // uniform in [-0.02, 0.02]
        ((state >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.04
    };
    let mut queries: Vec<[f32; D]> = Vec::with_capacity(SAMPLE);
    for rec in records.iter().step_by(stride) {
        let mut q = rec.vector;
        for v in &mut q {
            *v += noise();
        }
        queries.push(q);
    }
    let q_n = queries.len();
    println!("  {q_n} sampled queries\n");

    // Oracle: exact KD-tree fraud count per query (independent of nprobe).
    let t = Instant::now();
    let oracle: Vec<u8> = queries
        .iter()
        .map(|q| knn(q, dataset.kd_tree(), dataset.labels()).iter().sum())
        .collect();
    let oracle_qps = q_n as f64 / t.elapsed().as_secs_f64();
    println!("RSS after load (tree + ivf): {:.1} MiB", rss_mib());
    println!("exact oracle: {oracle_qps:.0} queries/s (single thread)\n");

    println!(
        "{:>7} | {:>10} | {:>8} | {:>8} | {:>10} | {:>9}",
        "nprobe", "agreement", "missed", "falsealr", "qps", "p99(us)"
    );
    println!("{}", "-".repeat(66));

    for &nprobe in &NPROBES {
        let mut agree = 0u64;
        let mut missed = 0u64; // oracle declined (fraud), ivf approved
        let mut false_alarm = 0u64; // oracle approved, ivf declined
        let mut lat = Vec::with_capacity(q_n);

        for (q, &exact_count) in queries.iter().zip(&oracle) {
            let t0 = Instant::now();
            let ivf_count = ivf::search(idx, q, nprobe);
            lat.push(t0.elapsed().as_nanos() as u64);

            let (ea, ia) = (approved(exact_count), approved(ivf_count));
            if ea == ia {
                agree += 1;
            } else if ea {
                false_alarm += 1;
            } else {
                missed += 1;
            }
        }

        let total_ns: u128 = lat.iter().map(|&x| x as u128).sum();
        let qps = q_n as f64 / (total_ns as f64 / 1e9);
        lat.sort_unstable();
        let pct = |x: u64| 100.0 * x as f64 / q_n as f64;
        println!(
            "{:>7} | {:>9.4}% | {:>8} | {:>8} | {:>10.0} | {:>9.1}",
            nprobe,
            pct(agree),
            missed,
            false_alarm,
            qps,
            percentile(&lat, 99.0)
        );
    }

    Ok(())
}
