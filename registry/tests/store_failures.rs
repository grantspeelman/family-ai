//! What callers see when the account database fails or is missing: store
//! errors, and the GraphQL API's `INTERNAL` error.

use std::error::Error;

use async_graphql::{EmptySubscription, Schema};
use registry::{
    Mutation, Query,
    accounts::{AccountStore, StoreError},
};
use serde_json::{Value, json};

mod common;

const INTERNAL_MESSAGE: &str = "Something went wrong. Try again later.";

/// A store whose `accounts` table is gone, so every query on it fails.
async fn store_without_accounts_table(name: &str) -> AccountStore {
    let path = common::fresh_db(name);
    let store = AccountStore::open(&path).await.unwrap();
    common::execute_sql(&path, "DROP TABLE accounts").await;
    store
}

fn assert_internal(json: &Value, field: &str) {
    assert_eq!(json["data"], Value::Null, "{json}");
    let error = &json["errors"][0];
    assert_eq!(error["message"], INTERNAL_MESSAGE, "{json}");
    assert_eq!(error["extensions"]["code"], "INTERNAL", "{json}");
    assert_eq!(error["path"][0], field, "{json}");
}

#[tokio::test]
async fn a_database_error_is_a_database_store_error_with_a_source() {
    let store = store_without_accounts_table("store-failure-get").await;

    let error = store.get(1).await.unwrap_err();

    assert!(matches!(error, StoreError::Database(_)), "{error:?}");
    assert!(
        error.to_string().starts_with("account database error: "),
        "{error}"
    );
    assert!(error.source().is_some());
}

#[tokio::test]
async fn a_duplicate_email_error_has_no_source() {
    let store = common::fresh_store("store-failure-duplicate").await;
    store.create("ana@example.com", "Silva").await.unwrap();

    let error = store.create("ana@example.com", "Costa").await.unwrap_err();

    assert!(error.source().is_none());
}

#[tokio::test]
async fn a_database_from_a_newer_registry_is_a_migration_error() {
    let path = common::fresh_db("store-failure-migration");
    drop(AccountStore::open(&path).await.unwrap());
    // A migration this build does not have, as if a newer registry had run.
    common::execute_sql(
        &path,
        "INSERT INTO _sqlx_migrations \
         (version, description, success, checksum, execution_time) \
         VALUES (99999999999999, 'from the future', TRUE, x'00', 0)",
    )
    .await;

    let error = AccountStore::open(&path).await.unwrap_err();

    assert!(matches!(error, StoreError::Migration(_)), "{error:?}");
    assert!(
        error
            .to_string()
            .starts_with("account database migration failed: "),
        "{error}"
    );
    assert!(error.source().is_some());
}

#[tokio::test]
async fn account_status_is_internal_when_the_store_fails() {
    let store = store_without_accounts_table("store-failure-status").await;
    let app = registry::app(store, common::provisioner());

    let json = common::graphql(
        app,
        "query { accountStatus(id: \"1\") { status } }",
        json!({}),
    )
    .await;

    assert_internal(&json, "accountStatus");
}

#[tokio::test]
async fn create_account_is_internal_when_the_store_fails() {
    let store = store_without_accounts_table("store-failure-create").await;
    let app = registry::app(store, common::provisioner());

    let json = common::graphql(
        app,
        "mutation { createAccount(email: \"ana@example.com\", familyName: \"Silva\") { id } }",
        json!({}),
    )
    .await;

    assert_internal(&json, "createAccount");
    let text = json.to_string();
    assert!(!text.contains("ana@example.com"), "{text}");
    assert!(!text.contains("Silva"), "{text}");
}

/// Runs `query` on a schema built without the account store, as a caller of
/// the public `Query` and `Mutation` types could.
async fn without_a_store(query: &str) -> Value {
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(common::provisioner())
        .finish();
    serde_json::to_value(schema.execute(query).await).unwrap()
}

#[tokio::test]
async fn account_status_is_internal_without_a_store() {
    let json = without_a_store("query { accountStatus(id: \"1\") { status } }").await;

    assert_internal(&json, "accountStatus");
}

#[tokio::test]
async fn create_account_is_internal_without_a_store() {
    let json = without_a_store(
        "mutation { createAccount(email: \"ana@example.com\", familyName: \"Silva\") { id } }",
    )
    .await;

    assert_internal(&json, "createAccount");
}
