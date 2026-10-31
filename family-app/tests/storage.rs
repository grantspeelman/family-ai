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

/// The applied migrations as sqlx records them: (version, success, checksum).
async fn applied(pool: &sqlx::SqlitePool) -> Vec<(i64, bool, Vec<u8>)> {
    sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
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
    let applied = applied(&pool).await;
    assert_eq!(applied.len(), 1);
    assert_eq!((applied[0].0, applied[0].1), (1, true));
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
async fn second_open_runs_no_migration_and_keeps_applied_migrations() {
    let dir = TempDir::new("twice");
    let pool = db::open(&dir.0).await.unwrap();
    let before = applied(&pool).await;
    pool.close().await;

    // A second run of migration 1 would fail (the table exists) or add a row.
    let pool = db::open(&dir.0).await.unwrap();
    assert_eq!(applied(&pool).await, before);
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
