//! The round-trip endpoints: write a note to the database and read it back.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// A row of the `notes` table, as stored.
#[derive(Serialize, sqlx::FromRow)]
pub struct Note {
    id: i64,
    body: String,
    created_at: String,
}

#[derive(Deserialize)]
pub struct NewNote {
    body: String,
}

/// A database failure. The details go to the server's log and never into the
/// response, because they can hold SQL and file paths.
pub struct DbError(sqlx::Error);

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        DbError(err)
    }
}

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        eprintln!("family-app: database error: {}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
    }
}

pub async fn create(
    State(db): State<SqlitePool>,
    Json(new): Json<NewNote>,
) -> Result<(StatusCode, Json<Note>), DbError> {
    let note = sqlx::query_as("INSERT INTO notes (body) VALUES (?) RETURNING id, body, created_at")
        .bind(new.body)
        .fetch_one(&db)
        .await?;
    Ok((StatusCode::CREATED, Json(note)))
}

pub async fn read(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Result<Response, DbError> {
    let note: Option<Note> = sqlx::query_as("SELECT id, body, created_at FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?;
    Ok(match note {
        Some(note) => Json(note).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    })
}
