use registry::{
    accounts::{AccountStore, db_path_from},
    fly::{FlyClient, FlyConfig},
    provisioning::{Provisioner, machine_port_from},
};
use tokio::{net::TcpListener, signal};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";
const FLY_HOSTNAME_VAR: &str = "FLY_API_HOSTNAME";

#[tokio::main]
async fn main() {
    let provisioner = provisioner_from_env().unwrap_or_else(|err| {
        eprintln!("registry: {err}");
        std::process::exit(1);
    });
    if provisioner.is_none() {
        eprintln!(
            "registry: {FLY_HOSTNAME_VAR} is not set, so provisioning is disabled \
             and createAccount returns PROVISIONING_UNAVAILABLE"
        );
    }
    let db_path = match db_path_from(|name| std::env::var_os(name)) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("registry: {err}");
            std::process::exit(1);
        }
    };
    let store = match AccountStore::open(&db_path).await {
        Ok(store) => store,
        Err(err) => {
            eprintln!("registry: cannot open {}: {err}", db_path.display());
            std::process::exit(1);
        }
    };
    match store.fail_interrupted().await {
        Ok(0) => {}
        Ok(count) => eprintln!("registry: marked {count} interrupted provisioning(s) as failed"),
        Err(err) => {
            eprintln!("registry: cannot mark interrupted provisioning as failed: {err}");
            std::process::exit(1);
        }
    }
    let addr = std::env::var("REGISTRY_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("registry: cannot listen on {addr}: {err}");
            std::process::exit(1);
        }
    };
    match listener.local_addr() {
        Ok(local) => eprintln!("registry: listening on {local}"),
        Err(_) => eprintln!("registry: listening on {addr}"),
    }
    if let Err(err) = axum::serve(listener, registry::app(store, provisioner))
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("registry: server error: {err}");
        std::process::exit(1);
    }
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
