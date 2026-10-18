use tokio::{net::TcpListener, signal};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let addr = std::env::var("REGISTRY_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("registry: cannot listen on {addr}: {err}");
            std::process::exit(1);
        }
    };
    eprintln!("registry: listening on {addr}");
    if let Err(err) = axum::serve(listener, registry::app())
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("registry: server error: {err}");
        std::process::exit(1);
    }
}

/// Resolves on Ctrl-C or SIGTERM, so `docker stop` ends the server cleanly.
async fn shutdown() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
