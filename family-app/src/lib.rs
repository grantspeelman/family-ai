//! The family app: the service that runs on each family's Machine.

use axum::{Router, routing::get};

/// Builds the HTTP app: `GET /healthz`.
pub fn app() -> Router {
    Router::new().route("/healthz", get(|| async { "ok" }))
}
