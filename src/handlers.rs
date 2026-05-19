use crate::models::{FraudResponse, TransactionRequest};
use axum::{Json, http::StatusCode};

pub async fn ready() -> StatusCode {
    StatusCode::OK
}

pub async fn fraud_score(Json(_payload): Json<TransactionRequest>) -> Json<FraudResponse> {
    let fraud_score = 0.0;
    Json(FraudResponse {
        approved: fraud_score < 0.6,
        fraud_score,
    })
}
