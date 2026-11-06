//! The `createAccount` mutation, through the HTTP app, on real database files.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use registry::accounts::AccountStore;
use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::Path;
use tower::ServiceExt;

mod common;

const MUTATION: &str = "mutation($email: String!, $familyName: String!) { \
    createAccount(email: $email, familyName: $familyName) { id email familyName } }";

async fn create(store: &AccountStore, email: &str, family_name: &str) -> Value {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "query": MUTATION, "variables": { "email": email, "familyName": family_name } })
                .to_string(),
        ))
        .unwrap();
    let response = registry::app(store.clone(), common::provisioner())
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn id_of(json: &Value) -> i64 {
    json["data"]["createAccount"]["id"]
        .as_str()
        .expect("id is a string")
        .parse()
        .expect("id is numeric")
}

/// Checks the response is one error with `code`, null data, and no echo of the inputs.
fn assert_error(json: &Value, code: &str, email: &str, family_name: &str) {
    assert!(json["data"].is_null(), "data should be null: {json}");
    let errors = json["errors"].as_array().expect("errors");
    assert_eq!(errors.len(), 1, "{json}");
    assert_eq!(errors[0]["extensions"]["code"], code, "{json}");
    let message = errors[0]["message"].as_str().unwrap();
    assert!(!message.is_empty());
    for input in [email.trim(), family_name.trim()] {
        if !input.is_empty() {
            assert!(!message.contains(input), "message echoes input: {message}");
        }
    }
}

/// Counts the rows in the database file directly.
async fn account_count(path: &Path) -> i64 {
    let options = SqliteConnectOptions::new().filename(path);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&mut connection)
        .await
        .unwrap()
}

#[tokio::test]
async fn creates_an_account_that_the_store_returns() {
    let store = common::fresh_store("create-account-ok").await;

    let json = create(&store, "ana@example.com", "Silva").await;

    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
    let account = &json["data"]["createAccount"];
    assert_eq!(account["email"], "ana@example.com");
    assert_eq!(account["familyName"], "Silva");
    let stored = store.get(id_of(&json)).await.unwrap().expect("stored");
    assert_eq!(stored.email, "ana@example.com");
    assert_eq!(stored.family_name, "Silva");
}

#[tokio::test]
async fn inputs_are_trimmed() {
    let store = common::fresh_store("create-account-trim").await;

    let json = create(&store, " ana@example.com ", " Silva ").await;

    assert_eq!(json["data"]["createAccount"]["email"], "ana@example.com");
    assert_eq!(json["data"]["createAccount"]["familyName"], "Silva");
    let stored = store.get(id_of(&json)).await.unwrap().unwrap();
    assert_eq!(stored.email, "ana@example.com");
    assert_eq!(stored.family_name, "Silva");
}

#[tokio::test]
async fn bad_emails_are_rejected_and_nothing_is_stored() {
    let path = common::fresh_db("create-account-bad-email");
    let store = AccountStore::open(&path).await.unwrap();
    let long = format!("{}@example.com", "a".repeat(255 - "@example.com".len()));
    assert_eq!(long.len(), 255);
    let bad = [
        "",
        "ana",
        "ana@",
        "@example.com",
        "ana@@example.com",
        "a@b@example.com",
        "ana@example",
        "ana@.com",
        "ana@example.",
        "an a@example.com",
        long.as_str(),
    ];

    for email in bad {
        let json = create(&store, email, "Silva").await;
        assert_error(&json, "INVALID_EMAIL", email, "Silva");
    }
    assert_eq!(account_count(&path).await, 0);
}

#[tokio::test]
async fn a_254_character_email_is_accepted() {
    let store = common::fresh_store("create-account-254-email").await;
    let email = format!("{}@example.com", "a".repeat(254 - "@example.com".len()));

    let json = create(&store, &email, "Silva").await;

    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
}

#[tokio::test]
async fn bad_family_names_are_rejected_and_nothing_is_stored() {
    let path = common::fresh_db("create-account-bad-name");
    let store = AccountStore::open(&path).await.unwrap();
    let long = "x".repeat(201);

    for name in ["", "   ", long.as_str()] {
        let json = create(&store, "ana@example.com", name).await;
        assert_error(&json, "INVALID_FAMILY_NAME", "ana@example.com", name);
    }
    assert_eq!(account_count(&path).await, 0);
}

#[tokio::test]
async fn a_200_character_family_name_is_accepted() {
    let store = common::fresh_store("create-account-200-name").await;
    let name = "x".repeat(200);

    let json = create(&store, "ana@example.com", &name).await;

    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
    assert_eq!(json["data"]["createAccount"]["familyName"], name.as_str());
}

#[tokio::test]
async fn a_taken_email_is_rejected_in_any_letter_case() {
    let store = common::fresh_store("create-account-taken").await;
    let first = create(&store, "ana@example.com", "Silva").await;
    let first_id = id_of(&first);

    for email in ["ana@example.com", "Ana@Example.com"] {
        let json = create(&store, email, "Costa").await;
        assert_error(&json, "EMAIL_TAKEN", email, "Costa");
    }

    let stored = store.get(first_id).await.unwrap().unwrap();
    assert_eq!(stored.email, "ana@example.com");
    assert_eq!(stored.family_name, "Silva");
}
