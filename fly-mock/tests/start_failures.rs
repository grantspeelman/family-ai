//! Failures around a process that never gets healthy. The scripts here are
//! written and then run, so only one test in this file may start a process:
//! another thread forking at that moment could make the exec fail.
#![cfg(unix)]

mod common;

use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};

use axum::http::StatusCode;
use common::{TempDir, app_with_volume, machine_body, post, settings, shell_kill};
use fly_mock::app_with;
use serde_json::json;

const MACHINES: &str = "/v1/apps/fam/machines";

/// Writes an executable script that records its process id in `pid`, then waits.
fn hanging_script(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let script = dir.join("hang.sh");
    let pid = dir.join("pid");
    let text = format!("#!/bin/sh\necho $$ > '{}'\nexec sleep 60\n", pid.display());
    std::fs::write(&script, text).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, pid)
}

#[tokio::test]
async fn time_limit_stops_the_process_and_returns_500() {
    let root = TempDir::new();
    let (script, pid_file) = hanging_script(root.path());
    let mut settings = settings(&script, root.path());
    settings.start_timeout = Duration::from_millis(1500);
    let app = app_with(settings);
    let volume = app_with_volume(&app).await;

    let start = Instant::now();
    let (status, body) = post(&app, MACHINES, machine_body(&volume)).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        body["error"].as_str().unwrap().contains("not healthy"),
        "{body}"
    );
    assert!(start.elapsed() >= Duration::from_millis(1500));
    assert!(start.elapsed() < Duration::from_secs(8));
    let pid = std::fs::read_to_string(pid_file).expect("the script ran");
    let alive = shell_kill(&["-0", pid.trim()]);
    assert!(!alive, "process {} is still running", pid.trim());
}

#[tokio::test]
async fn machine_with_no_mounts_returns_500_and_starts_nothing() {
    let root = TempDir::new();
    let marker = root.path().join("ran");
    let script = root.path().join("mark.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let app = app_with(settings(&script, root.path()));
    app_with_volume(&app).await;

    let (status, body) = post(&app, MACHINES, json!({"config": {"image": "img"}})).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("mount"), "{body}");
    assert!(!marker.exists(), "the binary was started");
}
