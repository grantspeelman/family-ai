//! Tests of `family_app::db::open`: the database file, migrations and the
//! `notes` table.

mod common;

use common::TempDir;
use family_app::db;
use sqlx::Row;

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
    let pool = db::open(dir.path()).await.unwrap();
    assert!(dir.path().join("family.db").is_file());
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
    let pool = db::open(dir.path()).await.unwrap();
    sqlx::query("INSERT INTO notes (body) VALUES ('hello')")
        .execute(&pool)
        .await
        .unwrap();
    let before: (i64, String, String) = sqlx::query_as("SELECT id, body, created_at FROM notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;

    let pool = db::open(dir.path()).await.unwrap();
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
    let pool = db::open(dir.path()).await.unwrap();
    let before = applied(&pool).await;
    pool.close().await;

    // A second run of migration 1 would fail (the table exists) or add a row.
    let pool = db::open(dir.path()).await.unwrap();
    assert_eq!(applied(&pool).await, before);
    pool.close().await;
}

#[tokio::test]
async fn body_is_required_and_defaults_fill_id_and_created_at() {
    let dir = TempDir::new("columns");
    let pool = db::open(dir.path()).await.unwrap();
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
    let missing = dir.path().join("absent");
    let err = db::open(&missing).await.unwrap_err();
    assert!(err.to_string().contains(&missing.display().to_string()));
    assert!(!missing.exists());
}

#[tokio::test]
async fn a_database_file_that_cannot_be_opened_is_an_error_naming_it() {
    let dir = TempDir::new("unopenable");
    // A directory where the database file should be.
    let path = dir.path().join("family.db");
    std::fs::create_dir(&path).unwrap();

    let err = db::open(dir.path()).await.unwrap_err();

    assert!(matches!(err, db::Error::Open(..)), "{err:?}");
    let text = err.to_string();
    assert!(
        text.starts_with(&format!("cannot open {}: ", path.display())),
        "{text}"
    );
}

#[tokio::test]
async fn a_database_from_a_newer_family_app_is_a_migration_error_naming_it() {
    let dir = TempDir::new("newer");
    let pool = db::open(dir.path()).await.unwrap();
    // A migration this build does not have, as if a newer family app had run.
    sqlx::query(
        "INSERT INTO _sqlx_migrations \
         (version, description, success, checksum, execution_time) \
         VALUES (99999999999999, 'from the future', TRUE, x'00', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let err = db::open(dir.path()).await.unwrap_err();

    assert!(matches!(err, db::Error::Migrate(..)), "{err:?}");
    let path = dir.path().join("family.db");
    let text = err.to_string();
    assert!(
        text.starts_with(&format!("cannot migrate {}: ", path.display())),
        "{text}"
    );
}
