//! Helpers shared by the test files.

// Each test file compiles this module separately and uses only some helpers.
#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Request, State},
    http::{Method, StatusCode, header},
};
use http_body_util::BodyExt;
use registry::{
    accounts::AccountStore,
    fly::{FlyClient, FlyConfig},
    provisioning::Provisioner,
};
use serde_json::{Value, json};
use tower::ServiceExt;

const LISTENING: &str = "fly-mock: listening on ";
/// Long enough for `cargo run` to build the mock on a cold cache.
const MOCK_START_TIMEOUT: Duration = Duration::from_secs(300);
const MOCK_STOP_TIMEOUT: Duration = Duration::from_secs(5);
/// The machine port the mock is started with. Not 8080, so a server on this
/// host's wildcard address can't block it.
pub const MOCK_MACHINE_PORT: u16 = 18082;

/// A database path no other test uses, with nothing left over from an earlier run.
pub fn fresh_db(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.db"));
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not clear {}: {error}", path.display()),
    }
    path
}

/// Opens a store on a fresh database file.
pub async fn fresh_store(name: &str) -> AccountStore {
    AccountStore::open(&fresh_db(name)).await.unwrap()
}

/// A running `fly-mock` process, killed on drop.
pub struct Mock {
    child: Child,
    pub base_url: String,
    data_dir: PathBuf,
}

impl Mock {
    pub fn start() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let built = Command::new(&cargo)
            .args(["build", "--quiet", "--locked", "-p", "family-app"])
            .current_dir(manifest_dir)
            .status()
            .expect("cannot run cargo");
        assert!(built.success(), "cannot build family-app");
        let data_dir = std::env::temp_dir().join(format!(
            "registry-fly-mock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut child = Command::new(cargo)
            .args(["run", "--quiet", "--locked", "-p", "fly-mock"])
            .current_dir(manifest_dir)
            .env("FLY_MOCK_ADDR", "127.0.0.1:0")
            .env("FLY_MOCK_DATA_DIR", &data_dir)
            // Not 8080, so a server on this host's wildcard address can't block it.
            .env("FLY_MOCK_MACHINE_PORT", MOCK_MACHINE_PORT.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cannot run cargo");
        let stderr = child.stderr.take().unwrap();
        let (addr_tx, addr_rx) = mpsc::channel();
        // Keep reading until the process exits, so stderr never fills up.
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                match line.strip_prefix(LISTENING) {
                    Some(addr) => {
                        let _ = addr_tx.send(addr.to_owned());
                    }
                    // Shows why, if cargo cannot build or run the mock.
                    None => eprintln!("{line}"),
                }
            }
        });
        let addr = addr_rx
            .recv_timeout(MOCK_START_TIMEOUT)
            .expect("fly-mock did not start");
        Self {
            child,
            base_url: format!("http://{addr}"),
            data_dir,
        }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        // `cargo run` replaces itself with the mock, so this signals the mock.
        // `kill` through the shell, as a builtin: slim images have no program.
        let _ = Command::new("sh")
            .args(["-c", r#"kill "$1""#, "sh", &self.child.id().to_string()])
            .status();
        let deadline = Instant::now() + MOCK_STOP_TIMEOUT;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

/// A base URL on which nothing listens.
pub async fn closed_port_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

/// A provisioner that sends to `base_url`, with machine port `port`.
pub fn provisioner_at(base_url: &str, port: u16, time_limit: Duration) -> Provisioner {
    let config = FlyConfig::new(base_url, None).unwrap();
    Provisioner::new(FlyClient::new(config).unwrap(), port).with_time_limit(time_limit)
}

/// A provisioner whose Fly API cannot be reached, so every account it starts
/// ends up failed.
pub fn provisioner() -> Option<Provisioner> {
    // Port 9 (discard) on loopback is closed in the dev container and CI.
    Some(provisioner_at(
        "http://127.0.0.1:9",
        8080,
        Duration::from_secs(5),
    ))
}

/// Sends a GraphQL request into the router and returns the JSON response.
pub async fn graphql(app: Router, query: &str, variables: Value) -> Value {
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "query": query, "variables": variables }).to_string(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// A request a stub saw.
#[derive(Debug, Clone)]
pub struct Seen {
    pub path: String,
    pub body: Bytes,
}

pub type Log = Arc<Mutex<Vec<Seen>>>;

/// How a stub answers one request: a status and a body, or never.
#[derive(Clone)]
pub enum Reply {
    With(u16, String),
    Never,
}

/// Starts a server that records each request and answers it with `reply`,
/// given the request's path. Returns its base URL and the record.
pub async fn stub(reply: impl Fn(&str) -> Reply + Send + Sync + 'static) -> (String, Log) {
    type Replier = Arc<dyn Fn(&str) -> Reply + Send + Sync>;
    async fn handle(
        State((reply, log)): State<(Replier, Log)>,
        request: Request,
    ) -> (StatusCode, String) {
        let (parts, body) = request.into_parts();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let path = parts.uri.path().to_owned();
        log.lock().unwrap().push(Seen {
            path: path.clone(),
            body,
        });
        match reply(&path) {
            Reply::With(status, body) => (StatusCode::from_u16(status).unwrap(), body),
            Reply::Never => std::future::pending().await,
        }
    }

    let log = Log::default();
    let replier: Replier = Arc::new(reply);
    let app = Router::new()
        .fallback(handle)
        .with_state((replier, log.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), log)
}
