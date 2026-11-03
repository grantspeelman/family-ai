//! In-process tests: requests go straight into the router, no socket.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::TempDir;

async fn send(name: &str, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let dir = TempDir::new(name);
    let pool = family_app::db::open(dir.path()).await.unwrap();
    let (status, _, body) = common::send(family_app::app(pool.clone()), request).await;
    pool.close().await;
    (status, body)
}

#[tokio::test]
async fn healthz_returns_ok() {
    let request = Request::get("/healthz").body(Body::empty()).unwrap();
    let (status, body) = send("healthz", request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"ok");
}

#[tokio::test]
async fn unknown_path_is_not_found() {
    let request = Request::get("/nope").body(Body::empty()).unwrap();
    let (status, _) = send("unknown-path", request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
