//! In-process tests: requests go straight into the router, no socket.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn get(path: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::get(path).body(Body::empty()).unwrap();
    let response = fly_mock::app().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

#[tokio::test]
async fn healthz_returns_ok() {
    let (status, body) = get("/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"ok");
}

#[tokio::test]
async fn unknown_path_is_not_found() {
    let (status, _) = get("/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
