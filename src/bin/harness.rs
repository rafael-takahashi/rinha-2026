//! Offline oracle + agreement/throughput harness.
//!
//! Replays captured query payloads and compares the production IVF verdict
//! against the exact KD-tree oracle (which the grader's expected verdict is
//! assumed to match). Reports:
//!   - verdict agreement and *directional* flips (missed-fraud vs false-alarm),
//!     so we can tune nprobe to ~100% agreement before shipping;
//!   - IVF throughput + latency percentiles (the metric that drives the load
//!     test on the 0.4-CPU slice) alongside the exact baseline;
//!   - RSS after loading the index (the memory wall the IVF change removes).
//!
//! If `data/ivf.rkyv` is absent it falls back to oracle-only baseline mode.
//!
//! Usage:
//!   [NPROBE=16] harness <queries.jsonl> [out_verdicts.jsonl]
//!
//! Capture `<queries.jsonl>` by running the API with `CAPTURE_PATH=/path.jsonl`
//! and driving it with the official load test.

use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::time::Instant;

use serde::Serialize;

use fraud_detection::config::{K, NPROBE_DEFAULT};
use fraud_detection::dataset::{Dataset, IvfData};
use fraud_detection::models::TransactionPayload;
use fraud_detection::score::{exact_score, ivf_score};

#[derive(Serialize)]
struct Verdict {
    exact_approved: bool,
    exact_fraud_score: f32,
    exact_fraud_count: u8,
    ivf_approved: Option<bool>,
    ivf_fraud_count: Option<u8>,
}

fn rss_bytes() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let resident_pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|f| f.parse().ok())
        .unwrap_or(0);
    resident_pages * 4096
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn percentile(sorted_ns: &[u64], p: f64) -> f64 {
    if sorted_ns.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * (sorted_ns.len() - 1) as f64).round() as usize;
    sorted_ns[rank] as f64 / 1000.0
}

