//! Tests that run the built `family-app` binary from outside. They cover startup
//! with `FAMILY_APP_DATA_DIR`, and `GET /healthz` and the note round trip over a
//! real socket.

mod common;

use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use common::TempDir;
use serde_json::Value;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

const BIN: &str = env!("CARGO_BIN_EXE_family-app");
const START_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const LISTENING: &str = "family-app: listening on ";

/// Stops the child on drop, so a failing assertion does not leak it. SIGTERM
/// first, so the run still counts towards coverage, then SIGKILL.
struct Running(Child);

impl Running {
    /// Sends a signal by name, such as `TERM` or `INT`, with the shell's `kill`
    /// builtin, since slim images have no `kill` program.
    fn signal(&self, name: &str) -> bool {
        Command::new("sh")
            .args([
                "-c",
                r#"kill -s "$1" "$2""#,
                "sh",
                name,
                &self.0.id().to_string(),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Waits up to `STOP_TIMEOUT` for the app to exit, or `None`.
    fn wait_for_exit(&mut self) -> Option<ExitStatus> {
        let deadline = Instant::now() + STOP_TIMEOUT;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
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
        if self.signal("TERM") && self.wait_for_exit().is_some() {
            return;
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

/// Starts the binary and waits for its whole "listening" line, which comes after the
/// database is open. Panics with the log so far if the app exits first or
/// `START_TIMEOUT` passes; `Running` then kills the child as the panic unwinds.
/// Returns the running app and the log.
fn start(data_dir: &Path, addr: &str) -> (Running, String) {
    let mut running = Running(
        Command::new(BIN)
            .env("FAMILY_APP_DATA_DIR", data_dir)
            .env("FAMILY_APP_ADDR", addr)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + START_TIMEOUT;
    let chunks = read_in_background(running.0.stderr.take().unwrap());
    let mut log = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            panic!(
                "timed out after {START_TIMEOUT:?} waiting for the app to listen; stderr: {}",
                String::from_utf8_lossy(&log)
            );
        }
        match chunks.recv_timeout(remaining) {
            Ok(chunk) => {
                log.extend_from_slice(&chunk);
                let text = String::from_utf8_lossy(&log);
                // The whole line, so the address in it is complete.
                if text
                    .split_once("listening on")
                    .is_some_and(|(_, rest)| rest.contains('\n'))
                {
                    return (running, text.into_owned());
                }
            }
            // Go round again: the check above decides whether time is up.
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => panic!(
                "the app exited before listening; stderr: {}",
                String::from_utf8_lossy(&log)
            ),
        }
    }
}

/// Reads `pipe` to its end on a new thread and sends what it reads. The thread
/// keeps reading after the receiver is gone, so the app's stderr stays open and
/// never fills up. It ends when the app exits.
fn read_in_background(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut chunk = [0; 4096];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = sender.send(chunk[..n].to_vec());
                }
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
    receiver
}

/// Sends raw HTTP bytes over a new connection and returns the whole response.
/// Panics with the partial response if it has not ended within `REQUEST_TIMEOUT`.
fn request(addr: &str, raw: &str) -> String {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(addr) {
            Ok(stream) => break stream,
            Err(err) if Instant::now() > deadline => panic!("connect: {err}"),
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    };
    stream.write_all(raw.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            panic!(
                "timed out after {REQUEST_TIMEOUT:?} reading the response; so far: {}",
                String::from_utf8_lossy(&response)
            );
        }
        stream.set_read_timeout(Some(remaining)).unwrap();
        match stream.read(&mut chunk) {
            Ok(0) => return String::from_utf8_lossy(&response).into_owned(),
            Ok(n) => response.extend_from_slice(&chunk[..n]),
            // Go round again: the check above decides whether time is up.
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(err) => panic!(
                "reading the response: {err}; so far: {}",
                String::from_utf8_lossy(&response)
            ),
        }
    }
}

/// Runs `check` against the database file, opened read-only so the check cannot
/// create or migrate anything itself.
fn with_read_only_db(path: &Path, check: impl AsyncFnOnce(&SqlitePool)) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let options = SqliteConnectOptions::new().filename(path).read_only(true);
        let pool = SqlitePool::connect_with(options).await.unwrap();
        check(&pool).await;
        pool.close().await;
    });
}

