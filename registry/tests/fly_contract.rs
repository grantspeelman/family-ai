//! The Fly client's contract with Fly's Machines API: what the client sends, and
//! what it can decode, checked against the OpenAPI snapshot the Fly mock keeps
//! in `fly-mock/tests/fixtures/openapi.json`.
//!
//! The client talks to a small stub server on `127.0.0.1` that records each
//! request body as sent. No `fly-mock` process runs.

use std::sync::{Arc, LazyLock, Mutex};

use axum::{
    Router,
    body::Bytes,
    extract::{Request, State},
    http::StatusCode,
};
use jsonschema::Validator;
use registry::fly::{
    CreateMachineRequest, CreateVolumeRequest, FlyClient, FlyConfig, FlyError, MachineConfig, Mount,
};
use serde_json::{Value, json};

static SPEC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fly-mock/tests/fixtures/openapi.json"))
        .expect("spec is JSON")
});

const APPS: &str = "/v1/apps";
const VOLUMES: &str = "/v1/apps/{app_name}/volumes";
const MACHINES: &str = "/v1/apps/{app_name}/machines";

/// The schema at `pointer`, failing the test if there is none.
fn schema_at(pointer: &str) -> &'static Value {
    SPEC.pointer(pointer)
        .unwrap_or_else(|| panic!("no schema at {pointer}"))
}

/// A validator for the schema at `pointer`, with `$ref`s resolved against the
/// whole spec.
fn validator(pointer: &str) -> Validator {
    let schema = schema_at(pointer);
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

fn schema_errors(pointer: &str, instance: &Value) -> Vec<String> {
    validator(pointer)
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect()
}

fn assert_valid(pointer: &str, instance: &Value) {
    let errors = schema_errors(pointer, instance);
    assert!(
        errors.is_empty(),
        "{instance} does not match {pointer}: {errors:?}"
    );
}

/// The spec's JSON pointer for the POST operation on `template`.
fn operation(template: &str) -> String {
    format!(
        "/paths/{}/post",
        template.replace('~', "~0").replace('/', "~1")
    )
}

fn request_pointer(template: &str) -> String {
    format!(
        "{}/requestBody/content/application~1json/schema",
        operation(template)
    )
}

fn response_pointer(template: &str, status: u16) -> String {
    format!(
        "{}/responses/{status}/content/application~1json/schema",
        operation(template)
    )
}

/// Follows `$ref`s until the schema is not a reference.
fn resolve(mut schema: &'static Value) -> &'static Value {
    while let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("external $ref {reference}"));
        schema = schema_at(pointer);
    }
    schema
}

/// The property schemas a schema declares, its own and those of its `allOf`.
fn declared(schema: &'static Value) -> Vec<(&'static String, &'static Value)> {
    let schema = resolve(schema);
    let mut properties: Vec<_> = schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .collect();
    for part in schema
        .get("allOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        properties.extend(declared(part));
    }
    properties
}

