//! Starting and stopping the registry binary: serving, stop signals and the
//! startup failures that exit with status 1.

use std::{net::TcpListener, path::Path, process::Output};

use registry::accounts::AccountStore;

mod common;

use common::{REGISTRY_STOP_TIMEOUT, Running, registry_command};

fn run_registry(db_path: Option<&Path>) -> Output {
    let mut command = registry_command();
    match db_path {
        Some(path) => command.env("REGISTRY_DB_PATH", path),
        None => command.env_remove("REGISTRY_DB_PATH"),
    };
    command.output().expect("run the registry binary")
}

fn assert_clean_failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!output.status.success(), "should fail; stderr: {stderr}");
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "panicked: {stderr}");
    stderr
}

fn serving(name: &str) -> Running {
    let mut command = registry_command();
    command.env("REGISTRY_DB_PATH", common::fresh_db(name));
    Running::start(command)
}

/// Sends `signal` and checks that the registry exits with status 0 in time.
fn assert_stops_cleanly_on(mut running: Running, signal: &str) {
    running.signal(signal);
    let status = running
        .wait_timeout(REGISTRY_STOP_TIMEOUT)
        .unwrap_or_else(|| panic!("the registry did not stop on SIG{signal}"));
    assert!(status.success(), "{status}; stderr: {}", running.stderr());
}

#[tokio::test]
async fn serves_healthz_on_the_bound_port_and_stops_cleanly_on_sigterm() {
    let running = serving("startup-sigterm");

    let port = running.base_url.rsplit(':').next().unwrap();
    assert_ne!(port, "0", "the log shows the requested port");
    assert!(running.base_url.starts_with("http://127.0.0.1:"));
    let response = reqwest::get(format!("{}/healthz", running.base_url))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "ok");

    assert_stops_cleanly_on(running, "TERM");
}

#[test]
fn stops_cleanly_on_ctrl_c() {
    assert_stops_cleanly_on(serving("startup-sigint"), "INT");
}

#[test]
fn exits_with_an_error_naming_the_address_when_it_is_in_use() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap().to_string();

    let output = registry_command()
        .env("REGISTRY_ADDR", &addr)
        .env("REGISTRY_DB_PATH", common::fresh_db("startup-in-use"))
        .output()
        .unwrap();

    let stderr = assert_clean_failure(&output);
    assert!(
        stderr.contains(&format!("cannot listen on {addr}")),
        "stderr: {stderr}"
    );
}

#[test]
fn exits_with_an_error_naming_the_variable_when_the_path_is_unset() {
    let output = run_registry(None);

    let stderr = assert_clean_failure(&output);
    assert!(stderr.contains("REGISTRY_DB_PATH"), "stderr: {stderr}");
}

#[test]
fn exits_with_an_error_when_the_store_cannot_be_opened() {
    let dir = common::fresh_db("startup-missing-dir");
    let path = dir.join("no-such-dir").join("registry.db");

    let output = run_registry(Some(&path));

    let stderr = assert_clean_failure(&output);
    assert!(stderr.contains("database"), "stderr: {stderr}");
}

#[tokio::test]
async fn exits_with_an_error_when_interrupted_accounts_cannot_be_marked_failed() {
    let path = common::fresh_db("startup-cannot-mark");
    let store = AccountStore::open(&path).await.unwrap();
    store.create("old@example.com", "Old").await.unwrap();
    drop(store);
    common::execute_sql(&path, common::BLOCK_ACCOUNT_UPDATES).await;

    let output = run_registry(Some(&path));

    let stderr = assert_clean_failure(&output);
    assert!(
        stderr.contains("cannot mark interrupted provisioning as failed"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("old@example.com"), "stderr: {stderr}");
    assert!(!stderr.contains("listening"), "stderr: {stderr}");
}