/// The JSON body of a response.
fn json_body(response: &str) -> Value {
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

#[test]
fn unset_data_dir_exits_non_zero_naming_the_variable() {
    let output = Command::new(BIN)
        .env_remove("FAMILY_APP_DATA_DIR")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FAMILY_APP_DATA_DIR"), "stderr: {stderr}");
}

#[test]
fn missing_data_dir_exits_non_zero_naming_the_path() {
    let dir = TempDir::new("missing");
    let missing = dir.path().join("absent");
    let output = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", &missing)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&missing.display().to_string()),
        "stderr: {stderr}"
    );
    assert!(!missing.exists());
}

#[test]
fn valid_data_dir_serves_healthz_and_creates_notes_table() {
    let dir = TempDir::new("valid");
    let addr = free_addr();
    let (child, log) = start(dir.path(), &addr);
    assert!(log.contains("family.db"), "stderr: {log}");

    let response = request(
        &addr,
        "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    drop(child);

    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    assert!(response.ends_with("ok"), "response: {response}");

    let db_path = dir.path().join("family.db");
    assert!(db_path.is_file());
    with_read_only_db(&db_path, async |pool| {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let success: bool =
            sqlx::query_scalar("SELECT success FROM _sqlx_migrations WHERE version = 1")
                .fetch_one(pool)
                .await
                .unwrap();
        assert!(success);
    });
}

#[test]
fn written_note_reads_back_over_the_socket_and_is_in_family_db() {
    let dir = TempDir::new("round-trip");
    let addr = free_addr();
    let (child, _) = start(dir.path(), &addr);

    let payload = r#"{"body": "over the socket"}"#;
    let created = request(
        &addr,
        &format!(
            "POST /notes HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        ),
    );
    assert!(created.starts_with("HTTP/1.1 201"), "response: {created}");
    let note = json_body(&created);
    assert_eq!(note["body"], "over the socket");

    let read = request(
        &addr,
        &format!(
            "GET /notes/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            note["id"]
        ),
    );
    drop(child);
    assert!(read.starts_with("HTTP/1.1 200"), "response: {read}");
    assert_eq!(json_body(&read), note);

    // The row is in the data directory's file, not some other database.
    with_read_only_db(&dir.path().join("family.db"), async |pool| {
        let body: String = sqlx::query_scalar("SELECT body FROM notes WHERE id = ?")
            .bind(note["id"].as_i64().unwrap())
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(body, "over the socket");
    });
}

#[test]
fn empty_data_dir_is_treated_as_unset() {
    let output = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FAMILY_APP_DATA_DIR"), "stderr: {stderr}");
}

/// The address from the log's "listening on" line.
fn listening_addr(log: &str) -> &str {
    log.lines()
        .find_map(|line| line.strip_prefix(LISTENING))
        .unwrap_or_else(|| panic!("no listening line; stderr: {log}"))
}

/// Sends `signal` and checks that the app exits with status 0 in time.
fn assert_stops_cleanly_on(mut running: Running, signal: &str) {
    assert!(running.signal(signal), "could not send SIG{signal}");
    let status = running
        .wait_for_exit()
        .unwrap_or_else(|| panic!("the app did not stop on SIG{signal}"));
    assert!(status.success(), "{status}");
}

#[test]
fn logs_the_bound_port_serves_healthz_and_stops_cleanly_on_sigterm() {
    let dir = TempDir::new("sigterm");
    let (running, log) = start(dir.path(), "127.0.0.1:0");

    let addr = listening_addr(&log);
    assert!(addr.starts_with("127.0.0.1:"), "stderr: {log}");
    assert!(
        !addr.ends_with(":0"),
        "the log shows the requested port: {log}"
    );
    let response = request(
        addr,
        "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    assert_stops_cleanly_on(running, "TERM");
}

#[test]
fn stops_cleanly_on_ctrl_c() {
    let dir = TempDir::new("sigint");
    let (running, _) = start(dir.path(), "127.0.0.1:0");

    assert_stops_cleanly_on(running, "INT");
}

#[test]
fn address_in_use_exits_with_status_1_naming_the_address() {
    let dir = TempDir::new("in-use");
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap().to_string();

    let output = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", dir.path())
        .env("FAMILY_APP_ADDR", &addr)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("cannot listen on {addr}")),
        "stderr: {stderr}"
    );
}
