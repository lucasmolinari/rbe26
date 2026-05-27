use std::sync::Arc;

use crate::{
    buckets::{
        AMOUNT_BINS, HOME_BINS, HOUR_BINS, MCC_BINS, RATIO_BINS, TX_BINS, VECTOR_BYTES, bucket_key,
        bucket_key_from_quantized, decode_bucket_key,
    },
    models::{FraudResponse, TransactionRequest},
    resources::Resources,
    vectorizer::vectorize,
};
use axum::{Json, extract::State, http::StatusCode};

const MIN_CANDIDATES: usize = 512;
const MAX_RADIUS: usize = 5;

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(
    State(resources): State<Arc<Resources>>,
    Json(payload): Json<TransactionRequest>,
) -> Json<FraudResponse> {
    let vectorized = vectorize(&payload, &resources.normalization, &resources.mcc_risk);
    let query = quantize_query(&vectorized, resources.vector_scale);
    let neighbours = nearest_5(resources.as_ref(), &query);

    let fraud_count = neighbours
        .iter()
        .filter(|&&id| resources.labels[id] == 1)
        .count() as f64;

    let fraud_score = fraud_count / 5.0;

    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}

fn quantize_query(vector: &[f64; 14], scale: f64) -> [i16; 14] {
    let mut quantized = [0i16; 14];
    for (out, value) in quantized.iter_mut().zip(vector) {
        *out = (value.clamp(-1.0, 1.0) * scale).round() as i16;
    }
    quantized
}

fn nearest_5(resources: &Resources, query: &[i16; 14]) -> [usize; 5] {
    let mut best_ids = [0usize; 5];
    let mut best_distances = [u64::MAX; 5];
    let mut candidates = 0usize;
    let center = decode_bucket_key(bucket_key_from_quantized(query));

    for radius in 0..=MAX_RADIUS {
        for amount in bounded_range(center[0], radius, AMOUNT_BINS) {
            for ratio in bounded_range(center[1], radius, RATIO_BINS) {
                for hour in bounded_range(center[2], radius.min(1), HOUR_BINS) {
                    for home in bounded_range(center[3], radius, HOME_BINS) {
                        for tx in bounded_range(center[4], radius, TX_BINS) {
                            for mcc in bounded_range(center[5], radius.min(1), MCC_BINS) {
                                if radius > 0 {
                                    let ring = amount
                                        .abs_diff(center[0])
                                        .max(ratio.abs_diff(center[1]))
                                        .max(home.abs_diff(center[3]))
                                        .max(tx.abs_diff(center[4]));
                                    if ring != radius {
                                        continue;
                                    }
                                }

                                let key = bucket_key(
                                    amount, ratio, hour, home, tx, mcc, center[6], center[7],
                                    center[8], center[9],
                                );
                                let start = resources.bucket_offsets[key] as usize;
                                let end = resources.bucket_offsets[key + 1] as usize;
                                candidates += end - start;
                                scan_bucket(
                                    resources,
                                    query,
                                    start,
                                    end,
                                    &mut best_ids,
                                    &mut best_distances,
                                );
                            }
                        }
                    }
                }
            }
        }

        if candidates >= MIN_CANDIDATES && best_distances[4] != u64::MAX {
            break;
        }
    }

    if best_distances[4] == u64::MAX {
        scan_bucket(
            resources,
            query,
            0,
            resources.vector_count.min(2048),
            &mut best_ids,
            &mut best_distances,
        );
    }

    best_ids
}

fn scan_bucket(
    resources: &Resources,
    query: &[i16; 14],
    start: usize,
    end: usize,
    best_ids: &mut [usize; 5],
    best_distances: &mut [u64; 5],
) {
    for id in start..end {
        let offset = id * VECTOR_BYTES;
        let mut distance = 0u64;

        for (dim, query_value) in query.iter().enumerate() {
            let i = offset + dim * 2;
            let value = i16::from_le_bytes([resources.vectors[i], resources.vectors[i + 1]]);
            let delta = value as i32 - *query_value as i32;
            distance += (delta * delta) as u64;
        }

        if distance < best_distances[4] {
            insert_best(id, distance, best_ids, best_distances);
        }
    }
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

fn bounded_range(center: usize, radius: usize, bins: usize) -> std::ops::RangeInclusive<usize> {
    center.saturating_sub(radius)..=center.saturating_add(radius).min(bins - 1)
}