fn report_latency(label: &str, mut ns: Vec<u64>, parsed: u64, wall_s: f64) {
    ns.sort_unstable();
    let mean_us = if parsed > 0 {
        ns.iter().map(|&n| n as u128).sum::<u128>() as f64 / parsed as f64 / 1000.0
    } else {
        0.0
    };
    let qps = if wall_s > 0.0 { parsed as f64 / wall_s } else { 0.0 };
    println!("--- {label} (single thread) ---");
    println!("throughput          : {qps:.1} queries/s");
    println!("latency mean        : {mean_us:.1} us");
    println!("latency p50         : {:.1} us", percentile(&ns, 50.0));
    println!("latency p99         : {:.1} us", percentile(&ns, 99.0));
    println!("latency p999        : {:.1} us", percentile(&ns, 99.9));
    println!(
        "latency max         : {:.1} us",
        ns.last().copied().unwrap_or(0) as f64 / 1000.0
    );
    println!();
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let queries_path = args
        .next()
        .ok_or("usage: [NPROBE=N] harness <queries.jsonl> [out_verdicts.jsonl]")?;
    let out_path = args.next();
    let nprobe: usize = std::env::var("NPROBE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(NPROBE_DEFAULT);

    let dataset = Dataset::load()?;
    let ivf = IvfData::load().ok();
    if ivf.is_none() {
        println!("(no data/ivf.rkyv -> oracle-only baseline mode)\n");
    }
    let rss_after_load = rss_bytes();

    let reader = BufReader::new(File::open(&queries_path)?);
    let mut writer = match &out_path {
        Some(p) => Some(BufWriter::new(File::create(p)?)),
        None => None,
    };

    let mut histogram = [0u64; K + 1];
    let mut exact_ns: Vec<u64> = Vec::new();
    let mut ivf_ns: Vec<u64> = Vec::new();
    let mut parsed = 0u64;
    let mut skipped = 0u64;

    // agreement tallies
    let mut approved_match = 0u64;
    let mut count_match = 0u64;
    let mut missed_fraud = 0u64; // exact declined (fraud), ivf approved
    let mut false_alarm = 0u64; // exact approved, ivf declined

    let exact_start = Instant::now();
    let mut ivf_wall_ns = 0u128;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let payload: TransactionPayload<'_> = match serde_json::from_str(&line) {
            Ok(p) => p,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let t0 = Instant::now();
        let exact = exact_score(&payload, &dataset);
        exact_ns.push(t0.elapsed().as_nanos() as u64);
        histogram[exact.fraud_count as usize] += 1;
        parsed += 1;

        let ivf_v = ivf.as_ref().map(|index| {
            let t1 = Instant::now();
            let scored = ivf_score(&payload, index.index(), nprobe);
            let dt = t1.elapsed().as_nanos();
            ivf_ns.push(dt as u64);
            ivf_wall_ns += dt;
            scored
        });

        if let Some(scored) = &ivf_v {
            if scored.response.approved == exact.response.approved {
                approved_match += 1;
            } else if exact.response.approved {
                false_alarm += 1;
            } else {
                missed_fraud += 1;
            }
            if scored.fraud_count == exact.fraud_count {
                count_match += 1;
            }
        }

        if let Some(w) = writer.as_mut() {
            let v = Verdict {
                exact_approved: exact.response.approved,
                exact_fraud_score: exact.response.fraud_score,
                exact_fraud_count: exact.fraud_count,
                ivf_approved: ivf_v.as_ref().map(|s| s.response.approved),
                ivf_fraud_count: ivf_v.as_ref().map(|s| s.fraud_count),
            };
            serde_json::to_writer(&mut *w, &v)?;
            w.write_all(b"\n")?;
        }
    }
    let exact_wall = exact_start.elapsed().as_secs_f64();
    if let Some(mut w) = writer {
        w.flush()?;
    }

    println!("=== harness ===");
    println!("queries file        : {queries_path}");
    println!("parsed / skipped    : {parsed} / {skipped}");
    println!("RSS after load      : {:.1} MiB", mib(rss_after_load));
    println!();

    report_latency("exact KD-tree", exact_ns, parsed, exact_wall);
    if ivf.is_some() {
        report_latency(
            &format!("IVF-f16 (nprobe={nprobe})"),
            ivf_ns,
            parsed,
            ivf_wall_ns as f64 / 1e9,
        );

        let pct = |n: u64| if parsed > 0 { 100.0 * n as f64 / parsed as f64 } else { 0.0 };
        println!("--- agreement vs exact oracle (nprobe={nprobe}) ---");
        println!(
            "verdict agreement   : {approved_match}/{parsed} ({:.3}%)",
            pct(approved_match)
        );
        println!("fraud-count match   : {:.3}%", pct(count_match));
        println!(
            "MISSED FRAUD (exact declined, ivf approved) : {missed_fraud} ({:.3}%)",
            pct(missed_fraud)
        );
        println!(
            "false alarm  (exact approved, ivf declined) : {false_alarm} ({:.3}%)",
            pct(false_alarm)
        );
        println!();
    }

    println!("--- exact fraud-count distribution (verdict flips at 3/5) ---");
    for (count, &n) in histogram.iter().enumerate() {
        let pct = if parsed > 0 { 100.0 * n as f64 / parsed as f64 } else { 0.0 };
        let band = match count {
            2 => "  <- approved, 1 from flipping",
            3 => "  <- declined, 1 from flipping",
            _ => "",
        };
        println!("  {count}/{K}: {n:>10}  ({pct:5.2}%){band}");
    }
    let boundary = histogram.get(2).copied().unwrap_or(0) + histogram.get(3).copied().unwrap_or(0);
    println!(
        "boundary band (2,3) : {boundary} ({:.2}%)",
        if parsed > 0 { 100.0 * boundary as f64 / parsed as f64 } else { 0.0 }
    );

    Ok(())
}
