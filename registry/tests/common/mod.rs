//! Helpers shared by the test files.

// Each test file compiles this module separately and uses only some helpers.
#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
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
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
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

/// Runs `sql` on the database file at `path` through its own connection, for
/// example to break the store under a running registry.
pub async fn execute_sql(path: &Path, sql: &str) {
    let options = SqliteConnectOptions::new().filename(path);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::raw_sql(sql).execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
}

/// Makes every later `UPDATE` of an account fail, while reads and inserts
/// still work. Run it on a database the store has already migrated.
pub const BLOCK_ACCOUNT_UPDATES: &str = "CREATE TRIGGER block_updates BEFORE UPDATE ON accounts \
     BEGIN SELECT RAISE(ABORT, 'updates are blocked'); END";

const REGISTRY_LISTENING: &str = "registry: listening on ";
const REGISTRY_START_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a stop signal gets to end the registry.
pub const REGISTRY_STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// The registry binary with port 0 and no provisioning settings, so each test
/// sets only what it is about.
pub fn registry_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_registry"));
    command
        .env("REGISTRY_ADDR", "127.0.0.1:0")
        .env_remove("FLY_API_HOSTNAME")
        .env_remove("FLY_API_TOKEN")
        .env_remove("REGISTRY_FAMILY_APP_PORT");
    command
}

/// A running registry binary with its stderr collected. Dropping it sends
/// SIGTERM, so the run still counts towards coverage, and falls back to SIGKILL.
pub struct Running {
    child: Child,
    pub base_url: String,
    stderr: Arc<Mutex<String>>,
}

impl Running {
    /// Starts `command` and waits for its "listening on" line.
    pub fn start(mut command: Command) -> Self {
        let mut child = command.stderr(Stdio::piped()).spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let collected = Arc::new(Mutex::new(String::new()));
        let (addr_tx, addr_rx) = mpsc::channel();
        let sink = collected.clone();
        // Keep reading until the process exits, so stderr never fills up.
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                {
                    let mut sink = sink.lock().unwrap();
                    sink.push_str(&line);
                    sink.push('\n');
                }
                if let Some(addr) = line.strip_prefix(REGISTRY_LISTENING) {
                    let _ = addr_tx.send(addr.to_owned());
                }
            }
        });
        let Ok(addr) = addr_rx.recv_timeout(REGISTRY_START_TIMEOUT) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the registry did not start; stderr: {}",
                collected.lock().unwrap()
            );
        };
        Self {
            child,
            base_url: format!("http://{addr}"),
            stderr: collected,
        }
    }

    /// Sends a GraphQL request over HTTP and returns the JSON response.
    pub async fn graphql(&self, query: &str, variables: Value) -> Value {
        reqwest::Client::new()
            .post(format!("{}/graphql", self.base_url))
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// Waits up to a few seconds for stderr to contain `text`.
    pub async fn wait_for(&self, text: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stderr = self.stderr();
            if stderr.contains(text) || Instant::now() > deadline {
                return stderr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Everything the registry has written to stderr so far.
    pub fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }

    /// Sends a signal by name, such as `TERM` or `INT`.
    pub fn signal(&self, name: &str) {
        assert!(
            send_signal(name, self.child.id()),
            "could not send SIG{name}"
        );
    }

    /// Waits up to `timeout` for the process to exit, or `None`.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return Some(status);
            }
            if Instant::now() > deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        send_signal("TERM", self.child.id());
        if self.wait_timeout(REGISTRY_STOP_TIMEOUT).is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Sends a signal by name with the shell's `kill` builtin, since slim images
/// have no `kill` program. Returns whether it was sent.
pub fn send_signal(name: &str, pid: u32) -> bool {
    Command::new("sh")
        .args(["-c", r#"kill -s "$1" "$2""#, "sh", name, &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
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
        send_signal("TERM", self.child.id());
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
