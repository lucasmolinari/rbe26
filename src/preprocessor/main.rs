use flate2::read::GzDecoder;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fs::File;
use std::io::{BufWriter, Write};

use rbe26::buckets::{BUCKET_COUNT, VECTOR_BYTES, VECTOR_DIMS, bucket_key_from_quantized};

const VECTOR_SCALE: f32 = 32767.0;

#[derive(Deserialize)]
struct Reference {
    pub vector: [f64; VECTOR_DIMS],
    #[serde(rename = "label", deserialize_with = "deserialize_label")]
    pub is_fraud: bool,
}

struct Record {
    key: u32,
    label: u8,
    vector: [u8; VECTOR_BYTES],
}

struct Collector {
    records: Vec<Record>,
}

struct CollectorVisitor;

fn deserialize_label<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s == "fraud")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vectors_path = "resources/vectors.bin";
    println!("parsing references.json.gz and writing bucketed compact vectors ...");

    let file = File::open("resources/references.json.gz")?;
    let decoder = GzDecoder::new(file);
    let reader = std::io::BufReader::new(decoder);
    let mut de = serde_json::Deserializer::from_reader(reader);

    let Collector { mut records } = de.deserialize_seq(CollectorVisitor)?;
    records.sort_unstable_by_key(|record| record.key);

    let mut offsets = vec![0u32; BUCKET_COUNT + 1];
    for record in &records {
        offsets[record.key as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }

    let out = File::create(vectors_path)?;
    let mut writer = BufWriter::new(out);
    writer.write_all(&(records.len() as u32).to_le_bytes())?;
    writer.write_all(&VECTOR_SCALE.to_le_bytes())?;
    writer.write_all(&(BUCKET_COUNT as u32).to_le_bytes())?;
    for offset in offsets {
        writer.write_all(&offset.to_le_bytes())?;
    }
    for record in &records {
        writer.write_all(&record.vector)?;
    }
    for record in &records {
        writer.write_all(&[record.label])?;
    }
    writer.flush()?;

    println!(
        "preprocessor done - {} bucketed compact vectors written to {}",
        records.len(),
        vectors_path
    );

    Ok(())
}

impl<'de> Visitor<'de> for CollectorVisitor {
    type Value = Collector;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an array of references")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut records = Vec::new();

        while let Some(reference) = seq.next_element::<Reference>()? {
            let mut quantized = [0i16; VECTOR_DIMS];
            let mut vector = [0u8; VECTOR_BYTES];

            for (dim, value) in reference.vector.iter().enumerate() {
                let value = (value.clamp(-1.0, 1.0) * VECTOR_SCALE as f64).round() as i16;
                quantized[dim] = value;
                vector[dim * 2..dim * 2 + 2].copy_from_slice(&value.to_le_bytes());
            }

            records.push(Record {
                key: bucket_key_from_quantized(&quantized) as u32,
                label: reference.is_fraud as u8,
                vector,
            });
        }

        Ok(Collector { records })
    }
}
