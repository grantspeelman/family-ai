//! The family app: the service that runs on each family's Machine.

pub mod db;
mod notes;

use axum::{
    Router,
    routing::{get, post},
};
use sqlx::SqlitePool;

/// Builds the HTTP app on the open database: `GET /healthz`, `POST /notes`
/// and `GET /notes/{id}`.
pub fn app(db: SqlitePool) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/notes", post(notes::create))
        .route("/notes/{id}", get(notes::read))
        .with_state(db)
}
