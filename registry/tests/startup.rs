//! Starting the registry binary without a usable database.

use std::process::{Command, Output};

mod common;

fn run_registry(db_path: Option<&std::path::Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_registry"));
    // Port 0: should startup get past the store, it must not clash with anything.
    command.env("REGISTRY_ADDR", "127.0.0.1:0");
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
