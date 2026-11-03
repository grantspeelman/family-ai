//! In-process tests of the create-app, create-volume and create-Machine
//! endpoints: requests go straight into the router, no socket. Every request
//! sent and every response received is checked against Fly's Machines OpenAPI
//! spec, vendored in `tests/fixtures/openapi.json`.

mod common;

use std::sync::LazyLock;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jsonschema::Validator;
use serde_json::{Value, json};
use tower::ServiceExt;

static SPEC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("fixtures/openapi.json")).expect("spec is JSON")
});

/// A validator for the schema at `pointer` in the spec, with `$ref`s resolved
/// against the whole spec document.
fn validator(pointer: &str) -> Validator {
    let schema = SPEC
        .pointer(pointer)
        .unwrap_or_else(|| panic!("no schema at {pointer}"));
    let mut root = SPEC.clone();
    let object = root.as_object_mut().unwrap();
    object.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    for (key, value) in schema.as_object().unwrap() {
        object.insert(key.clone(), value.clone());
    }
    jsonschema::validator_for(&root).unwrap()
}

fn assert_valid(pointer: &str, instance: &Value) {
    let errors: Vec<String> = validator(pointer)
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "{instance} does not match {pointer}: {errors:?}"
    );
}

/// The spec's JSON pointer for an operation: `/paths/<path>/post`.
fn operation(template: &str) -> String {
    format!(
        "/paths/{}/post",
        template.replace('~', "~0").replace('/', "~1")
    )
}

/// Sends `body` to the path `template` names, with `{app_name}` filled in.
/// Checks the request against the spec, then the response against the
/// spec's schema for that status, or Fly's `ErrorResponse` for an error the
/// spec does not list.
async fn post(app: &Router, template: &str, app_name: &str, body: Value) -> (StatusCode, Value) {
    let op = operation(template);
    assert_valid(
        &format!("{op}/requestBody/content/application~1json/schema"),
        &body,
    );
    let request = Request::post(template.replace("{app_name}", app_name))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).expect("response is JSON");
    let documented = format!("{op}/responses/{}", status.as_u16());
    if SPEC.pointer(&documented).is_some() {
        assert_valid(
            &format!("{documented}/content/application~1json/schema"),
            &json,
        );
    } else {
        assert!(
            status.is_client_error() || status == StatusCode::INTERNAL_SERVER_ERROR,
            "undocumented status {status}"
        );
        assert_valid("/components/schemas/ErrorResponse", &json);
    }
    (status, json)
}

const APPS: &str = "/v1/apps";
const VOLUMES: &str = "/v1/apps/{app_name}/volumes";
const MACHINES: &str = "/v1/apps/{app_name}/machines";

async fn create_app(app: &Router, name: &str) -> (StatusCode, Value) {
    post(app, APPS, "", json!({"name": name, "org_slug": "personal"})).await
}

async fn create_volume(app: &Router, app_name: &str, name: &str) -> (StatusCode, Value) {
    post(
        app,
        VOLUMES,
        app_name,
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
    let (status, body) = create_app(&app, "fam-1").await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!body["id"].as_str().unwrap().is_empty());
    assert!(body["created_at"].is_u64());
}

#[tokio::test]
async fn taken_app_name_is_rejected() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let (status, body) = create_app(&app, "fam-1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["status"], "name_taken");
    assert_fly_error(&body);
}

#[tokio::test]
async fn create_volume_returns_volume() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    let (status, volume) = post(
        &app,
        VOLUMES,
        "fam-1",
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

/// A mock that boots the real family app, and the directory it keeps volumes in.
fn booting_app() -> (Router, common::TempDir) {
    let root = common::TempDir::new();
    let settings = common::settings(common::family_app_bin(), root.path());
    (fly_mock::app_with(settings), root)
}

#[tokio::test]
async fn create_machine_returns_machine() {
    let (app, _root) = booting_app();
    create_app(&app, "fam-1").await;
    let (_, volume) = create_volume(&app, "fam-1", "data").await;
    let config = json!({
        "image": "registry.fly.io/family-app:latest",
        "env": {"RUST_LOG": "info"},
        "mounts": [{"volume": volume["id"], "path": "/data"}],
    });
    let (status, machine) = post(
        &app,
        MACHINES,
        "fam-1",
        json!({"name": "web", "region": "jnb", "config": config}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = machine["id"].as_str().unwrap();
    assert_eq!(id.len(), 14);
    assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    assert_eq!(machine["name"], "web");
    assert_eq!(machine["region"], "jnb");
    assert_eq!(machine["state"], "started");
    common::assert_private_loopback(&machine);
    assert_eq!(machine["config"], config);
}

#[tokio::test]
async fn machine_without_name_or_region_gets_defaults() {
    let (app, _root) = booting_app();
    create_app(&app, "fam-1").await;
    let (_, volume) = create_volume(&app, "fam-1", "data").await;
    let body = common::machine_body(volume["id"].as_str().unwrap());
    let (status, machine) = post(&app, MACHINES, "fam-1", body).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!machine["name"].as_str().unwrap().is_empty());
    assert!(!machine["region"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn machines_and_volumes_get_unique_ids_and_addresses() {
    let (app, _root) = booting_app();
    create_app(&app, "fam-1").await;
    let (_, volume) = create_volume(&app, "fam-1", "data").await;
    let body = common::machine_body(volume["id"].as_str().unwrap());
    let (_, a) = post(&app, MACHINES, "fam-1", body.clone()).await;
    let (_, b) = post(&app, MACHINES, "fam-1", body).await;
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
async fn failed_start_matches_the_error_schema() {
    let root = common::TempDir::new();
    let app = fly_mock::app_with(common::settings("/bin/false", root.path()));
    create_app(&app, "fam-1").await;
    let (_, volume) = create_volume(&app, "fam-1", "data").await;
    let body = common::machine_body(volume["id"].as_str().unwrap());
    let (status, body) = post(&app, MACHINES, "fam-1", body).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_fly_error(&body);
}

#[tokio::test]
async fn machine_under_unknown_app_is_not_found() {
    let app = fly_mock::app();
    let (status, body) = post(&app, MACHINES, "ghost", json!({"config": {"image": "img"}})).await;
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
            MACHINES,
            "fam-1",
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

#[tokio::test]
async fn malformed_configs_get_axums_default_rejection() {
    let app = fly_mock::app();
    create_app(&app, "fam-1").await;
    for config in [json!({"image": "img", "mounts": "nope"}), json!([])] {
        let request = Request::post("/v1/apps/fam-1/machines")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "config": config }).to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert!(response.status().is_client_error(), "{config}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        // Plain text, not Fly's JSON error body.
        assert!(serde_json::from_slice::<Value>(&bytes).is_err());
    }
}
