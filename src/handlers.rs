use std::{cell::RefCell, sync::Arc};

use crate::{
    buckets::{VECTOR_DIMS, partition_key_from_quantized},
    models::{FraudResponse, TransactionRequest},
    resources::{Block, Resources},
    vectorizer::vectorize,
};
use axum::{Json, extract::State, http::StatusCode};

const DIM_ORDER: [usize; 14] = [0, 2, 7, 8, 12, 3, 5, 6, 1, 4, 9, 10, 11, 13];
const CANDIDATE_COUNT: usize = 6;
const RERANK_RESOLUTION: i32 = 256;
const RERANK_OFFSET: i32 = 128;

thread_local! {
    static SEARCH_SCRATCH: RefCell<SearchScratch> = RefCell::new(SearchScratch::new());
}

struct SearchScratch {
    partitions: Vec<(u64, usize)>,
    blocks: Vec<(u64, usize)>,
}

impl SearchScratch {
    fn new() -> Self {
        Self {
            partitions: Vec::with_capacity(4096),
            blocks: Vec::with_capacity(256),
        }
    }
}

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(
    State(resources): State<Arc<Resources>>,
    Json(payload): Json<TransactionRequest>,
) -> Json<FraudResponse> {
    let vectorized = vectorize(&payload, &resources.normalization, &resources.mcc_risk);
    let query = quantize_query(&vectorized, resources.vector_scale);
    let exact_query = quantize_exact_query(&vectorized, resources.vector_scale);
    let neighbours = nearest_5(resources.as_ref(), &query, &exact_query);
    let fraud_count = count_labels(resources.as_ref(), &neighbours);

    let fraud_score = fraud_count as f64 / 5.0;

    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}

fn count_labels(resources: &Resources, neighbours: &[usize; 5]) -> usize {
    neighbours
        .iter()
        .filter(|&&id| resources.labels[id] == 1)
        .count()
}

fn quantize_query(vector: &[f64; 14], scale: f64) -> [i16; 14] {
    let mut quantized = [0i16; 14];
    for (out, value) in quantized.iter_mut().zip(vector) {
        *out = (value.clamp(-1.0, 1.0) * scale).round() as i16;
    }
    quantized
}

fn quantize_exact_query(vector: &[f64; 14], scale: f64) -> [i32; 14] {
    let mut quantized = [0i32; 14];
    let scale = scale * RERANK_RESOLUTION as f64;
    for (out, value) in quantized.iter_mut().zip(vector) {
        *out = (value.clamp(-1.0, 1.0) * scale).round() as i32;
    }
    quantized
}

fn nearest_5(resources: &Resources, query: &[i16; 14], exact_query: &[i32; 14]) -> [usize; 5] {
    let mut candidate_ids = [0usize; CANDIDATE_COUNT];
    let mut candidate_distances = [u64::MAX; CANDIDATE_COUNT];

    let partition = partition_key_from_quantized(query);
    SEARCH_SCRATCH.with_borrow_mut(|scratch| {
        scan_partition_blocks(
            resources,
            query,
            partition,
            &mut candidate_ids,
            &mut candidate_distances,
            scratch,
        );

        scan_other_partitions(
            resources,
            query,
            partition,
            &mut candidate_ids,
            &mut candidate_distances,
            scratch,
        );
    });

    rerank_candidates(resources, exact_query, &candidate_ids, &candidate_distances)
}

fn scan_partition_blocks(
    resources: &Resources,
    query: &[i16; 14],
    partition: usize,
    best_ids: &mut [usize; CANDIDATE_COUNT],
    best_distances: &mut [u64; CANDIDATE_COUNT],
    scratch: &mut SearchScratch,
) {
    let start = resources.partition_offsets[partition] as usize;
    let end = resources.partition_offsets[partition + 1] as usize;
    scan_block_range(
        resources,
        query,
        start,
        end,
        best_ids,
        best_distances,
        scratch,
    );
}

fn scan_other_partitions(
    resources: &Resources,
    query: &[i16; 14],
    partition: usize,
    best_ids: &mut [usize; CANDIDATE_COUNT],
    best_distances: &mut [u64; CANDIDATE_COUNT],
    scratch: &mut SearchScratch,
) {
    scratch.partitions.clear();

    for &other_partition in &resources.non_empty_partitions {
        if other_partition == partition {
            continue;
        }

        let bounds = &resources.partition_bounds[other_partition];
        if let Some(distance) = bbox_lower_bound(
            &bounds.min,
            &bounds.max,
            query,
            best_distances[CANDIDATE_COUNT - 1],
        ) {
            scratch.partitions.push((distance, other_partition));
        }
    }

    scratch
        .partitions
        .sort_unstable_by_key(|&(distance, _)| distance);

    let mut partition_index = 0;
    while partition_index < scratch.partitions.len() {
        let (distance, other_partition) = scratch.partitions[partition_index];
        partition_index += 1;

        if distance >= best_distances[CANDIDATE_COUNT - 1] {
            break;
        }

        let start = resources.partition_offsets[other_partition] as usize;
        let end = resources.partition_offsets[other_partition + 1] as usize;
        scan_block_range(
            resources,
            query,
            start,
            end,
            best_ids,
            best_distances,
            scratch,
        );
    }
}

