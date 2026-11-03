use std::{future::Future, process::ExitCode};

use tokio::net::TcpListener;

/// Fly's own Machines API port on its private network, and not the registry's 8080.
const DEFAULT_ADDR: &str = "127.0.0.1:4280";

#[tokio::main]
async fn main() -> ExitCode {
    // Listen for the stop signals before announcing the address, so a signal
    // sent as soon as the address is printed still stops the server cleanly.
    let shutdown = shutdown();
    let settings = match fly_mock::Settings::from_env() {
        Ok(settings) => settings,
        Err(message) => {
            eprintln!("fly-mock: {message}");
            return ExitCode::FAILURE;
        }
    };
    let addr = std::env::var("FLY_MOCK_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("fly-mock: cannot listen on {addr}: {err}");
            return ExitCode::FAILURE;
        }
    };
    // The bound address, so a port of 0 shows the port the OS picked.
    let bound = listener
        .local_addr()
        .map_or(addr, |bound| bound.to_string());
    eprintln!("fly-mock: listening on {bound}");
    // Returning, not `process::exit`, is what stops the family apps: it drops
    // the router, along with any clone a connection task still holds when the
    // runtime shuts down, and dropping the router kills the child processes.
    if let Err(err) = axum::serve(listener, fly_mock::app_with(settings))
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("fly-mock: server error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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
