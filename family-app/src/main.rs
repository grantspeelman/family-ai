use std::path::Path;

use tokio::{net::TcpListener, signal};

const DEFAULT_ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let Some(data_dir) = std::env::var_os("FAMILY_APP_DATA_DIR").filter(|dir| !dir.is_empty())
    else {
        eprintln!(
            "family-app: FAMILY_APP_DATA_DIR is not set or empty; set it to the data volume's directory"
        );
        std::process::exit(1);
    };
    // Kept for the life of the process.
    let _db = match family_app::db::open(Path::new(&data_dir)).await {
        Ok(db) => db,
        Err(err) => {
            eprintln!("family-app: {err}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "family-app: database {}",
        Path::new(&data_dir).join(family_app::db::DB_FILE).display()
    );
    let addr = std::env::var("FAMILY_APP_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("family-app: cannot listen on {addr}: {err}");
            std::process::exit(1);
        }
    };
    eprintln!("family-app: listening on {addr}");
    if let Err(err) = axum::serve(listener, family_app::app())
        .with_graceful_shutdown(shutdown())
        .await
    {
        eprintln!("family-app: server error: {err}");
        std::process::exit(1);
    }
}

/// Resolves on Ctrl-C or SIGTERM, so a stop signal ends the server cleanly.
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
