//! The family app's process gets only the environment it needs. The script here
//! is written and then run, so only one test in this file may start a process:
//! another thread forking at that moment could make the exec fail.
#![cfg(unix)]

mod common;

use std::{
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
};

use common::{TempDir, json_body, raw_http};
use serde_json::json;

const LISTENING: &str = "fly-mock: listening on ";

/// Stops the mock even when an assertion fails.
struct Mock(Child);

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn post(addr: &str, path: &str, body: &serde_json::Value) -> serde_json::Value {
    json_body(&raw_http(addr, "POST", path, &body.to_string()))
}

#[test]
fn child_gets_only_the_family_app_variables_and_the_allowlist() {
    let root = TempDir::new();
    let script = root.path().join("record-env.sh");
    let recorded = root.path().join("env");
    // `export` is a shell builtin, so this needs no PATH. The script exits, so
    // create-Machine fails, but only after the environment is on disk.
    std::fs::write(
        &script,
        format!("#!/bin/sh\nexport -p > '{}'\n", recorded.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Keep any profile path the coverage job set, so this run still counts.
    let profile = std::env::var_os("LLVM_PROFILE_FILE")
        .unwrap_or_else(|| root.path().join("unused.profraw").into());
    let mut child = Command::new(env!("CARGO_BIN_EXE_fly-mock"))
        .env("FLY_MOCK_ADDR", "127.0.0.1:0")
        .env("FLY_MOCK_FAMILY_APP_BIN", &script)
        .env("FLY_MOCK_DATA_DIR", root.path())
        .env("UNRELATED_VARIABLE", "must-not-reach-the-child")
        .env("RUST_LOG", "debug")
        .env("RUST_BACKTRACE", "1")
        .env("LLVM_PROFILE_FILE", profile)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start fly-mock");
    let stderr = child.stderr.take().unwrap();
    let _mock = Mock(child);
    let mut line = String::new();
    BufReader::new(stderr).read_line(&mut line).unwrap();
    let addr = line
        .trim()
        .strip_prefix(LISTENING)
        .unwrap_or_else(|| panic!("fly-mock did not start: {line:?}"));

    post(
        addr,
        "/v1/apps",
        &json!({"name": "fam", "org_slug": "personal"}),
    );
    let volume = post(
        addr,
        "/v1/apps/fam/volumes",
        &json!({"name": "data", "region": "jnb", "size_gb": 1}),
    );
    let body = common::machine_body(volume["id"].as_str().unwrap());
    let response = raw_http(addr, "POST", "/v1/apps/fam/machines", &body.to_string());
    assert!(response.starts_with("HTTP/1.1 500 "), "{response}");

    let env = std::fs::read_to_string(&recorded).expect("the script ran");
    for name in [
        "FAMILY_APP_ADDR",
        "FAMILY_APP_DATA_DIR",
        "RUST_BACKTRACE",
        "LLVM_PROFILE_FILE",
    ] {
        assert!(env.contains(name), "{name} did not reach the child");
    }
    for name in [
        "UNRELATED_VARIABLE",
        "RUST_LOG",
        "FLY_MOCK_FAMILY_APP_BIN",
        "HOME",
    ] {
        assert!(!env.contains(name), "{name} reached the child");
    }
}
