use std::{future::Future, io, path::Path, process::ExitCode};

use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";

/// Every startup or server failure ends here: one stderr line, then status 1.
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("family-app: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Starts the server and runs it until Ctrl-C or SIGTERM. An error is the
/// message to log, without the `family-app: ` prefix.
async fn run() -> Result<(), String> {
    // Listen for the stop signals before announcing the address, so a signal
    // sent as soon as the address is printed still stops the server cleanly.
    let shutdown = shutdown().map_err(context("cannot listen for stop signals"))?;
    let data_dir = std::env::var_os("FAMILY_APP_DATA_DIR")
        .filter(|dir| !dir.is_empty())
        .ok_or("FAMILY_APP_DATA_DIR is not set or empty; set it to the data volume's directory")?;
    let data_dir = Path::new(&data_dir);
    // Kept for the life of the process, by the router's handlers.
    let db = family_app::db::open(data_dir)
        .await
        .map_err(|err| err.to_string())?;
    let db_file = data_dir.join(family_app::db::DB_FILE);
    eprintln!("family-app: database {}", db_file.display());
    let addr = std::env::var("FAMILY_APP_ADDR");
    let addr = addr.as_deref().unwrap_or(DEFAULT_ADDR);
    let listener = TcpListener::bind(addr)
        .await
        .map_err(context(&format!("cannot listen on {addr}")))?;
    // The bound address, so a port of 0 shows the port the OS picked.
    let bound = listener
        .local_addr()
        .map_err(context("cannot read the bound address"))?;
    eprintln!("family-app: listening on {bound}");
    axum::serve(listener, family_app::app(db))
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

/// Resolves on Ctrl-C (SIGINT) or SIGTERM, so a stop signal ends the server
/// cleanly. Both handlers are registered when this is called, not when the
/// future is first polled, and failing to register one is a startup error.
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
