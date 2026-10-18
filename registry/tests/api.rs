//! In-process tests: requests go straight into the router, no socket.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn send(request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = registry::app().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

fn graphql(query: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({ "query": query }).to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn api_version_is_the_crate_version() {
    let (status, body) = send(graphql("{ apiVersion }")).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["apiVersion"], env!("CARGO_PKG_VERSION"));
    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
}

#[tokio::test]
async fn healthz_returns_ok() {
    let request = Request::get("/healthz").body(Body::empty()).unwrap();
    let (status, body) = send(request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"ok");
}

#[tokio::test]
async fn unknown_path_is_not_found() {
    let request = Request::get("/nope").body(Body::empty()).unwrap();
    let (status, _) = send(request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn graphql_rejects_get() {
    // No GraphiQL or playground: GET /graphql serves nothing.
    let request = Request::get("/graphql").body(Body::empty()).unwrap();
    let (status, _) = send(request).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[test]
fn schema_file_matches_the_code() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/schema.graphql");
    let generated = registry::schema().sdl();
    if std::env::var("UPDATE_SCHEMA").as_deref() == Ok("1") {
        std::fs::write(path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(path).unwrap_or_default();
    assert_eq!(
        committed, generated,
        "registry/schema.graphql is out of date. Regenerate it with: \
         UPDATE_SCHEMA=1 cargo test -p registry --test api schema_file_matches_the_code"
    );
}
