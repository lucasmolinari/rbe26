mod handlers;
mod models;
mod vectorizer;

use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/ready", get(handlers::ready))
        .route("/fraud-score", post(handlers::fraud_score));

    let addr = SocketAddr::from(([0, 0, 0, 0], 9999));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
