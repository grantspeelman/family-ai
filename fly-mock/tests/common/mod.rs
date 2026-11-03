//! Helpers shared by the tests that boot real family app processes. Each test
//! file uses only some of them.
#![allow(dead_code, reason = "each test crate uses a different subset")]

use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use fly_mock::Settings;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// The machine port for tests. Not 8080, so a server on this host's wildcard
/// address and port 8080 cannot get in the way.
pub const MACHINE_PORT: u16 = 18080;

/// How long a stopped process's address gets to stop accepting connections.
const STOP_LIMIT: Duration = Duration::from_secs(5);

/// A fresh directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("fly-mock-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The real family app binary: `FLY_MOCK_FAMILY_APP_BIN`, or `family-app` in
/// the Cargo target directory of this test executable.
pub fn family_app_bin() -> PathBuf {
    let path = std::env::var_os("FLY_MOCK_FAMILY_APP_BIN").map_or_else(
        || {
            // The test executable is <target>/debug/deps/<test>-<hash>.
            let exe = std::env::current_exe().unwrap();
            exe.parent().unwrap().parent().unwrap().join("family-app")
        },
        PathBuf::from,
    );
    assert!(
        path.is_file(),
        "no family app binary at {}: run `cargo build -p family-app` first",
        path.display()
    );
    path
}

/// A mock settings value with a generous time limit, using `data_root`.
pub fn settings(binary: impl Into<PathBuf>, data_root: &Path) -> Settings {
    Settings {
        family_app_bin: binary.into(),
        data_root: data_root.to_path_buf(),
        machine_port: MACHINE_PORT,
        start_timeout: Duration::from_secs(10),
    }
}

/// Runs the shell's `kill` with these arguments. It is a shell builtin, so this
/// works on hosts with no `kill` program, such as slim container images.
pub fn shell_kill(args: &[&str]) -> bool {
    Command::new("sh")
        .args(["-c", r#"kill "$@""#, "sh"])
        .args(args)
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// A POST straight into the router, with no contract checks.
pub async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).expect("JSON body"))
}

/// Creates the app `fam` with one volume and returns the volume's id.
pub async fn app_with_volume(app: &Router) -> String {
    let (status, _) = post(
        app,
        "/v1/apps",
        json!({"name": "fam", "org_slug": "personal"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, volume) = post(
        app,
        "/v1/apps/fam/volumes",
        json!({"name": "data", "region": "jnb", "size_gb": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    volume["id"].as_str().unwrap().to_owned()
}

/// A create-Machine body mounting `volume`.
pub fn machine_body(volume: &str) -> Value {
    json!({"config": {"image": "ignored", "mounts": [{"volume": volume, "path": "/data"}]}})
}

/// Sends one raw HTTP/1.1 request and returns the whole response.
pub fn raw_http(addr: &str, method: &str, path: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// The JSON body of a raw HTTP response.
pub fn json_body(response: &str) -> Value {
    let (_, body) = response.split_once("\r\n\r\n").expect("no response body");
    serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {response}"))
}

/// The `<private_ip>:<port>` address of a Machine response.
pub fn machine_addr(machine: &Value, port: u16) -> String {
    format!("{}:{port}", machine["private_ip"].as_str().unwrap())
}

/// Checks that the address runs the family app, so `/healthz` answers `ok`.
pub fn assert_healthy(addr: &str) {
    let response = raw_http(addr, "GET", "/healthz", "");
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    assert!(response.ends_with("\r\n\r\nok"), "{response}");
}

/// Checks that a Machine's address is loopback and not `127.0.0.1`.
pub fn assert_private_loopback(machine: &Value) {
    let ip: IpAddr = machine["private_ip"].as_str().unwrap().parse().unwrap();
    assert!(ip.is_loopback(), "{ip}");
    assert!(ip.is_ipv4(), "{ip}");
    assert_ne!(ip.to_string(), "127.0.0.1");
}

/// Waits for the address to stop accepting connections.
pub fn assert_stops_accepting(addr: &str) {
    let deadline = Instant::now() + STOP_LIMIT;
    while TcpStream::connect(addr).is_ok() {
        assert!(
            Instant::now() < deadline,
            "{addr} still accepts connections"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
