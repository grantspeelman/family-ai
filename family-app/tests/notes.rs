//! The database round trip over HTTP: `POST /notes` and `GET /notes/{id}`,
//! sent straight into the router against a real `family.db`.

mod common;

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use common::TempDir;
use family_app::db;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// A router on a freshly opened database in its own directory. Holding the
/// directory keeps it alive for the test; it is removed on drop.
async fn setup(name: &str) -> (TempDir, SqlitePool, Router) {
    let dir = TempDir::new(name);
    let pool = db::open(dir.path()).await.unwrap();
    let router = family_app::app(pool.clone());
    (dir, pool, router)
}

fn post_raw(body: &str) -> Request<Body> {
    Request::post("/notes")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn post(router: &Router, body: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    common::send(router.clone(), post_raw(body)).await
}

async fn get(router: &Router, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::get(path).body(Body::empty()).unwrap();
    common::send(router.clone(), request).await
}

fn parse(body: &[u8]) -> Value {
    serde_json::from_slice(body).unwrap()
}

fn assert_json(headers: &HeaderMap) {
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
}

async fn row_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Writes a note, asserts the write succeeded, and returns the stored row.
async fn write(router: &Router, text: &str) -> Value {
    let (status, _, body) = post(router, &json!({ "body": text }).to_string()).await;
    assert_eq!(status, StatusCode::CREATED);
    parse(&body)
}

#[tokio::test]
async fn write_returns_the_row_sqlite_stored() {
    let (_dir, pool, router) = setup("write").await;

    let (status, headers, body) = post(&router, r#"{"body": "hello"}"#).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_json(&headers);
    let note = parse(&body);
    assert_eq!(note.as_object().unwrap().len(), 3, "note: {note}");
    assert_eq!(note["body"], "hello");
    let stored: (i64, String, String) = sqlx::query_as("SELECT id, body, created_at FROM notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(note["id"], stored.0);
    assert_eq!(note["body"], stored.1);
    assert_eq!(note["created_at"], stored.2);
    // YYYY-MM-DD HH:MM:SS
    let created_at = stored.2.as_bytes();
    assert_eq!(created_at.len(), 19, "created_at: {}", stored.2);
    assert_eq!(created_at[10], b' ');
    pool.close().await;
}

#[tokio::test]
async fn read_returns_the_written_row() {
    let (_dir, pool, router) = setup("read").await;
    let note = write(&router, "hello").await;

    let (status, headers, body) = get(&router, &format!("/notes/{}", note["id"])).await;

    assert_eq!(status, StatusCode::OK);
    assert_json(&headers);
    assert_eq!(parse(&body), note);
    pool.close().await;
}

#[tokio::test]
async fn read_comes_from_the_database() {
    let (_dir, pool, router) = setup("read-db").await;
    let note = write(&router, "before").await;
    sqlx::query("UPDATE notes SET body = 'after'")
        .execute(&pool)
        .await
        .unwrap();

    let (_, _, body) = get(&router, &format!("/notes/{}", note["id"])).await;

    assert_eq!(parse(&body)["body"], "after");
    pool.close().await;
}

#[tokio::test]
async fn two_writes_get_different_ids_and_read_back_their_own_body() {
    let (_dir, pool, router) = setup("two").await;
    let first = write(&router, "first").await;
    let second = write(&router, "second").await;
    assert_ne!(first["id"], second["id"]);

    for note in [first, second] {
        let (_, _, body) = get(&router, &format!("/notes/{}", note["id"])).await;
        assert_eq!(parse(&body), note);
    }
    pool.close().await;
}

#[tokio::test]
async fn empty_and_awkward_bodies_round_trip_unchanged() {
    let (_dir, pool, router) = setup("awkward").await;
    for text in ["", "it's \"quoted\"; DROP TABLE notes; --", "héllo ✓\n"] {
        let note = write(&router, text).await;
        assert_eq!(note["body"], text);
        let (_, _, body) = get(&router, &format!("/notes/{}", note["id"])).await;
        assert_eq!(parse(&body)["body"], text);
    }
    assert_eq!(row_count(&pool).await, 3);
    pool.close().await;
}

#[tokio::test]
async fn id_with_no_row_is_not_found() {
    let (_dir, pool, router) = setup("not-found").await;
    let (status, _, _) = get(&router, "/notes/1").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    pool.close().await;
}

#[tokio::test]
async fn bad_write_is_a_client_error_and_stores_nothing() {
    let (_dir, pool, router) = setup("bad-write").await;
    for body in ["{not json", "{}", r#"{"body": 1}"#, r#"{"body": null}"#] {
        let (status, _, _) = post(&router, body).await;
        assert!(status.is_client_error(), "{body}: {status}");
    }
    let no_content_type = Request::post("/notes")
        .body(Body::from(r#"{"body": "hello"}"#))
        .unwrap();
    let (status, _, _) = common::send(router.clone(), no_content_type).await;
    assert!(status.is_client_error(), "no content type: {status}");

    assert_eq!(row_count(&pool).await, 0);
    pool.close().await;
}

#[tokio::test]
async fn non_integer_id_is_a_client_error() {
    let (_dir, pool, router) = setup("bad-id").await;
    for path in ["/notes/abc", "/notes/1.5", "/notes/9223372036854775808"] {
        let (status, _, _) = get(&router, path).await;
        assert!(status.is_client_error(), "{path}: {status}");
    }
    pool.close().await;
}

#[tokio::test]
async fn note_survives_reopening_the_data_directory() {
    let (dir, pool, router) = setup("restart").await;
    let note = write(&router, "kept").await;
    drop(router);
    pool.close().await;

    let pool = db::open(dir.path()).await.unwrap();
    let router = family_app::app(pool.clone());
    let (status, _, body) = get(&router, &format!("/notes/{}", note["id"])).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse(&body), note);
    pool.close().await;
}

#[tokio::test]
async fn database_errors_are_generic_500s() {
    let (_dir, pool, router) = setup("db-error").await;
    sqlx::query("DROP TABLE notes")
        .execute(&pool)
        .await
        .unwrap();

    let write = post(&router, r#"{"body": "hello"}"#).await;
    let read = get(&router, "/notes/1").await;

    for (status, _, body) in [write, read] {
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, b"internal server error");
    }
    pool.close().await;
}
