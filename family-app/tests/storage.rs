//! Tests of `family_app::db::open`: the database file, migrations and the
//! `notes` table.

use std::path::PathBuf;

use family_app::db;
use sqlx::Row;

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("family-app-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn user_version(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn fresh_directory_gets_family_db_with_notes_table() {
    let dir = TempDir::new("fresh");
    let pool = db::open(&dir.0).await.unwrap();
    assert!(dir.0.join("family.db").is_file());
    let columns: Vec<String> =
        sqlx::query("SELECT name FROM pragma_table_info('notes') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get("name"))
            .collect();
    assert_eq!(columns, ["id", "body", "created_at"]);
    assert_eq!(user_version(&pool).await, db::SCHEMA_VERSION);
    pool.close().await;
}

#[tokio::test]
async fn row_survives_close_and_reopen() {
    let dir = TempDir::new("reopen");
    let pool = db::open(&dir.0).await.unwrap();
    sqlx::query("INSERT INTO notes (body) VALUES ('hello')")
        .execute(&pool)
        .await
        .unwrap();
    let before: (i64, String, String) = sqlx::query_as("SELECT id, body, created_at FROM notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;

    let pool = db::open(&dir.0).await.unwrap();
    let after: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, body, created_at FROM notes")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(after, [before]);
    pool.close().await;
}

#[tokio::test]
async fn second_open_runs_no_migration_and_keeps_version() {
    let dir = TempDir::new("twice");
    let pool = db::open(&dir.0).await.unwrap();
    let version = user_version(&pool).await;
    pool.close().await;

    // A second run of the first migration would fail: the table exists.
    let pool = db::open(&dir.0).await.unwrap();
    assert_eq!(user_version(&pool).await, version);
    pool.close().await;
}

#[tokio::test]
async fn body_is_required_and_defaults_fill_id_and_created_at() {
    let dir = TempDir::new("columns");
    let pool = db::open(&dir.0).await.unwrap();
    assert!(
        sqlx::query("INSERT INTO notes DEFAULT VALUES")
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO notes (body) VALUES ('only body')")
        .execute(&pool)
        .await
        .unwrap();
    let (id, created_at): (Option<i64>, Option<String>) =
        sqlx::query_as("SELECT id, created_at FROM notes WHERE body = 'only body'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(id.is_some());
    assert!(created_at.is_some_and(|value| !value.is_empty()));
    pool.close().await;
}

#[tokio::test]
async fn missing_directory_is_an_error_and_is_not_created() {
    let dir = TempDir::new("missing");
    let missing = dir.0.join("absent");
    let err = db::open(&missing).await.unwrap_err();
    assert!(err.to_string().contains(&missing.display().to_string()));
    assert!(!missing.exists());
}
