use std::{future::Future, io, process::ExitCode};

use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};

/// Fly's own Machines API port on its private network, and not the registry's 8080.
const DEFAULT_ADDR: &str = "127.0.0.1:4280";

/// Every startup or server failure ends here: one stderr line, then status 1.
/// Returning, not `process::exit`, is what stops the family apps: it drops the
/// router, along with any clone a connection task still holds when the runtime
/// shuts down, and dropping the router kills the child processes.
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("fly-mock: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Starts the server and runs it until Ctrl-C or SIGTERM. An error is the
/// message to log, without the `fly-mock: ` prefix.
async fn run() -> Result<(), String> {
    // Listen for the stop signals before announcing the address, so a signal
    // sent as soon as the address is printed still stops the server cleanly.
    let shutdown = shutdown().map_err(context("cannot listen for stop signals"))?;
    let settings = fly_mock::Settings::from_env()?;
    let addr = std::env::var("FLY_MOCK_ADDR");
    let addr = addr.as_deref().unwrap_or(DEFAULT_ADDR);
    let listener = TcpListener::bind(addr)
        .await
        .map_err(context(&format!("cannot listen on {addr}")))?;
    // The bound address, so a port of 0 shows the port the OS picked.
    let bound = listener
        .local_addr()
        .map_err(context("cannot read the bound address"))?;
    eprintln!("fly-mock: listening on {bound}");
    axum::serve(listener, fly_mock::app_with(settings))
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(context("server error"))
}

/// Turns an I/O error into the message to log, saying what failed. Startup and
/// the server share it, so the failures no test can cause, such as a stop
/// signal that cannot be registered, take the same path as a taken address.
fn context(doing: &str) -> impl FnOnce(io::Error) -> String + '_ {
    move |err| format!("{doing}: {err}")
}

/// Resolves on Ctrl-C (SIGINT) or SIGTERM. Both handlers are registered when
/// this is called, not when the future is first polled, and failing to
/// register one is a startup error.
fn shutdown() -> io::Result<impl Future<Output = ()>> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
    })
}
