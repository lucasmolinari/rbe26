use axum::{
    Router,
    routing::{get, post},
};
use rbe26::{handlers, resources};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = resources::Resources::new()?;
    let resources = Arc::new(resources);

    let app = Router::new().route("/ready", get(handlers::ready)).route(
        "/fraud-score",
        post(handlers::fraud_score).with_state(resources),
    );

    let addr = std::env::var("ADDR").unwrap_or_else(|_| "0.0.0.0:9999".into());
    eprintln!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for terminate signal")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