/// Paths of keys in `instance` that `schema` does not declare, at any depth.
/// Keys below `config.env` are free-form, so they are not checked.
fn undeclared(schema: &'static Value, instance: &Value, path: &str) -> Vec<String> {
    if path == "/config/env" {
        return Vec::new();
    }
    let schema = resolve(schema);
    match instance {
        Value::Object(fields) => {
            let properties = declared(schema);
            fields
                .iter()
                .flat_map(|(key, value)| {
                    let here = format!("{path}/{key}");
                    match properties.iter().find(|(name, _)| *name == key) {
                        Some((_, property)) => undeclared(property, value, &here),
                        None => vec![here],
                    }
                })
                .collect()
        }
        Value::Array(items) => {
            let item_schema = schema
                .get("items")
                .unwrap_or_else(|| panic!("no items schema at {path}"));
            items
                .iter()
                .enumerate()
                .flat_map(|(i, item)| undeclared(item_schema, item, &format!("{path}/{i}")))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Checks a recorded request body: it matches the request schema for
/// `template`, and every key it holds is declared there.
fn assert_request_matches(template: &str, body: &Value) {
    let pointer = request_pointer(template);
    assert_valid(&pointer, body);
    let unknown = undeclared(schema_at(&pointer), body, "");
    assert!(
        unknown.is_empty(),
        "{body} has keys {pointer} does not declare: {unknown:?}"
    );
}

type Log = Arc<Mutex<Vec<Bytes>>>;

/// Starts a server on `127.0.0.1` that answers everything with `status` and
/// `body`, recording each request body as sent.
async fn stub(status: u16, body: Value) -> (FlyClient, Log) {
    async fn handle(
        State((status, body, log)): State<(u16, String, Log)>,
        request: Request,
    ) -> (StatusCode, [(&'static str, &'static str); 1], String) {
        let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
            .await
            .unwrap();
        log.lock().unwrap().push(bytes);
        (
            StatusCode::from_u16(status).unwrap(),
            [("content-type", "application/json")],
            body,
        )
    }

    let log = Log::default();
    let app = Router::new()
        .fallback(handle)
        .with_state((status, body.to_string(), log.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = FlyConfig::new(&format!("http://{addr}"), None).unwrap();
    (FlyClient::new(config).unwrap(), log)
}

/// The one body the stub received, parsed from the bytes the client sent.
fn sent(log: &Log) -> Value {
    let bodies = log.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one request");
    serde_json::from_slice(&bodies[0]).expect("request body is JSON")
}

fn app_sample() -> Value {
    json!({"id": "app_1", "created_at": 1_762_000_000})
}

fn volume_sample() -> Value {
    json!({
        "id": "vol_123",
        "name": "data",
        "region": "jnb",
        "size_gb": 3,
        "state": "created",
        "encrypted": true,
        "created_at": "2026-11-01T00:00:00Z",
    })
}

fn machine_sample() -> Value {
    json!({
        "id": "0123456789abcd",
        "instance_id": "01J0000000000000000000000",
        "name": "web",
        "state": "started",
        "region": "jnb",
        "private_ip": "fdaa:0:1::2",
        "created_at": "2026-11-01T00:00:00Z",
        "config": {
            "image": "registry.fly.io/family-app:latest",
            "env": {"RUST_LOG": "info"},
            "mounts": [{"volume": "vol_123", "path": "/data"}],
        },
    })
}

fn full_machine_request() -> CreateMachineRequest {
    CreateMachineRequest {
        name: Some("web".into()),
        region: Some("jnb".into()),
        config: MachineConfig {
            image: "registry.fly.io/family-app:latest".into(),
            env: Some([("RUST_LOG".to_owned(), "info".to_owned())].into()),
            mounts: Some(vec![Mount {
                volume: "vol_123".into(),
                path: "/data".into(),
            }]),
        },
    }
}

#[tokio::test]
async fn create_app_sends_a_create_app_request() {
    assert_eq!(
        schema_at(&request_pointer(APPS))["$ref"],
        "#/components/schemas/CreateAppRequest"
    );
    let sample = app_sample();
    assert_valid(&response_pointer(APPS, 201), &sample);
    let (client, log) = stub(201, sample).await;

    client.create_app("fam-1", "personal").await.unwrap();

    let body = sent(&log);
    assert_eq!(body, json!({"name": "fam-1", "org_slug": "personal"}));
    assert_request_matches(APPS, &body);
}

#[tokio::test]
async fn create_app_error_response_gives_api_error() {
    let sample = json!({"error": "app name is taken", "status": "name_taken"});
    assert_valid(&response_pointer(APPS, 422), &sample);
    assert_valid("/components/schemas/ErrorResponse", &sample);
    let (client, _log) = stub(422, sample).await;

    let error = client.create_app("fam-1", "personal").await.unwrap_err();

    match error {
        FlyError::Api { status, message } => {
            assert_eq!(status, 422);
            assert_eq!(message, "app name is taken");
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn create_volume_matches_the_spec() {
    let sample = volume_sample();
    assert_valid(&response_pointer(VOLUMES, 200), &sample);
    let (client, log) = stub(200, sample).await;
    let request = CreateVolumeRequest {
        name: "data".into(),
        region: "jnb".into(),
        size_gb: 3,
    };

    let volume = client.create_volume("fam-1", &request).await.unwrap();

    assert_request_matches(VOLUMES, &sent(&log));
    assert_eq!(volume.id, "vol_123");
    assert_eq!(volume.name, "data");
    assert_eq!(volume.region, "jnb");
    assert_eq!(volume.size_gb, 3);
    assert_eq!(volume.state, "created");
}

#[tokio::test]
async fn create_machine_with_every_optional_field_matches_the_spec() {
    let sample = machine_sample();
    assert_valid(&response_pointer(MACHINES, 200), &sample);
    let (client, log) = stub(200, sample).await;
    let request = full_machine_request();

    let machine = client.create_machine("fam-1", &request).await.unwrap();

    let body = sent(&log);
    for key in ["/name", "/region", "/config/env", "/config/mounts"] {
        assert!(body.pointer(key).is_some(), "{body} has no {key}");
    }
    assert_request_matches(MACHINES, &body);
    assert_eq!(machine.id, "0123456789abcd");
    assert_eq!(machine.name, "web");
    assert_eq!(machine.state, "started");
    assert_eq!(machine.region, "jnb");
    assert_eq!(machine.private_ip, "fdaa:0:1::2");
    assert_eq!(machine.config, request.config);
}

#[tokio::test]
async fn create_machine_with_no_optional_field_matches_the_spec() {
    let (client, log) = stub(200, machine_sample()).await;
    let request = CreateMachineRequest {
        name: None,
        region: None,
        config: MachineConfig {
            image: "registry.fly.io/family-app:latest".into(),
            env: None,
            mounts: None,
        },
    };

    client.create_machine("fam-1", &request).await.unwrap();

    let body = sent(&log);
    assert_eq!(
        body,
        json!({"config": {"image": "registry.fly.io/family-app:latest"}})
    );
    assert_request_matches(MACHINES, &body);
}

#[test]
fn a_misnamed_field_is_rejected() {
    let body = json!({"app_name": "fam-1", "org_slug": "personal"});
    let pointer = request_pointer(APPS);
    assert_eq!(
        undeclared(schema_at(&pointer), &body, ""),
        vec!["/app_name".to_owned()]
    );
    let result = std::panic::catch_unwind(|| assert_request_matches(APPS, &body));
    assert!(result.is_err());
}

#[test]
fn a_nested_misnamed_field_is_rejected() {
    let body = json!({"config": {"image": "i", "mounts": [{"vol": "v", "path": "/data"}]}});
    let pointer = request_pointer(MACHINES);
    assert_eq!(
        undeclared(schema_at(&pointer), &body, ""),
        vec!["/config/mounts/0/vol".to_owned()]
    );
}

#[test]
fn env_keys_are_exempt() {
    let body = json!({"config": {"image": "i", "env": {"ANY_NAME": "x"}}});
    assert_request_matches(MACHINES, &body);
}

#[test]
fn a_size_sent_as_a_string_is_rejected() {
    let body = json!({"name": "data", "region": "jnb", "size_gb": "3"});
    assert!(!schema_errors(&request_pointer(VOLUMES), &body).is_empty());
}

#[test]
#[should_panic(expected = "no schema at")]
fn a_missing_schema_pointer_fails() {
    assert_valid(&response_pointer(VOLUMES, 299), &json!({}));
}
