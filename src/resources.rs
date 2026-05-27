use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;

use crate::buckets::{BUCKET_COUNT, VECTOR_BYTES};

#[derive(Deserialize, Debug)]
pub struct Normalization {
    pub max_amount: f64,
    pub max_installments: f64,
    pub amount_vs_avg_ratio: f64,
    pub max_minutes: f64,
    pub max_km: f64,
    pub max_tx_count_24h: f64,
    pub max_merchant_avg_amount: f64,
}

pub struct Resources {
    pub vectors: Vec<u8>,
    pub labels: Vec<u8>,
    pub bucket_offsets: Vec<u32>,
    pub vector_count: usize,
    pub vector_scale: f64,
    pub normalization: Normalization,
    pub mcc_risk: HashMap<String, f64>,
}

impl std::fmt::Debug for Resources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resources")
            .field("vector_count", &self.vector_count)
            .field("normalization", &self.normalization)
            .field("mcc_risk", &self.mcc_risk)
            .finish_non_exhaustive()
    }
}

impl Resources {
    pub fn new() -> Result<Self, String> {
        let normalization = std::fs::read_to_string("resources/normalization.json")
            .map_err(|e| format!("failed to read normalization.json: {}", e))?;
        let normalization: Normalization = serde_json::from_str(&normalization)
            .map_err(|e| format!("failed to parse normalization.json: {}", e))?;

        let mcc_risk = std::fs::read_to_string("resources/mcc_risk.json")
            .map_err(|e| format!("failed to read mcc_risk.json: {}", e))?;
        let mcc_risk: HashMap<String, f64> = serde_json::from_str(&mcc_risk)
            .map_err(|e| format!("failed to parse mcc_risk.json: {}", e))?;

        let vectors_path =
            std::env::var("VECTORS_PATH").unwrap_or_else(|_| "resources/vectors.bin".to_string());
        eprintln!("loading bucketed vectors from {vectors_path}");
        let (vectors, labels, bucket_offsets, vector_count, vector_scale) =
            load_compact_vectors(&vectors_path)?;
        eprintln!("bucketed vectors loaded ({vector_count} points)");

        Ok(Self {
            vectors,
            labels,
            bucket_offsets,
            vector_count,
            vector_scale,
            normalization,
            mcc_risk,
        })
    }
}

type CompactVectors = (Vec<u8>, Vec<u8>, Vec<u32>, usize, f64);

fn load_compact_vectors(path: &str) -> Result<CompactVectors, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("failed to open {path}: {e}"))?;

    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|e| format!("failed to read {path} header: {e}"))?;

    let count = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
    let scale = f32::from_le_bytes(header[4..8].try_into().unwrap()) as f64;
    let bucket_count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    if count == 0 {
        return Err(format!("{path} has zero vectors"));
    }
    if bucket_count != BUCKET_COUNT {
        return Err(format!(
            "{path} has {bucket_count} buckets, expected {BUCKET_COUNT}"
        ));
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(format!("{path} has invalid vector scale {scale}"));
    }

    let offsets_len = (bucket_count + 1) * std::mem::size_of::<u32>();
    let expected_len = 12 + offsets_len + count * VECTOR_BYTES + count;
    let actual_len = file
        .metadata()
        .map_err(|e| format!("failed to stat {path}: {e}"))?
        .len() as usize;
    if actual_len != expected_len {
        return Err(format!(
            "{path} has {actual_len} bytes, expected {expected_len} for {count} vectors"
        ));
    }

    let mut offset_bytes = vec![0u8; offsets_len];
    file.read_exact(&mut offset_bytes)
        .map_err(|e| format!("failed to read {path} bucket offsets: {e}"))?;
    let bucket_offsets = offset_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    if bucket_offsets.first() != Some(&0) || bucket_offsets.last() != Some(&(count as u32)) {
        return Err(format!("{path} has invalid bucket offsets"));
    }

    let mut vectors = vec![0u8; count * VECTOR_BYTES];
    file.read_exact(&mut vectors)
        .map_err(|e| format!("failed to read {path} vectors: {e}"))?;

    let mut labels = vec![0u8; count];
    file.read_exact(&mut labels)
        .map_err(|e| format!("failed to read {path} labels: {e}"))?;

    Ok((vectors, labels, bucket_offsets, count, scale))
}
