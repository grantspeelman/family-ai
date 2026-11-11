use std::{future::Future, io, process::ExitCode};

use registry::{
    accounts::{AccountStore, db_path_from},
    fly::{FlyClient, FlyConfig},
    provisioning::{Provisioner, machine_port_from},
};
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";
const FLY_HOSTNAME_VAR: &str = "FLY_API_HOSTNAME";

/// Every startup or server failure ends here: one stderr line, then status 1.
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("registry: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Starts the server and runs it until Ctrl-C or SIGTERM. An error is the
/// message to log, without the `registry: ` prefix.
async fn run() -> Result<(), String> {
    // Listen for the stop signals before announcing the address, so a signal
    // sent as soon as the address is printed still stops the server cleanly.
    let shutdown = shutdown().map_err(context("cannot listen for stop signals"))?;
    let provisioner = provisioner_from_env()?;
    if provisioner.is_none() {
        eprintln!(
            "registry: {FLY_HOSTNAME_VAR} is not set, so provisioning is disabled \
             and createAccount returns PROVISIONING_UNAVAILABLE"
        );
    }
    let db_path = db_path_from(|name| std::env::var_os(name)).map_err(|err| err.to_string())?;
    let store = AccountStore::open(&db_path)
        .await
        .map_err(|err| format!("cannot open {}: {err}", db_path.display()))?;
    let interrupted = store
        .fail_interrupted()
        .await
        .map_err(|err| format!("cannot mark interrupted provisioning as failed: {err}"))?;
    if interrupted > 0 {
        eprintln!("registry: marked {interrupted} interrupted provisioning(s) as failed");
    }
    let addr = std::env::var("REGISTRY_ADDR");
    let addr = addr.as_deref().unwrap_or(DEFAULT_ADDR);
    let listener = TcpListener::bind(addr)
        .await
        .map_err(context(&format!("cannot listen on {addr}")))?;
    // The bound address, so a port of 0 shows the port the OS picked.
    let bound = listener
        .local_addr()
        .map_err(context("cannot read the bound address"))?;
    eprintln!("registry: listening on {bound}");
    axum::serve(listener, registry::app(store, provisioner))
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(context("server error"))
}

/// Reads the Fly and machine port config. `Ok(None)` when `FLY_API_HOSTNAME`
/// is unset or empty. Errors name the variable and never contain the token.
fn provisioner_from_env() -> Result<Option<Provisioner>, String> {
    let machine_port = machine_port_from(|name| std::env::var_os(name))?;
    if std::env::var_os(FLY_HOSTNAME_VAR).is_none_or(|value| value.is_empty()) {
        return Ok(None);
    }
    let config = FlyConfig::from_env().map_err(|err| err.to_string())?;
    let client = FlyClient::new(config).map_err(|err| err.to_string())?;
    Ok(Some(Provisioner::new(client, machine_port)))
}

/// Turns an I/O error into the message to log, saying what failed. Startup and
/// the server share it, so the failures no test can cause, such as a stop
/// signal that cannot be registered, take the same path as a taken address.
fn context(doing: &str) -> impl FnOnce(io::Error) -> String + '_ {
    move |err| format!("{doing}: {err}")
}

/// Resolves on Ctrl-C (SIGINT) or SIGTERM, so `docker stop` ends the server
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
