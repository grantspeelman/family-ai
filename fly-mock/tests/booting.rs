//! In-process tests of create-Machine: requests go straight into the router,
//! which starts the real family app as a child process.

mod common;

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use axum::http::StatusCode;
use common::{
    MACHINE_PORT, TempDir, app_with_volume, assert_healthy, assert_private_loopback,
    assert_stops_accepting, family_app_bin, machine_addr, machine_body, post, settings,
};
use fly_mock::app_with;
use serde_json::{Value, json};

const MACHINES: &str = "/v1/apps/fam/machines";

fn assert_fly_error(body: &Value) {
    assert!(!body["error"].as_str().expect("error string").is_empty());
}

#[tokio::test]
async fn machine_is_a_running_family_app_on_the_mapped_volume() {
    let root = TempDir::new();
    let app = app_with(settings(family_app_bin(), root.path()));
    let volume = app_with_volume(&app).await;
    let (status, machine) = post(&app, MACHINES, machine_body(&volume)).await;
    assert_eq!(status, StatusCode::OK, "{machine}");
    assert_eq!(machine["state"], "started");
    assert_private_loopback(&machine);
    assert_healthy(&machine_addr(&machine, MACHINE_PORT));
    assert!(root.path().join(&volume).join("family.db").is_file());
}

#[tokio::test]
async fn machines_in_one_mock_get_their_own_addresses() {
    let root = TempDir::new();
    let app = app_with(settings(family_app_bin(), root.path()));
    let volume = app_with_volume(&app).await;
    let (_, a) = post(&app, MACHINES, machine_body(&volume)).await;
    let (_, b) = post(&app, MACHINES, machine_body(&volume)).await;
    assert_ne!(a["private_ip"], b["private_ip"]);
    assert_healthy(&machine_addr(&a, MACHINE_PORT));
    assert_healthy(&machine_addr(&b, MACHINE_PORT));
}

#[tokio::test]
async fn separate_mocks_give_machines_different_addresses() {
    let root = TempDir::new();
    let first = app_with(settings(family_app_bin(), root.path()));
    let second = app_with(settings(family_app_bin(), root.path()));
    let volume = app_with_volume(&first).await;
    let other = app_with_volume(&second).await;
    let (_, a) = post(&first, MACHINES, machine_body(&volume)).await;
    let (_, b) = post(&second, MACHINES, machine_body(&other)).await;
    assert_ne!(a["private_ip"], b["private_ip"]);
    assert_private_loopback(&b);
    assert_healthy(&machine_addr(&a, MACHINE_PORT));
    assert_healthy(&machine_addr(&b, MACHINE_PORT));
}

#[tokio::test]
async fn more_machines_than_one_address_block_holds_stay_unique() {
    // A block holds 61 addresses, and the mock then moves to a new one.
    let root = TempDir::new();
    let app = app_with(settings(family_app_bin(), root.path()));
    let volume = app_with_volume(&app).await;
    let mut addresses = HashSet::new();
    for _ in 0..65 {
        let (status, machine) = post(&app, MACHINES, machine_body(&volume)).await;
        assert_eq!(status, StatusCode::OK, "{machine}");
        assert_private_loopback(&machine);
        assert_healthy(&machine_addr(&machine, MACHINE_PORT));
        assert!(addresses.insert(machine["private_ip"].to_string()));
    }
}

#[tokio::test]
async fn missing_binary_fails_well_within_the_time_limit() {
    let root = TempDir::new();
    let missing = root.path().join("no-such-binary");
    let app = app_with(settings(&missing, root.path()));
    let volume = app_with_volume(&app).await;
    let start = Instant::now();
    let (status, body) = post(&app, MACHINES, machine_body(&volume)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_fly_error(&body);
    assert!(body["error"].as_str().unwrap().contains("cannot start"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn binary_that_exits_at_once_fails_before_the_time_limit() {
    let root = TempDir::new();
    let app = app_with(settings("/bin/false", root.path()));
    let volume = app_with_volume(&app).await;
    let start = Instant::now();
    let (status, body) = post(&app, MACHINES, machine_body(&volume)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_fly_error(&body);
    assert!(body["error"].as_str().unwrap().contains("exited"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn dropping_the_router_stops_the_family_app() {
    let root = TempDir::new();
    let app = app_with(settings(family_app_bin(), root.path()));
    let clone = app.clone();
    let volume = app_with_volume(&app).await;
    let (_, machine) = post(&app, MACHINES, machine_body(&volume)).await;
    let addr = machine_addr(&machine, MACHINE_PORT);
    assert_healthy(&addr);
    drop(app);
    assert_healthy(&addr);
    drop(clone);
    assert_stops_accepting(&addr);
}

#[tokio::test]
async fn config_image_is_ignored() {
    let root = TempDir::new();
    let app = app_with(settings(family_app_bin(), root.path()));
    let volume = app_with_volume(&app).await;
    let body = json!({"config": {"mounts": [{"volume": volume, "path": "/elsewhere"}]}});
    let (status, machine) = post(&app, MACHINES, body).await;
    assert_eq!(status, StatusCode::OK, "{machine}");
    assert_healthy(&machine_addr(&machine, MACHINE_PORT));
}
