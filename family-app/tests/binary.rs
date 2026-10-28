//! Tests of the `family-app` binary's startup with `FAMILY_APP_DATA_DIR`.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_family-app");

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("family-app-binary-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

#[test]
fn unset_data_dir_exits_non_zero_naming_the_variable() {
    let output = Command::new(BIN)
        .env_remove("FAMILY_APP_DATA_DIR")
        .env("FAMILY_APP_ADDR", free_addr())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FAMILY_APP_DATA_DIR"), "stderr: {stderr}");
}

#[test]
fn missing_data_dir_exits_non_zero_naming_the_path() {
    let dir = TempDir::new("missing");
    let missing = dir.0.join("absent");
    let output = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", &missing)
        .env("FAMILY_APP_ADDR", free_addr())
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
    let mut child = Command::new(BIN)
        .env("FAMILY_APP_DATA_DIR", &dir.0)
        .env("FAMILY_APP_ADDR", &addr)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Wait for the "listening" line, which comes after the database is open.
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
    assert!(log.contains("family.db"), "stderr: {log}");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect(&addr) {
            Ok(stream) => break stream,
            Err(err) if Instant::now() > deadline => panic!("connect: {err}"),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    assert!(response.ends_with("ok"), "response: {response}");

    let db_path = dir.0.join("family.db");
    assert!(db_path.is_file());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = family_app::db::open(&dir.0).await.unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        pool.close().await;
    });
}
