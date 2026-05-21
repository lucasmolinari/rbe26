use flate2::read::GzDecoder;
use qdrant_client::{
    Qdrant,
    qdrant::{
        CreateCollectionBuilder, Distance, PointStruct, UpsertPointsBuilder, VectorParamsBuilder,
    },
};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::json;
use std::io::{BufWriter, Read, Write};

#[derive(Deserialize)]
struct Reference {
    pub vector: [f64; 14],
    #[serde(rename = "label", deserialize_with = "deserialize_label")]
    pub is_fraud: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // step 1: convert gz json to binary
    println!("converting references.json.gz to binary...");
    let file = std::fs::File::open("resources/references.json.gz")?;
    let decoder = GzDecoder::new(file);
    let reader = std::io::BufReader::new(decoder);
    let mut de = serde_json::Deserializer::from_reader(reader);

    let out = std::fs::File::create("resources/references.bin")?;
    let writer = BufWriter::new(out);

    struct RefProcessor<W> {
        writer: W,
        count: usize,
    }

    impl<'de, W: Write> Visitor<'de> for RefProcessor<W> {
        type Value = usize;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an array of references")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut writer = self.writer;
            let mut count = self.count;
            while let Some(reference) = seq.next_element::<Reference>()? {
                for val in &reference.vector {
                    let bytes = (*val as f32).to_le_bytes();
                    writer.write_all(&bytes).map_err(de::Error::custom)?;
                }
                writer
                    .write_all(&[reference.is_fraud as u8])
                    .map_err(de::Error::custom)?;
                count += 1;
            }
            writer.flush().map_err(de::Error::custom)?;
            Ok(count)
        }
    }

    let count = de
        .deserialize_seq(RefProcessor { writer, count: 0 })
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("converted {} records to binary", count);

    // step 2: seed qdrant
    println!("seeding qdrant...");
    let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://qdrant:6334".into());
    let client = Qdrant::from_url(&qdrant_url).build()?;

    if let Ok(info) = client.collection_info("references").await {
        let count = info.result.unwrap().points_count.unwrap_or(0);
        if count > 0 {
            println!("already seeded ({} points), skipping", count);
            return Ok(());
        }
    }

    if client.collection_info("references").await.is_err() {
        client
            .create_collection(
                CreateCollectionBuilder::new("references")
                    .vectors_config(VectorParamsBuilder::new(14, Distance::Euclid)),
            )
            .await?;
        println!("collection created");
    }

    // step 3: stream binary into qdrant
    let bin_file = std::fs::File::open("resources/references.bin")?;
    let mut reader = std::io::BufReader::new(bin_file);
    let mut batch: Vec<PointStruct> = Vec::with_capacity(10_000);
    let mut total = 0usize;
    let mut i = 0usize;

    loop {
        let mut record = [0u8; 57];
        match reader.read_exact(&mut record) {
            Ok(_) => {
                let mut vector = [0f32; 14];
                for (j, chunk) in record[..56].chunks(4).enumerate() {
                    vector[j] = f32::from_le_bytes(chunk.try_into().unwrap());
                }
                let is_fraud = record[56] == 1;

                batch.push(PointStruct::new(
                    i as u64,
                    vector.to_vec(),
                    json!({"is_fraud": is_fraud}).as_object().unwrap().clone(),
                ));
                i += 1;

                if batch.len() == 10_000 {
                    client
                        .upsert_points(UpsertPointsBuilder::new(
                            "references",
                            batch.drain(..).collect::<Vec<_>>(),
                        ))
                        .await?;
                    total += 10_000;
                    println!("seeded {} records", total);
                }
            }
            Err(_) => break,
        }
    }

    if !batch.is_empty() {
        let remaining = batch.len();
        client
            .upsert_points(UpsertPointsBuilder::new("references", batch))
            .await?;
        total += remaining;
    }

    println!("seeding complete — {} total records", total);
    Ok(())
}

fn deserialize_label<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s == "fraud")
}
