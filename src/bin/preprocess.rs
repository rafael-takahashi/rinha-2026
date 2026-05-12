use flate2::read::GzDecoder;
use kiddo::immutable::float::kdtree::ImmutableKdTree;
use serde::Deserialize;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
};

#[derive(Deserialize)]
struct Record {
    vector: [f32; 14],
    label: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let f = File::open("resources/references.json.gz")?;
    let gz = GzDecoder::new(f);
    let reader = BufReader::new(gz);

    let records: Vec<Record> = serde_json::from_reader(reader)?;

    fs::create_dir_all("data")?;

    let mut labels = BufWriter::new(File::create("data/labels.bin")?);
    let mut points: Vec<[f32; 14]> = Vec::with_capacity(records.len());

    for rec in &records {
        let label: u8 = match rec.label.as_str() {
            "fraud" => 1,
            "legit" => 0,
            other => return Err(format!("unknown label: {other}").into()),
        };
        points.push(rec.vector);
        labels.write_all(&[label])?;
    }

    labels.flush()?;

    let tree: ImmutableKdTree<f32, u32, 14, 32> = ImmutableKdTree::new_from_slice(&points);

    let archived_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&tree)?;
    fs::write("data/tree.rkyv", archived_bytes)?;

    Ok(())
}
