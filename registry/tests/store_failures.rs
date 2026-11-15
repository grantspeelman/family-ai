//! What GraphQL callers see when the account database fails or is missing:
//! the `INTERNAL` error. `store_database.rs` and the unit tests in
//! `src/accounts.rs` cover the store's own errors.

use async_graphql::{EmptySubscription, Schema};
use registry::{Mutation, Query, accounts::AccountStore};
use serde_json::{Value, json};

mod common;

const INTERNAL_MESSAGE: &str = "Something went wrong. Try again later.";

/// A store whose `accounts` table is gone, so every query on it fails.
async fn store_without_accounts_table(name: &str) -> AccountStore {
    let path = common::fresh_db(name);
    let store = AccountStore::open(&path).await.unwrap();
    common::execute_sql(&path, "DROP TABLE accounts");
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
