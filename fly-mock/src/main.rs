use std::future::Future;

use tokio::net::TcpListener;

/// Fly's own Machines API port on its private network, and not the registry's 8080.
const DEFAULT_ADDR: &str = "127.0.0.1:4280";

#[tokio::main]
async fn main() {
    // Listen for the stop signals before announcing the address, so a signal
    // sent as soon as the address is printed still stops the server cleanly.
    let shutdown = shutdown();
    let addr = std::env::var("FLY_MOCK_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("fly-mock: cannot listen on {addr}: {err}");
            std::process::exit(1);
        }
    };
    // The bound address, so a port of 0 shows the port the OS picked.
    let bound = listener
        .local_addr()
        .map_or(addr, |bound| bound.to_string());
    eprintln!("fly-mock: listening on {bound}");
    if let Err(err) = axum::serve(listener, fly_mock::app())
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("fly-mock: server error: {err}");
        std::process::exit(1);
    }
}

/// Resolves on Ctrl-C or SIGTERM. The handlers are registered when this is
/// called, not when the future is first polled.
#[cfg(unix)]
fn shutdown() -> impl Future<Output = ()> {
    use tokio::signal::unix::SignalKind;
    let interrupt = on_signal(SignalKind::interrupt());
    let terminate = on_signal(SignalKind::terminate());
    async {
        tokio::select! {
            () = interrupt => {},
            () = terminate => {},
        }
    }
}

/// Resolves when `kind` arrives, or never if it cannot be listened for.
#[cfg(unix)]
fn on_signal(kind: tokio::signal::unix::SignalKind) -> impl Future<Output = ()> {
    let stream = tokio::signal::unix::signal(kind);
    async move {
        match stream {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    }
}

#[cfg(not(unix))]
fn shutdown() -> impl Future<Output = ()> {
    async {
        let _ = tokio::signal::ctrl_c().await;
    }
}
