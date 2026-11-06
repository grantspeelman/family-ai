//! Provisioning state in the account store: the migration, its constraints
//! and the startup step that fails interrupted provisioning.

use std::path::Path;

use registry::accounts::{AccountStore, Status};
use sqlx::{Connection, SqliteConnection, migrate::Migrator, sqlite::SqliteConnectOptions};

mod common;

async fn connect(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options).await.unwrap()
}

#[tokio::test]
async fn a_row_from_before_the_migration_reads_back_failed_without_an_address() {
    let path = common::fresh_db("store-old-row");
    let old = std::env::temp_dir().join(format!("registry-old-migrations-{}", std::process::id()));
    std::fs::create_dir_all(&old).unwrap();
    let first = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0001_create_accounts.sql"
    );
    std::fs::copy(first, old.join("0001_create_accounts.sql")).unwrap();
    let mut connection = connect(&path).await;
    Migrator::new(old.as_path())
        .await
        .unwrap()
        .run(&mut connection)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO accounts (email, email_key, family_name) \
         VALUES ('ana@example.com', 'ana@example.com', 'Silva')",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    connection.close().await.unwrap();
    std::fs::remove_dir_all(&old).unwrap();

    let store = AccountStore::open(&path).await.unwrap();

    let account = store.get(1).await.unwrap().unwrap();
    assert_eq!(account.status, Status::Failed);
    assert_eq!(account.address, None);
    assert_eq!(account.fly_app_name, None);
}

#[tokio::test]
async fn a_status_outside_the_three_values_cannot_be_stored() {
    let path = common::fresh_db("store-bad-status");
    let store = AccountStore::open(&path).await.unwrap();
    let account = store.create("ana@example.com", "Silva").await.unwrap();
    let mut connection = connect(&path).await;

    for status in ["done", "READY", ""] {
        let result = sqlx::query("UPDATE accounts SET status = ? WHERE id = ?")
            .bind(status)
            .bind(account.id)
            .execute(&mut connection)
            .await;
        assert!(result.is_err(), "status {status:?} was stored");
    }
    let result = sqlx::query("UPDATE accounts SET address = 'http://x' WHERE id = ?")
        .bind(account.id)
        .execute(&mut connection)
        .await;
    assert!(result.is_err(), "an address was stored while provisioning");
}

#[tokio::test]
async fn new_accounts_start_provisioning_without_an_address() {
    let store = common::fresh_store("store-new").await;

    let account = store.create("ana@example.com", "Silva").await.unwrap();

    assert_eq!(account.status, Status::Provisioning);
    assert_eq!(account.address, None);
}

#[tokio::test]
async fn startup_fails_interrupted_accounts_and_keeps_ready_ones() {
    let store = common::fresh_store("store-restart").await;
    let interrupted = store.create("ana@example.com", "Silva").await.unwrap();
    let ready = store.create("bo@example.com", "Costa").await.unwrap();
    assert!(
        store
            .mark_ready(ready.id, "http://10.0.0.7:8080")
            .await
            .unwrap()
    );

    assert_eq!(store.fail_interrupted().await.unwrap(), 1);

    let interrupted = store.get(interrupted.id).await.unwrap().unwrap();
    assert_eq!(interrupted.status, Status::Failed);
    assert_eq!(interrupted.address, None);
    let ready = store.get(ready.id).await.unwrap().unwrap();
    assert_eq!(ready.status, Status::Ready);
    assert_eq!(ready.address.as_deref(), Some("http://10.0.0.7:8080"));
}

#[tokio::test]
async fn the_final_update_applies_only_while_provisioning() {
    let store = common::fresh_store("store-final-once").await;
    let account = store.create("ana@example.com", "Silva").await.unwrap();
    assert!(store.mark_failed(account.id).await.unwrap());

    assert!(
        !store
            .mark_ready(account.id, "http://10.0.0.7:8080")
            .await
            .unwrap()
    );
    store
        .set_fly_app_name(account.id, "family-1")
        .await
        .unwrap();

    let account = store.get(account.id).await.unwrap().unwrap();
    assert_eq!(account.status, Status::Failed);
    assert_eq!(account.address, None);
    assert_eq!(account.fly_app_name, None);
}
