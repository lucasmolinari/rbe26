use std::sync::Arc;

use crate::{
    models::{FraudResponse, TransactionRequest},
    resources::Resources,
};
use axum::{Json, extract::State, http::StatusCode};

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(
    State(_): State<Arc<Resources>>,
    Json(_payload): Json<TransactionRequest>,
) -> Json<FraudResponse> {
    let fraud_score = 0.0;
    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}