fn scan_block_range(
    resources: &Resources,
    query: &[i16; 14],
    start: usize,
    end: usize,
    best_ids: &mut [usize; CANDIDATE_COUNT],
    best_distances: &mut [u64; CANDIDATE_COUNT],
    scratch: &mut SearchScratch,
) {
    scratch.blocks.clear();

    for block_id in start..end {
        let block = &resources.blocks[block_id];
        if let Some(distance) = bbox_lower_bound(
            &block.min,
            &block.max,
            query,
            best_distances[CANDIDATE_COUNT - 1],
        ) {
            scratch.blocks.push((distance, block_id));
        }
    }

    scratch
        .blocks
        .sort_unstable_by_key(|&(distance, _)| distance);

    for &(distance, block_id) in &scratch.blocks {
        if distance >= best_distances[CANDIDATE_COUNT - 1] {
            break;
        }

        scan_block(
            resources,
            query,
            &resources.blocks[block_id],
            best_ids,
            best_distances,
        );
    }
}

fn bbox_lower_bound(
    min: &[i16; 14],
    max: &[i16; 14],
    query: &[i16; 14],
    limit: u64,
) -> Option<u64> {
    let mut distance = 0u64;

    for dim in DIM_ORDER {
        let query_value = query[dim] as i64;
        let lower = min[dim] as i64;
        let upper = max[dim] as i64;
        let delta = if query_value < lower {
            lower - query_value
        } else if query_value > upper {
            query_value - upper
        } else {
            0
        };
        distance += (delta * delta) as u64;
        if distance >= limit {
            return None;
        }
    }

    Some(distance)
}

fn scan_block(
    resources: &Resources,
    query: &[i16; 14],
    block: &Block,
    best_ids: &mut [usize; CANDIDATE_COUNT],
    best_distances: &mut [u64; CANDIDATE_COUNT],
) {
    for id in block.start as usize..block.end as usize {
        let offset = id * VECTOR_DIMS;
        let mut distance = 0u64;
        let limit = best_distances[CANDIDATE_COUNT - 1];

        for dim in DIM_ORDER {
            let value = resources.vectors[offset + dim];
            let delta = value as i64 - query[dim] as i64;
            distance += (delta * delta) as u64;
            if distance >= limit {
                break;
            }
        }

        if distance < best_distances[CANDIDATE_COUNT - 1] {
            insert_candidate(id, distance, best_ids, best_distances);
        }
    }
}

fn rerank_candidates(
    resources: &Resources,
    query: &[i32; 14],
    candidate_ids: &[usize; CANDIDATE_COUNT],
    candidate_distances: &[u64; CANDIDATE_COUNT],
) -> [usize; 5] {
    let mut best_ids = [0usize; 5];
    let mut best_distances = [u64::MAX; 5];

    for (&id, &candidate_distance) in candidate_ids.iter().zip(candidate_distances) {
        if candidate_distance == u64::MAX {
            continue;
        }

        let offset = id * VECTOR_DIMS;
        let mut distance = 0u64;
        let limit = best_distances[4];

        for dim in DIM_ORDER {
            let value = exact_vector_value(resources, offset + dim);
            let delta = value as i64 - query[dim] as i64;
            distance += (delta * delta) as u64;
            if distance >= limit {
                break;
            }
        }

        if distance < best_distances[4] {
            insert_best(id, distance, &mut best_ids, &mut best_distances);
        }
    }

    best_ids
}

fn exact_vector_value(resources: &Resources, offset: usize) -> i32 {
    resources.vectors[offset] as i32 * RERANK_RESOLUTION + resources.residuals[offset] as i32
        - RERANK_OFFSET
}

fn insert_candidate(
    id: usize,
    distance: u64,
    best_ids: &mut [usize; CANDIDATE_COUNT],
    best_distances: &mut [u64; CANDIDATE_COUNT],
) {
    let mut pos = CANDIDATE_COUNT - 1;
    while pos > 0 && distance < best_distances[pos - 1] {
        best_distances[pos] = best_distances[pos - 1];
        best_ids[pos] = best_ids[pos - 1];
        pos -= 1;
    }
    best_distances[pos] = distance;
    best_ids[pos] = id;
}

fn insert_best(id: usize, distance: u64, best_ids: &mut [usize; 5], best_distances: &mut [u64; 5]) {
    let mut pos = 4;
    while pos > 0 && distance < best_distances[pos - 1] {
        best_distances[pos] = best_distances[pos - 1];
        best_ids[pos] = best_ids[pos - 1];
        pos -= 1;
    }
    best_distances[pos] = distance;
    best_ids[pos] = id;
}
