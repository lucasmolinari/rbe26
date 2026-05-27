use flate2::read::GzDecoder;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fs::File;
use std::io::{BufWriter, Write};

use rbe26::buckets::{
    BLOCK_SIZE, PARTITION_COUNT, VECTOR_DIMS, partition_key_from_quantized, sort_key_from_quantized,
};

const VECTOR_SCALE: f32 = 32767.0;

#[derive(Deserialize)]
struct Reference {
    pub vector: [f64; VECTOR_DIMS],
    #[serde(rename = "label", deserialize_with = "deserialize_label")]
    pub is_fraud: bool,
}

struct Record {
    partition: u32,
    sort_key: u32,
    label: u8,
    vector: [i16; VECTOR_DIMS],
}

struct Block {
    start: u32,
    end: u32,
    min: [i16; VECTOR_DIMS],
    max: [i16; VECTOR_DIMS],
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
    println!("parsing references.json.gz and writing exact block index ...");

    let file = File::open("resources/references.json.gz")?;
    let decoder = GzDecoder::new(file);
    let reader = std::io::BufReader::new(decoder);
    let mut de = serde_json::Deserializer::from_reader(reader);

    let Collector { mut records } = de.deserialize_seq(CollectorVisitor)?;
    records.sort_unstable_by_key(|record| (record.partition, record.sort_key));

    let mut partition_offsets = vec![0u32; PARTITION_COUNT + 1];
    let mut blocks = Vec::new();

    let mut start = 0usize;
    for partition in 0..PARTITION_COUNT {
        partition_offsets[partition] = blocks.len() as u32;

        while start < records.len() && (records[start].partition as usize) < partition {
            start += 1;
        }

        let mut end = start;
        while end < records.len() && records[end].partition as usize == partition {
            end += 1;
        }

        let mut block_start = start;
        while block_start < end {
            let block_end = (block_start + BLOCK_SIZE).min(end);
            blocks.push(build_block(&records, block_start, block_end));
            block_start = block_end;
        }

        start = end;
    }
    partition_offsets[PARTITION_COUNT] = blocks.len() as u32;

    let out = File::create(vectors_path)?;
    let mut writer = BufWriter::new(out);
    writer.write_all(&(records.len() as u32).to_le_bytes())?;
    writer.write_all(&VECTOR_SCALE.to_le_bytes())?;
    writer.write_all(&(blocks.len() as u32).to_le_bytes())?;
    for offset in partition_offsets {
        writer.write_all(&offset.to_le_bytes())?;
    }
    for block in &blocks {
        writer.write_all(&block.start.to_le_bytes())?;
        writer.write_all(&block.end.to_le_bytes())?;
        for value in block.min {
            writer.write_all(&value.to_le_bytes())?;
        }
        for value in block.max {
            writer.write_all(&value.to_le_bytes())?;
        }
    }
    for record in &records {
        for value in record.vector {
            writer.write_all(&value.to_le_bytes())?;
        }
    }
    for record in &records {
        writer.write_all(&[record.label])?;
    }
    writer.flush()?;

    println!(
        "preprocessor done - {} vectors and {} blocks written to {}",
        records.len(),
        blocks.len(),
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
            let mut vector = [0i16; VECTOR_DIMS];

            for (dim, value) in reference.vector.iter().enumerate() {
                let value = (value.clamp(-1.0, 1.0) * VECTOR_SCALE as f64).round() as i16;
                vector[dim] = value;
            }

            records.push(Record {
                partition: partition_key_from_quantized(&vector) as u32,
                sort_key: sort_key_from_quantized(&vector) as u32,
                label: reference.is_fraud as u8,
                vector,
            });
        }

        Ok(Collector { records })
    }
}

fn build_block(records: &[Record], start: usize, end: usize) -> Block {
    let mut min = [i16::MAX; VECTOR_DIMS];
    let mut max = [i16::MIN; VECTOR_DIMS];

    for record in &records[start..end] {
        for dim in 0..VECTOR_DIMS {
            min[dim] = min[dim].min(record.vector[dim]);
            max[dim] = max[dim].max(record.vector[dim]);
        }
    }

    Block {
        start: start as u32,
        end: end as u32,
        min,
        max,
    }
}
