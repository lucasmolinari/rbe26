use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;

use crate::buckets::{PARTITION_COUNT, VECTOR_BYTES, VECTOR_DIMS};

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
    pub vectors: Vec<i16>,
    pub residuals: Vec<u8>,
    pub labels: Vec<u8>,
    pub blocks: Vec<Block>,
    pub partition_bounds: Vec<Bounds>,
    pub non_empty_partitions: Vec<usize>,
    pub partition_offsets: Vec<u32>,
    pub vector_count: usize,
    pub vector_scale: f64,
    pub normalization: Normalization,
    pub mcc_risk: HashMap<String, f64>,
}

pub struct Block {
    pub start: u32,
    pub end: u32,
    pub min: [i16; VECTOR_DIMS],
    pub max: [i16; VECTOR_DIMS],
}

pub struct Bounds {
    pub min: [i16; VECTOR_DIMS],
    pub max: [i16; VECTOR_DIMS],
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
        let (vectors, residuals, labels, blocks, partition_offsets, vector_count, vector_scale) =
            load_compact_vectors(&vectors_path)?;
        let partition_bounds = build_partition_bounds(&blocks, &partition_offsets);
        let non_empty_partitions = build_non_empty_partitions(&partition_offsets);
        eprintln!(
            "block index loaded ({vector_count} points, {} blocks)",
            blocks.len()
        );

        Ok(Self {
            vectors,
            residuals,
            labels,
            blocks,
            partition_bounds,
            non_empty_partitions,
            partition_offsets,
            vector_count,
            vector_scale,
            normalization,
            mcc_risk,
        })
    }
}

type CompactVectors = (Vec<i16>, Vec<u8>, Vec<u8>, Vec<Block>, Vec<u32>, usize, f64);

fn load_compact_vectors(path: &str) -> Result<CompactVectors, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("failed to open {path}: {e}"))?;

    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|e| format!("failed to read {path} header: {e}"))?;

    let count = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
    let scale = f32::from_le_bytes(header[4..8].try_into().unwrap()) as f64;
    let block_count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    if count == 0 {
        return Err(format!("{path} has zero vectors"));
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(format!("{path} has invalid vector scale {scale}"));
    }

    let partition_offsets_len = (PARTITION_COUNT + 1) * std::mem::size_of::<u32>();
    let blocks_len = block_count * block_bytes();
    let residuals_len = count * VECTOR_DIMS;
    let expected_len =
        12 + partition_offsets_len + blocks_len + count * VECTOR_BYTES + residuals_len + count;
    let actual_len = file
        .metadata()
        .map_err(|e| format!("failed to stat {path}: {e}"))?
        .len() as usize;
    if actual_len != expected_len {
        return Err(format!(
            "{path} has {actual_len} bytes, expected {expected_len} for {count} vectors"
        ));
    }

    let mut partition_offset_bytes = vec![0u8; partition_offsets_len];
    file.read_exact(&mut partition_offset_bytes)
        .map_err(|e| format!("failed to read {path} partition offsets: {e}"))?;
    let partition_offsets = partition_offset_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    if partition_offsets.first() != Some(&0)
        || partition_offsets.last() != Some(&(block_count as u32))
    {
        return Err(format!("{path} has invalid partition offsets"));
    }

    let mut block_bytes = vec![0u8; blocks_len];
    file.read_exact(&mut block_bytes)
        .map_err(|e| format!("failed to read {path} block metadata: {e}"))?;
    let blocks = parse_blocks(&block_bytes, count)?;

    let mut vector_bytes = vec![0u8; count * VECTOR_BYTES];
    file.read_exact(&mut vector_bytes)
        .map_err(|e| format!("failed to read {path} vectors: {e}"))?;
    let vectors = vector_bytes
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    let mut residuals = vec![0u8; residuals_len];
    file.read_exact(&mut residuals)
        .map_err(|e| format!("failed to read {path} vector residuals: {e}"))?;

    let mut labels = vec![0u8; count];
    file.read_exact(&mut labels)
        .map_err(|e| format!("failed to read {path} labels: {e}"))?;

    Ok((
        vectors,
        residuals,
        labels,
        blocks,
        partition_offsets,
        count,
        scale,
    ))
}

fn block_bytes() -> usize {
    2 * std::mem::size_of::<u32>() + 2 * VECTOR_DIMS * std::mem::size_of::<i16>()
}

fn parse_blocks(bytes: &[u8], vector_count: usize) -> Result<Vec<Block>, String> {
    let mut blocks = Vec::with_capacity(bytes.len() / block_bytes());

    for bytes in bytes.chunks_exact(block_bytes()) {
        let start = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let end = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if start >= end || end as usize > vector_count {
            return Err("vectors.bin has invalid block bounds".to_string());
        }

        let mut min = [0i16; VECTOR_DIMS];
        let mut max = [0i16; VECTOR_DIMS];
        let mut offset = 8;
        for value in &mut min {
            *value = i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
            offset += 2;
        }
        for value in &mut max {
            *value = i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
            offset += 2;
        }

        blocks.push(Block {
            start,
            end,
            min,
            max,
        });
    }

    Ok(blocks)
}

fn build_partition_bounds(blocks: &[Block], partition_offsets: &[u32]) -> Vec<Bounds> {
    let mut bounds = Vec::with_capacity(PARTITION_COUNT);

    for partition in 0..PARTITION_COUNT {
        let start = partition_offsets[partition] as usize;
        let end = partition_offsets[partition + 1] as usize;

        if start == end {
            bounds.push(Bounds {
                min: [0; VECTOR_DIMS],
                max: [0; VECTOR_DIMS],
            });
            continue;
        }

        let mut min = [i16::MAX; VECTOR_DIMS];
        let mut max = [i16::MIN; VECTOR_DIMS];

        for block in &blocks[start..end] {
            for dim in 0..VECTOR_DIMS {
                min[dim] = min[dim].min(block.min[dim]);
                max[dim] = max[dim].max(block.max[dim]);
            }
        }

        bounds.push(Bounds { min, max });
    }

    bounds
}

fn build_non_empty_partitions(partition_offsets: &[u32]) -> Vec<usize> {
    let mut partitions = Vec::new();

    for partition in 0..PARTITION_COUNT {
        if partition_offsets[partition] != partition_offsets[partition + 1] {
            partitions.push(partition);
        }
    }

    partitions
}
