//! The account store, through its public API, on real database files.

use std::path::PathBuf;

use registry::accounts::{AccountStore, StoreError};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

/// A path no other test uses, with nothing left over from an earlier run.
fn fresh_db(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("accounts-{name}.db"));
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not clear {}: {error}", path.display()),
    }
    path
}

async fn open_fresh(name: &str) -> AccountStore {
    AccountStore::open(&fresh_db(name)).await.unwrap()
}

#[tokio::test]
async fn create_returns_an_account_that_can_be_fetched_by_id() {
    let store = open_fresh("create-fetch").await;

    let created = store.create("ana@example.com", "Silva").await.unwrap();
    assert_eq!(created.email, "ana@example.com");
    assert_eq!(created.family_name, "Silva");

    let fetched = store.get(created.id).await.unwrap();
    assert_eq!(fetched, Some(created));
}

#[tokio::test]
async fn accounts_get_different_ids() {
    let store = open_fresh("ids").await;

    let first = store.create("a@example.com", "Same").await.unwrap();
    let second = store.create("b@example.com", "Same").await.unwrap();

    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn duplicate_email_is_rejected_and_first_account_is_unchanged() {
    let store = open_fresh("duplicate").await;
    let first = store.create("ana@example.com", "Silva").await.unwrap();

    let error = store
        .create("ana@example.com", "Costa")
        .await
        .expect_err("a duplicate email must be rejected");

    assert!(matches!(error, StoreError::DuplicateEmail), "got {error:?}");
    assert_eq!(store.get(first.id).await.unwrap(), Some(first));
}

#[tokio::test]
async fn emails_differing_only_in_letter_case_are_duplicates() {
    let store = open_fresh("case").await;
    store.create("ana@example.com", "Silva").await.unwrap();

    let error = store.create("Ana@Example.com", "Costa").await.unwrap_err();

    assert!(matches!(error, StoreError::DuplicateEmail), "got {error:?}");
}

#[tokio::test]
async fn letter_case_is_ignored_beyond_ascii() {
    let store = open_fresh("case-unicode").await;
    store.create("josé@example.com", "Silva").await.unwrap();
    store.create("straße@example.com", "Weber").await.unwrap();

    for email in ["JOSÉ@example.com", "STRASSE@example.com"] {
        let error = store.create(email, "Costa").await.unwrap_err();
        assert!(
            matches!(error, StoreError::DuplicateEmail),
            "{email}: got {error:?}"
        );
    }
}

#[tokio::test]
async fn email_and_family_name_are_returned_exactly_as_given() {
    let store = open_fresh("exact").await;

    let created = store
        .create("MiXeD@Example.COM", "de la CRUZ")
        .await
        .unwrap();
    assert_eq!(created.email, "MiXeD@Example.COM");
    assert_eq!(created.family_name, "de la CRUZ");

    let fetched = store.get(created.id).await.unwrap().unwrap();
    assert_eq!(fetched.email, "MiXeD@Example.COM");
    assert_eq!(fetched.family_name, "de la CRUZ");
}

#[tokio::test]
async fn sql_in_values_is_stored_as_plain_text() {
    let store = open_fresh("sql-text").await;
    let family_name = "x'); DROP TABLE accounts; --";

    let created = store.create("ana@example.com", family_name).await.unwrap();

    let fetched = store.get(created.id).await.unwrap().unwrap();
    assert_eq!(fetched.family_name, family_name);
}

#[tokio::test]
async fn fetching_an_unknown_id_returns_none() {
    let store = open_fresh("unknown").await;

    assert_eq!(store.get(12345).await.unwrap(), None);
}

#[tokio::test]
async fn accounts_survive_a_restart_and_the_email_stays_taken() {
    let path = fresh_db("restart");

    let store = AccountStore::open(&path).await.unwrap();
    let created = store.create("ana@example.com", "Silva").await.unwrap();
    drop(store);

    let reopened = AccountStore::open(&path).await.unwrap();
    assert_eq!(reopened.get(created.id).await.unwrap(), Some(created));
    let error = reopened
        .create("ANA@example.com", "Costa")
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::DuplicateEmail), "got {error:?}");
}

#[tokio::test]
async fn opening_an_existing_database_repeatedly_keeps_its_data() {
    let path = fresh_db("idempotent");
    let store = AccountStore::open(&path).await.unwrap();
    let created = store.create("ana@example.com", "Silva").await.unwrap();

    // Also while the first handle is still open.
    let second = AccountStore::open(&path).await.unwrap();
    let third = AccountStore::open(&path).await.unwrap();

    assert_eq!(second.get(created.id).await.unwrap(), Some(created.clone()));
    assert_eq!(third.get(created.id).await.unwrap(), Some(created));
}

#[tokio::test]
async fn opening_inside_a_missing_directory_returns_an_error() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("accounts-no-such-directory")
        .join("registry.db");

    assert!(AccountStore::open(&path).await.is_err());
}

#[tokio::test]
async fn opening_a_file_that_is_not_a_database_returns_an_error_and_keeps_the_file() {
    let path = fresh_db("not-a-database");
    let contents = "this is not a SQLite database, and it is long enough to show it";
    std::fs::write(&path, contents).unwrap();

    assert!(AccountStore::open(&path).await.is_err());

    assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
}

#[tokio::test]
async fn the_primary_key_is_the_id_not_the_email() {
    let path = fresh_db("primary-key");
    AccountStore::open(&path).await.unwrap();

    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    let rows = sqlx::query("SELECT name FROM pragma_table_info('accounts') WHERE pk > 0")
        .fetch_all(&mut connection)
        .await
        .unwrap();
    let primary_key: Vec<String> = rows.iter().map(|row| row.get("name")).collect();

    assert_eq!(primary_key, ["id"]);
}

#[tokio::test]
async fn clones_share_the_database_across_tasks() {
    fn assert_shareable<T: Clone + Send + Sync + 'static>() {}
    assert_shareable::<AccountStore>();

    let store = open_fresh("shared").await;
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            tokio::spawn(async move { store.create("ana@example.com", "Silva").await })
        })
        .collect();

    let mut created = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => created += 1,
            Err(StoreError::DuplicateEmail) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(created, 1);
}

#[tokio::test]
async fn the_duplicate_error_does_not_contain_the_email() {
    let store = open_fresh("error-text").await;
    store.create("ana@example.com", "Silva").await.unwrap();

    let error = store.create("ana@example.com", "Costa").await.unwrap_err();

    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("ana@example.com"), "leaked in: {text}");
        assert!(!text.contains("Costa"), "leaked in: {text}");
    }
}
