//! In-process tests of the create-app, create-volume and create-Machine
//! endpoints: requests go straight into the router, no socket.

use std::net::IpAddr;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

async fn create_app(app: &Router, name: &str) -> StatusCode {
    post(
        app,
        "/v1/apps",
        json!({"app_name": name, "org_slug": "personal"}),
    )
    .await
    .0
}

async fn create_volume(app: &Router, app_name: &str, name: &str) -> (StatusCode, Value) {
    post(
        app,
        &format!("/v1/apps/{app_name}/volumes"),
        json!({"name": name, "region": "jnb", "size_gb": 1}),
    )
    .await
}

fn assert_fly_error(body: &Value) {
    let message = body["error"].as_str().expect("error is a string");
    assert!(!message.is_empty());
}

#[tokio::test]
async fn create_app_returns_created() {
    let app = fly_mock::app();
    assert_eq!(create_app(&app, "fam-1").await, StatusCode::CREATED);
}

#[tokio::test]
async fn create_volume_returns_volume() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let (status, volume) = post(
        &app,
        "/v1/apps/fam-1/volumes",
        json!({"name": "data", "region": "jnb", "size_gb": 3}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(volume["id"].as_str().unwrap().starts_with("vol_"));
    assert_eq!(volume["name"], "data");
    assert_eq!(volume["region"], "jnb");
    assert_eq!(volume["size_gb"], 3);
    assert_eq!(volume["state"], "created");
}

#[tokio::test]
async fn create_machine_returns_machine() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let (_, volume) = create_volume(&app, "fam-1", "data").await;
    let config = json!({
        "image": "registry.fly.io/family-app:latest",
        "env": {"RUST_LOG": "info"},
        "mounts": [{"volume": volume["id"], "path": "/data"}],
    });
    let (status, machine) = post(
        &app,
        "/v1/apps/fam-1/machines",
        json!({"name": "web", "region": "jnb", "config": config}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = machine["id"].as_str().unwrap();
    assert_eq!(id.len(), 14);
    assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    assert_eq!(machine["name"], "web");
    assert_eq!(machine["region"], "jnb");
    assert_eq!(machine["state"], "created");
    machine["private_ip"]
        .as_str()
        .unwrap()
        .parse::<IpAddr>()
        .unwrap();
    assert_eq!(machine["config"], config);
}

#[tokio::test]
async fn machine_without_name_or_region_gets_defaults() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let (status, machine) = post(
        &app,
        "/v1/apps/fam-1/machines",
        json!({"config": {"image": "img"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!machine["name"].as_str().unwrap().is_empty());
    assert!(!machine["region"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn machines_and_volumes_get_unique_ids_and_addresses() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let body = json!({"config": {"image": "img"}});
    let (_, a) = post(&app, "/v1/apps/fam-1/machines", body.clone()).await;
    let (_, b) = post(&app, "/v1/apps/fam-1/machines", body).await;
    assert_ne!(a["id"], b["id"]);
    assert_ne!(a["private_ip"], b["private_ip"]);
    let (_, v1) = create_volume(&app, "fam-1", "one").await;
    let (_, v2) = create_volume(&app, "fam-1", "two").await;
    assert_ne!(v1["id"], v2["id"]);
}

#[tokio::test]
async fn volume_under_unknown_app_is_not_found() {
    let app = fly_mock::app();
    let (status, body) = create_volume(&app, "ghost", "data").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_fly_error(&body);
}

#[tokio::test]
async fn machine_under_unknown_app_is_not_found() {
    let app = fly_mock::app();
    let (status, body) = post(
        &app,
        "/v1/apps/ghost/machines",
        json!({"config": {"image": "img"}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_fly_error(&body);
}

#[tokio::test]
async fn machine_mounting_unknown_volume_is_not_found() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    create_app(&app, "fam-2").await;
    // A volume that exists, but in another app, is unknown to this one.
    let (_, other) = create_volume(&app, "fam-2", "data").await;
    for volume in [json!("vol_missing"), other["id"].clone()] {
        let (status, body) = post(
            &app,
            "/v1/apps/fam-1/machines",
            json!({"config": {"image": "img", "mounts": [{"volume": volume, "path": "/data"}]}}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_fly_error(&body);
    }
}

#[tokio::test]
async fn separate_apps_do_not_share_state() {
    let first = fly_mock::app();
    let second = fly_mock::app();
    create_app(&first, "fam-1").await;
    let (status, body) = create_volume(&second, "fam-1", "data").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_fly_error(&body);
}
