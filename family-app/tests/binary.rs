//! Tests of the `family-app` binary's startup with `FAMILY_APP_DATA_DIR`.

mod common;

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, ChildStderr, Command, Stdio},
    time::{Duration, Instant},
};

use common::TempDir;
use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_family-app");

/// Kills the child on drop, so a failing assertion does not leak it.
struct Running {
    child: Child,
    // Held open so the app's own stderr writes keep working while it runs.
    _stderr: BufReader<ChildStderr>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

/// Starts the binary and waits for its "listening" line, which comes after the
/// database is open. Returns the running app and its stderr so far.
fn start(data_dir: &Path, addr: &str) -> (Running, String) {
    let mut child = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", data_dir)
        .env("FAMILY_APP_ADDR", addr)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut log = String::new();
    loop {
        let mut line = String::new();
        if stderr.read_line(&mut line).unwrap() == 0 {
            break;
        }
        log.push_str(&line);
        if line.contains("listening on") {
            break;
        }
    }
    let running = Running {
        child,
        _stderr: stderr,
    };
    (running, log)
}

/// Sends raw HTTP bytes over a new connection and returns the whole response.
fn request(addr: &str, raw: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect(addr) {
            Ok(stream) => break stream,
            Err(err) if Instant::now() > deadline => panic!("connect: {err}"),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    stream.write_all(raw.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
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
    // Read-only, so this check cannot create or migrate anything itself.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db_path)
            .read_only(true);
        let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let success: bool =
            sqlx::query_scalar("SELECT success FROM _sqlx_migrations WHERE version = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(success);
        pool.close().await;
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
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.path().join("family.db"))
            .read_only(true);
        let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
        let body: String = sqlx::query_scalar("SELECT body FROM notes WHERE id = ?")
            .bind(note["id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(body, "over the socket");
        pool.close().await;
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
