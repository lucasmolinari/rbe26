use std::sync::Arc;

use crate::{
    models::{FraudResponse, TransactionRequest},
    resources::Resources,
    vectorizer::vectorize,
};
use axum::{Json, extract::State, http::StatusCode};
use qdrant_client::qdrant::SearchPointsBuilder;

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(
    State(resources): State<Arc<Resources>>,
    Json(payload): Json<TransactionRequest>,
) -> Json<FraudResponse> {
    let vectorized = vectorize(&payload, &resources.normalization, &resources.mcc_risk);

    let search_result = resources
        .qdrant_client
        .search_points(
            SearchPointsBuilder::new("references", vectorized.iter().map(|&x| x as f32).collect::<Vec<f32>>(), 5)
                .with_payload(true),
        )
        .await;

    let fraud_count = match search_result {
        Ok(result) => result
            .result
            .iter()
            .filter(|hit| {
                hit.payload
                    .get("is_fraud")
                    .and_then(|v| match v.kind {
                        Some(qdrant_client::qdrant::value::Kind::BoolValue(b)) => Some(b),
                        _ => None,
                    })
                    .unwrap_or(false)
            })
            .count() as f64,
        Err(_) => 0.0,
    };

    let fraud_score = fraud_count / 5.0;

    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}
