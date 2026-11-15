//! The registry binary's provisioning config, startup step and log lines.

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use registry::accounts::{AccountStore, Status};
use serde_json::{Value, json};

mod common;

const TOKEN_MARKER: &str = "token-marker-7f3e9c";
const EMAIL: &str = "zed.provisioning@example.com";
const FAMILY: &str = "Quixotic-Familyname";
const LISTENING: &str = "registry: listening on ";

fn registry() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_registry"));
    command
        .env("REGISTRY_ADDR", "127.0.0.1:0")
        .env("FLY_API_TOKEN", TOKEN_MARKER)
        .env_remove("FLY_API_HOSTNAME")
        .env_remove("REGISTRY_FAMILY_APP_PORT");
    command
}

fn assert_clean_failure(output: &Output, variable: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "panicked: {stderr}");
    assert!(stderr.contains(variable), "stderr: {stderr}");
    assert!(!stderr.contains(TOKEN_MARKER), "token in stderr: {stderr}");
}

#[test]
fn an_invalid_fly_hostname_exits_naming_the_variable() {
    let db = common::fresh_db("binary-bad-hostname");
    for value in ["not a url", "ftp://example.com"] {
        let output = registry()
            .env("REGISTRY_DB_PATH", &db)
            .env("FLY_API_HOSTNAME", value)
            .output()
            .unwrap();
        assert_clean_failure(&output, "FLY_API_HOSTNAME");
    }
}

#[test]
fn a_bad_machine_port_exits_naming_the_variable() {
    let db = common::fresh_db("binary-bad-port");
    for value in ["abc", "0", "70000"] {
        let output = registry()
            .env("REGISTRY_DB_PATH", &db)
            .env("FLY_API_HOSTNAME", "http://127.0.0.1:9")
            .env("REGISTRY_FAMILY_APP_PORT", value)
            .output()
            .unwrap();
        assert_clean_failure(&output, "REGISTRY_FAMILY_APP_PORT");
    }
}

/// A running registry binary, killed on drop, with its stderr collected.
struct Running {
    child: Child,
    base_url: String,
    stderr: Arc<Mutex<String>>,
}

impl Running {
    fn start(mut command: Command) -> Self {
        let mut child = command.stderr(Stdio::piped()).spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let collected = Arc::new(Mutex::new(String::new()));
        let (addr_tx, addr_rx) = mpsc::channel();
        let sink = collected.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                {
                    let mut sink = sink.lock().unwrap();
                    sink.push_str(&line);
                    sink.push('\n');
                }
                if let Some(addr) = line.strip_prefix(LISTENING) {
                    let _ = addr_tx.send(addr.to_owned());
                }
            }
        });
        let addr = addr_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the registry did not start");
        Self {
            child,
            base_url: format!("http://{addr}"),
            stderr: collected,
        }
    }

    async fn graphql(&self, query: &str, variables: Value) -> Value {
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
    async fn wait_for(&self, text: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stderr = self.stderr();
            if stderr.contains(text) || Instant::now() > deadline {
                return stderr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn startup_fails_interrupted_accounts_and_failures_log_no_personal_data() {
    let db = common::fresh_db("binary-run");
    let store = AccountStore::open(&db).await.unwrap();
    let interrupted = store.create("old@example.com", "Old").await.unwrap();
    drop(store);
    let mut command = registry();
    command
        .env("REGISTRY_DB_PATH", &db)
        .env("FLY_API_HOSTNAME", common::closed_port_url().await);
    let running = Running::start(command);

    let status = running
        .graphql(
            "query($id: ID!) { accountStatus(id: $id) { status address } }",
            json!({ "id": interrupted.id.to_string() }),
        )
        .await;
    assert_eq!(status["data"]["accountStatus"]["status"], "FAILED");

    let created = running
        .graphql(
            "mutation($e: String!, $f: String!) { createAccount(email: $e, familyName: $f) { id } }",
            json!({ "e": EMAIL, "f": FAMILY }),
        )
        .await;
    let id: i64 = created["data"]["createAccount"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let store = AccountStore::open(&db).await.unwrap();
    while store.get(id).await.unwrap().unwrap().status == Status::Provisioning {
        assert!(Instant::now() < deadline, "still provisioning");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let line = format!("provisioning account {id} failed");
    let stderr = running.wait_for(&line).await;

    assert!(stderr.contains(&line), "stderr: {stderr}");
    let checks = [
        ("the new email", EMAIL),
        ("the new family name", FAMILY),
        ("the token marker", TOKEN_MARKER),
        ("the interrupted account's email", "old@example.com"),
    ];
    for (label, value) in checks {
        assert!(!stderr.contains(value), "{label} is in stderr");
    }
}

#[test]
fn without_fly_the_registry_starts_and_says_provisioning_is_disabled() {
    let mut command = registry();
    command.env("REGISTRY_DB_PATH", common::fresh_db("binary-no-fly"));
    let running = Running::start(command);

    let stderr = running.stderr();

    assert!(stderr.contains("provisioning is disabled"), "{stderr}");
    assert!(!stderr.contains(TOKEN_MARKER), "{stderr}");
}
