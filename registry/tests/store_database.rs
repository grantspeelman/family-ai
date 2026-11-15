//! The account store's database file: opening it, its migrations, and what
//! the store shows of itself.

use std::error::Error;

use diesel::{Connection, RunQueryDsl, SqliteConnection, sql_query};
use diesel_migrations::MigrationHarness;
use registry::accounts::{AccountStore, MIGRATIONS, Status, StoreError};

mod common;

fn connect(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::establish(path.to_str().unwrap()).unwrap()
}

#[tokio::test]
async fn migrations_revert_cleanly_and_apply_again() {
    let path = common::fresh_db("store-revert");
    drop(AccountStore::open(&path).await.unwrap());
    connect(&path).revert_all_migrations(MIGRATIONS).unwrap();

    let store = AccountStore::open(&path).await.unwrap();

    let account = store.create("ana@example.com", "Silva").await.unwrap();
    assert_eq!(account.status, Status::Provisioning);
}

#[tokio::test]
async fn a_failed_migration_is_a_migration_error() {
    let path = common::fresh_db("store-bad-migration");
    drop(AccountStore::open(&path).await.unwrap());
    // The first migration cannot run again over the table it created.
    sql_query("DELETE FROM __diesel_schema_migrations")
        .execute(&mut connect(&path))
        .unwrap();

    let error = AccountStore::open(&path).await.unwrap_err();

    assert!(matches!(error, StoreError::Migration(_)), "got {error:?}");
    assert!(
        error
            .to_string()
            .starts_with("account database migration failed")
    );
    assert!(error.source().is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn a_path_that_is_not_utf8_is_a_database_error() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let path = std::env::temp_dir().join(OsStr::from_bytes(b"registry-\xff.db"));

    let error = AccountStore::open(&path).await.unwrap_err();

    assert!(matches!(error, StoreError::Database(_)), "got {error:?}");
    assert!(error.source().is_some());
}

#[tokio::test]
async fn debug_output_shows_the_pool_and_no_path() {
    let path = common::fresh_db("store-debug");
    let store = AccountStore::open(&path).await.unwrap();

    let text = format!("{store:?}");

    assert!(text.starts_with("AccountStore"), "{text}");
    assert!(!text.contains(&path.display().to_string()), "{text}");
}
