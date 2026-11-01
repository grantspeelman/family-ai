//! The Fly mock: a local stand-in for Fly's Machines API, so local dev and CI
//! never need a real Fly token.
//!
//! Paths and JSON follow Fly's Machines API (`https://api.machines.dev`), so
//! moving to real Fly changes only the base URL. State lives in memory.

use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The region used when a Machine request names none.
const DEFAULT_REGION: &str = "mock";

/// Builds the HTTP app with its own empty in-memory store.
pub fn app() -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/apps", post(create_app))
        .route("/v1/apps/{app_name}/volumes", post(create_volume))
        .route("/v1/apps/{app_name}/machines", post(create_machine))
        .with_state(Store::default())
}

type Store = Arc<Mutex<Inner>>;

#[derive(Default)]
struct Inner {
    /// Volume ids per app name.
    apps: HashMap<String, HashSet<String>>,
    /// Counts every volume and Machine created, so ids and addresses are unique.
    next: u64,
}

impl Inner {
    fn next(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

/// Fly's error body, `{"error": "<message>"}`.
fn error(status: StatusCode, message: String) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn app_not_found(app_name: &str) -> Response {
    error(StatusCode::NOT_FOUND, format!("app not found: {app_name}"))
}

fn lock(store: &Store) -> MutexGuard<'_, Inner> {
    store.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Deserialize)]
struct CreateApp {
    name: String,
    #[allow(dead_code, reason = "accepted as Fly does; the mock has no orgs")]
    org_slug: String,
}

async fn create_app(State(store): State<Store>, Json(req): Json<CreateApp>) -> Response {
    let mut inner = lock(&store);
    if inner.apps.contains_key(&req.name) {
        let body = json!({
            "error": format!("app name already taken: {}", req.name),
            "status": "name_taken",
        });
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response();
    }
    let n = inner.next();
    inner.apps.insert(req.name, HashSet::new());
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let body = json!({ "id": format!("app_{n:016x}"), "created_at": created_at });
    (StatusCode::CREATED, Json(body)).into_response()
}

#[derive(Deserialize)]
struct CreateVolume {
    name: String,
    region: String,
    size_gb: u64,
}

#[derive(Serialize)]
struct Volume {
    id: String,
    name: String,
    region: String,
    size_gb: u64,
    state: &'static str,
}

async fn create_volume(
    State(store): State<Store>,
    Path(app_name): Path<String>,
    Json(req): Json<CreateVolume>,
) -> Response {
    let mut inner = lock(&store);
    if !inner.apps.contains_key(&app_name) {
        return app_not_found(&app_name);
    }
    let n = inner.next();
    let volumes = inner.apps.entry(app_name).or_default();
    let id = format!("vol_{n:016x}");
    volumes.insert(id.clone());
    Json(Volume {
        id,
        name: req.name,
        region: req.region,
        size_gb: req.size_gb,
        state: "created",
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateMachine {
    name: Option<String>,
    region: Option<String>,
    config: Config,
}

/// A Machine config: the raw JSON, echoed back exactly as sent, and the
/// volume ids it mounts. A malformed `mounts` fails the `Json` extractor, so
/// axum's default rejection answers it.
#[derive(Deserialize)]
#[serde(try_from = "Value")]
struct Config {
    raw: Value,
    volumes: Vec<String>,
}

impl TryFrom<Value> for Config {
    type Error = serde_json::Error;

    fn try_from(raw: Value) -> Result<Self, Self::Error> {
        #[derive(Deserialize)]
        struct Mounts {
            #[serde(default)]
            mounts: Vec<Mount>,
        }
        #[derive(Deserialize)]
        struct Mount {
            volume: String,
        }
        if !raw.is_object() {
            return Err(serde::de::Error::custom("config must be an object"));
        }
        let Mounts { mounts } = serde_json::from_value(raw.clone())?;
        let volumes = mounts.into_iter().map(|m| m.volume).collect();
        Ok(Self { raw, volumes })
    }
}

#[derive(Serialize)]
struct Machine {
    id: String,
    name: String,
    state: &'static str,
    region: String,
    private_ip: String,
    config: Value,
}

async fn create_machine(
    State(store): State<Store>,
    Path(app_name): Path<String>,
    Json(req): Json<CreateMachine>,
) -> Response {
    let mut inner = lock(&store);
    let Some(volumes) = inner.apps.get(&app_name) else {
        return app_not_found(&app_name);
    };
    if let Some(volume) = req.config.volumes.iter().find(|v| !volumes.contains(*v)) {
        return error(StatusCode::NOT_FOUND, format!("volume not found: {volume}"));
    }
    let n = inner.next();
    let id = format!("{n:014x}");
    // Fly gives Machines an address on its private IPv6 network (fdaa::/16).
    let private_ip = Ipv6Addr::from((0xfdaa_u128 << 112) | u128::from(n));
    Json(Machine {
        name: req.name.unwrap_or_else(|| format!("machine-{id}")),
        id,
        state: "created",
        region: req.region.unwrap_or_else(|| DEFAULT_REGION.to_string()),
        private_ip: private_ip.to_string(),
        config: req.config.raw,
    })
    .into_response()
}
