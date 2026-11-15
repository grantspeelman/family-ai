//! Tests of `family_app::db::open`: the database file, migrations, connection
//! settings and the `notes` table.

mod common;

use common::{TempDir, with_conn};
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use diesel_migrations::MigrationHarness;
use family_app::db;

#[derive(QueryableByName, Debug, PartialEq)]
struct Name {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName, Debug, PartialEq)]
struct Version {
    #[diesel(sql_type = Text)]
    version: String,
}

#[derive(QueryableByName, Debug, PartialEq)]
struct NoteRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = Text)]
    body: String,
    #[diesel(sql_type = Text)]
    created_at: String,
}

#[derive(QueryableByName)]
struct Defaults {
    #[diesel(sql_type = Nullable<BigInt>)]
    id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    created_at: Option<String>,
}

#[derive(QueryableByName, Debug, PartialEq)]
struct Settings {
    #[diesel(sql_type = BigInt)]
    foreign_keys: i64,
    #[diesel(sql_type = BigInt)]
    busy_timeout: i64,
}

/// The applied migrations' versions, as Diesel records them.
async fn applied(pool: &db::Pool) -> Vec<String> {
    with_conn(pool, |conn| {
        sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version")
            .load::<Version>(conn)
            .unwrap()
    })
    .await
    .into_iter()
    .map(|row| row.version)
    .collect()
}

async fn notes(pool: &db::Pool) -> Vec<NoteRow> {
    with_conn(pool, |conn| {
        sql_query("SELECT id, body, created_at FROM notes")
            .load(conn)
            .unwrap()
    })
    .await
}

