use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "0.0.0.0:9999".into());
    let upstreams = std::env::var("UPSTREAMS").unwrap_or_else(|_| "api1:9999,api2:9999".into());
    let upstreams = upstreams
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if upstreams.is_empty() {
        return Err("UPSTREAMS is empty".into());
    }

    let listener = TcpListener::bind(&addr).await?;
    let upstreams = Arc::new(upstreams);
    let next = Arc::new(AtomicUsize::new(0));

    eprintln!("listening on {addr}, proxying to {upstreams:?}");

    loop {
        let (client, _) = listener.accept().await?;
        let upstreams = Arc::clone(&upstreams);
        let next = Arc::clone(&next);

        tokio::spawn(async move {
            if let Err(err) = proxy(client, upstreams, next).await {
                eprintln!("proxy error: {err}");
            }
        });
    }
}

async fn proxy(
    mut client: TcpStream,
    upstreams: Arc<Vec<String>>,
    next: Arc<AtomicUsize>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.set_nodelay(true)?;

    let start = next.fetch_add(1, Ordering::Relaxed);
    let mut last_error = None;

    for offset in 0..upstreams.len() {
        let upstream = &upstreams[(start + offset) % upstreams.len()];
        match TcpStream::connect(upstream).await {
            Ok(mut server) => {
                server.set_nodelay(true)?;
                let _ = copy_bidirectional(&mut client, &mut server).await?;
                return Ok(());
            }
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error
        .map(|err| format!("all upstreams failed: {err}"))
        .unwrap_or_else(|| "all upstreams failed".to_string())
        .into())
}
