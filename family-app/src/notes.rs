//! The round-trip endpoints: write a note to the database and read it back.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use diesel::{prelude::*, sqlite::Sqlite};
use serde::{Deserialize, Serialize};

use crate::{
    db::{self, BoxError, Pool},
    schema::notes,
};

/// A row of the `notes` table, as stored.
#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = notes, check_for_backend(Sqlite))]
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
#[derive(Debug)]
pub struct DbError(BoxError);

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        eprintln!("family-app: database error: {}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
    }
}

/// Runs `query` on a pooled connection, off the async runtime's threads.
async fn run<T: Send + 'static>(
    pool: &Pool,
    query: impl FnOnce(&mut SqliteConnection) -> QueryResult<T> + Send + 'static,
) -> Result<T, DbError> {
    let conn = pool.get().await.map_err(|err| DbError(err.into()))?;
    match conn.interact(query).await {
        Ok(result) => result.map_err(|err| DbError(err.into())),
        Err(err) => Err(DbError(db::interact_error(err))),
    }
}

pub async fn create(
    State(db): State<Pool>,
    Json(new): Json<NewNote>,
) -> Result<(StatusCode, Json<Note>), DbError> {
    let note = run(&db, move |conn| {
        diesel::insert_into(notes::table)
            .values(notes::body.eq(new.body))
            .returning(Note::as_returning())
            .get_result(conn)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(note)))
}

pub async fn read(State(db): State<Pool>, Path(id): Path<i64>) -> Result<Response, DbError> {
    let note = run(&db, move |conn| {
        notes::table
            .find(id)
            .select(Note::as_select())
            .first(conn)
            .optional()
    })
    .await?;
    Ok(match note {
        Some(note) => Json(note).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_panicking_query_is_a_generic_500() {
        let dir =
            std::env::temp_dir().join(format!("family-app-unit-panic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = db::open(&dir).await.unwrap();

        let err = run(&pool, |_| -> QueryResult<()> { panic!("query bug") })
            .await
            .unwrap_err();

        assert!(
            err.0.to_string().contains("database task failed"),
            "{err:?}"
        );
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        pool.close();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
