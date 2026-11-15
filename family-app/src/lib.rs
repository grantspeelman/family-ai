//! The family app: the service that runs on each family's Machine.

pub mod db;
mod notes;
mod schema;

use axum::{
    Router,
    routing::{get, post},
};

/// Builds the HTTP app on the open database: `GET /healthz`, `POST /notes`
/// and `GET /notes/{id}`.
pub fn app(db: db::Pool) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/notes", post(notes::create))
        .route("/notes/{id}", get(notes::read))
        .with_state(db)
}
