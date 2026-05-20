use std::sync::Arc;

use crate::{
    models::{FraudResponse, TransactionRequest},
    resources::Resources,
    vectorizer::vectorize,
};
use axum::{Json, extract::State, http::StatusCode};

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(
    State(resources): State<Arc<Resources>>,
    Json(payload): Json<TransactionRequest>,
) -> Json<FraudResponse> {
    let vectorized = vectorize(&payload, &resources.normalization, &resources.mcc_risk);

    let mut distances: Vec<(f64, bool)> = resources
        .references
        .iter()
        .map(|r| (euclidean_distance(&vectorized, &r.vector), r.is_fraud))
        .collect();

    distances.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let fraud_count = distances
        .iter()
        .take(5)
        .filter(|(_, is_fraud)| *is_fraud)
        .count();

    let fraud_score = fraud_count as f64 / 5.0;

    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}

fn euclidean_distance(a: &[f64; 14], b: &[f64; 14]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
