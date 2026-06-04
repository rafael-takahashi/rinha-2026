use flate2::read::GzDecoder;
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use serde::Deserialize;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
};

use fraud_detection::config::{D, KMEANS_ITERS, KMEANS_SAMPLE, NLIST};
use fraud_detection::ivf::IvfIndex;
use fraud_detection::kmeans;

#[derive(Deserialize)]
struct Record {
    vector: [f32; D],
    label: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let f = File::open("resources/references.json.gz")?;
    let gz = GzDecoder::new(f);
    let reader = BufReader::new(gz);

    let records: Vec<Record> = serde_json::from_reader(reader)?;

    fs::create_dir_all("data")?;

    let mut points: Vec<[f32; D]> = Vec::with_capacity(records.len());
    let mut labels: Vec<u8> = Vec::with_capacity(records.len());
    for rec in &records {
        let label: u8 = match rec.label.as_str() {
            "fraud" => 1,
            "legit" => 0,
            other => return Err(format!("unknown label: {other}").into()),
        };
        points.push(rec.vector);
        labels.push(label);
    }

    // labels.bin + KD-tree: consumed only by the offline oracle harness.
    let mut labels_file = BufWriter::new(File::create("data/labels.bin")?);
    labels_file.write_all(&labels)?;
    labels_file.flush()?;

    let tree: ImmutableKdTree<f32, u32, D, 32> = ImmutableKdTree::new_from_slice(&points);
    let tree_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&tree)?;
    fs::write("data/tree.rkyv", tree_bytes)?;

    // IVF-f16 index: the production runtime index.
    println!("training k-means: nlist={NLIST}, sample={KMEANS_SAMPLE}, iters={KMEANS_ITERS}");
    let centroids = kmeans::train(&points, NLIST, KMEANS_SAMPLE, KMEANS_ITERS);
    println!("assigning {} points to cells", points.len());
    let assignments = kmeans::assign_all(&points, &centroids, NLIST);
    let ivf = IvfIndex::build(&points, &labels, &centroids, &assignments);
    let ivf_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&ivf)?;
    fs::write("data/ivf.rkyv", &ivf_bytes)?;
    println!(
        "wrote data/ivf.rkyv ({:.1} MiB)",
        ivf_bytes.len() as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}
