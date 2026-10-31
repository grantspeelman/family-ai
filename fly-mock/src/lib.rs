//! The Fly mock: a local stand-in for Fly's Machines API, so local dev and CI
//! never need a real Fly token.

use axum::{Router, routing::get};

/// Builds the HTTP app. Today it serves only `GET /healthz`.
pub fn app() -> Router {
    Router::new().route("/healthz", get(|| async { "ok" }))
}