async fn tables(pool: &db::Pool) -> Vec<String> {
    with_conn(pool, |conn| {
        sql_query("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .load::<Name>(conn)
            .unwrap()
    })
    .await
    .into_iter()
    .map(|row| row.name)
    .collect()
}

#[tokio::test]
async fn fresh_directory_gets_family_db_with_notes_table() {
    let dir = TempDir::new("fresh");
    let pool = db::open(dir.path()).await.unwrap();
    assert!(dir.path().join("family.db").is_file());
    let columns: Vec<String> = with_conn(&pool, |conn| {
        sql_query("SELECT name FROM pragma_table_info('notes') ORDER BY cid")
            .load::<Name>(conn)
            .unwrap()
    })
    .await
    .into_iter()
    .map(|row| row.name)
    .collect();
    assert_eq!(columns, ["id", "body", "created_at"]);
    assert_eq!(applied(&pool).await, ["0001"]);
    pool.close();
}

#[tokio::test]
async fn row_survives_close_and_reopen() {
    let dir = TempDir::new("reopen");
    let pool = db::open(dir.path()).await.unwrap();
    with_conn(&pool, |conn| {
        sql_query("INSERT INTO notes (body) VALUES ('hello')")
            .execute(conn)
            .unwrap()
    })
    .await;
    let before = notes(&pool).await;
    assert_eq!(before.len(), 1);
    pool.close();

    let pool = db::open(dir.path()).await.unwrap();
    assert_eq!(notes(&pool).await, before);
    pool.close();
}

#[tokio::test]
async fn second_open_runs_no_migration_and_keeps_applied_migrations() {
    let dir = TempDir::new("twice");
    let pool = db::open(dir.path()).await.unwrap();
    let before = applied(&pool).await;
    pool.close();

    // A second run of migration 1 would fail (the table exists) or add a row.
    let pool = db::open(dir.path()).await.unwrap();
    assert_eq!(applied(&pool).await, before);
    pool.close();
}

#[tokio::test]
async fn body_is_required_and_defaults_fill_id_and_created_at() {
    let dir = TempDir::new("columns");
    let pool = db::open(dir.path()).await.unwrap();
    let defaults_only = with_conn(&pool, |conn| {
        sql_query("INSERT INTO notes DEFAULT VALUES").execute(conn)
    })
    .await;
    assert!(defaults_only.is_err());
    let row: Defaults = with_conn(&pool, |conn| {
        sql_query("INSERT INTO notes (body) VALUES ('only body')")
            .execute(conn)
            .unwrap();
        sql_query("SELECT id, created_at FROM notes WHERE body = 'only body'")
            .get_result(conn)
            .unwrap()
    })
    .await;
    assert!(row.id.is_some());
    assert!(row.created_at.is_some_and(|value| !value.is_empty()));
    pool.close();
}

#[tokio::test]
async fn missing_directory_is_an_error_and_is_not_created() {
    let dir = TempDir::new("missing");
    let missing = dir.path().join("absent");
    let Err(err) = db::open(&missing).await else {
        panic!("a missing directory opened");
    };
    assert!(err.to_string().contains(&missing.display().to_string()));
    assert!(!missing.exists());
}

#[tokio::test]
async fn every_pooled_connection_enforces_foreign_keys_and_waits_when_busy() {
    let dir = TempDir::new("settings");
    let pool = db::open(dir.path()).await.unwrap();
    // Held at once, so the pool has to open a second connection.
    let first = pool.get().await.unwrap();
    let second = pool.get().await.unwrap();

    for conn in [first, second] {
        let settings: Settings = conn
            .interact(|conn| {
                sql_query(
                    "SELECT (SELECT foreign_keys FROM pragma_foreign_keys) AS foreign_keys, \
                     (SELECT timeout FROM pragma_busy_timeout) AS busy_timeout",
                )
                .get_result(conn)
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            settings,
            Settings {
                foreign_keys: 1,
                busy_timeout: 5000
            }
        );
    }
    pool.close();
}

#[tokio::test]
async fn migrations_revert_cleanly_and_apply_again() {
    let dir = TempDir::new("revert");
    let pool = db::open(dir.path()).await.unwrap();

    with_conn(&pool, |conn| {
        conn.revert_all_migrations(db::MIGRATIONS).unwrap();
    })
    .await;
    assert_eq!(tables(&pool).await, ["__diesel_schema_migrations"]);
    pool.close();

    let pool = db::open(dir.path()).await.unwrap();
    assert!(tables(&pool).await.contains(&"notes".to_owned()));
    assert_eq!(applied(&pool).await, ["0001"]);
    pool.close();
}

#[cfg(unix)]
#[tokio::test]
async fn a_data_dir_path_that_is_not_utf8_is_an_open_error_naming_it() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let dir = TempDir::new("non-utf8");
    let odd = dir.path().join(OsStr::from_bytes(b"data-\xff"));
    std::fs::create_dir(&odd).unwrap();

    let Err(err) = db::open(&odd).await else {
        panic!("a non-UTF-8 path opened");
    };

    assert!(matches!(err, db::Error::Open(..)), "got {err:?}");
    assert!(err.to_string().starts_with("cannot open "), "{err}");
}

#[tokio::test]
async fn a_database_file_that_cannot_be_opened_is_an_open_error_naming_it() {
    let dir = TempDir::new("unopenable");
    // A directory where the database file should be, so no connection opens.
    let path = dir.path().join("family.db");
    std::fs::create_dir(&path).unwrap();

    let Err(err) = db::open(dir.path()).await else {
        panic!("a directory opened as a database");
    };

    assert!(matches!(err, db::Error::Open(..)), "got {err:?}");
    let text = err.to_string();
    assert!(
        text.starts_with(&format!("cannot open {}: ", path.display())),
        "{text}"
    );
}

#[tokio::test]
async fn a_migration_that_fails_is_a_migrate_error_naming_the_file() {
    let dir = TempDir::new("migrate-fails");
    let path = dir.path().join("family.db");
    // A `notes` table no migration created, so migration 0001 cannot create it.
    let mut conn = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    conn.batch_execute("CREATE TABLE notes (id INTEGER)")
        .unwrap();
    drop(conn);

    let Err(err) = db::open(dir.path()).await else {
        panic!("the migration applied over an existing table");
    };

    assert!(matches!(err, db::Error::Migrate(..)), "got {err:?}");
    let text = err.to_string();
    assert!(
        text.starts_with(&format!("cannot migrate {}: ", path.display())),
        "{text}"
    );
}
