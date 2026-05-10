use flate2::read::GzDecoder;
use serde::{
    Deserialize, Deserializer,
    de::{self, SeqAccess, Visitor},
};
use std::{
    error::Error,
    fmt,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
};

#[derive(Deserialize)]
struct Record {
    vector: [f32; 14],
    label: String,
}

struct RecordSink<'a> {
    vectors: &'a mut BufWriter<File>,
    labels: &'a mut BufWriter<File>,
}

impl<'de, 'a> Visitor<'de> for RecordSink<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("array of records")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(rec) = seq.next_element::<Record>()? {
            let label: u8 = match rec.label.as_str() {
                "fraud" => 1,
                "legit" => 0,
                other => return Err(de::Error::custom(format!("unknown label: {other}"))),
            };

            for val in &rec.vector {
                self.vectors
                    .write_all(&val.to_le_bytes())
                    .map_err(de::Error::custom)?;
            }

            self.labels.write_all(&[label]).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let f = File::open("resources/references.json.gz")?;
    let gz = GzDecoder::new(f);
    let reader = BufReader::new(gz);

    fs::create_dir_all("data")?;

    let mut vectors = BufWriter::new(File::create("data/vectors.bin")?);
    let mut labels = BufWriter::new(File::create("data/labels.bin")?);

    let sink = RecordSink {
        vectors: &mut vectors,
        labels: &mut labels,
    };

    serde_json::Deserializer::from_reader(reader).deserialize_seq(sink)?;

    vectors.flush()?;
    labels.flush()?;

    Ok(())
}
