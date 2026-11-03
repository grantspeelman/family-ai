//! Process tests: run the `fly-mock` binary as `cargo run -p fly-mock` would,
//! talk to it over TCP and stop it with a signal.
#![cfg(unix)]

mod common;

use common::{
    TempDir, assert_healthy, assert_stops_accepting, family_app_bin, json_body, machine_addr,
    machine_body, raw_http,
};
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(20);
const LISTENING: &str = "fly-mock: listening on ";

/// A running `fly-mock` binary and the first line it wrote to stderr.
struct Server {
    child: Child,
    first_line: String,
}

impl Server {
    /// Starts the binary with `FLY_MOCK_ADDR` set to `addr`, or unset for `None`.
    fn start(addr: Option<&str>) -> Self {
        Self::start_with(addr, &[])
    }

    /// Like `start`, with the other `FLY_MOCK_*` variables unset except `envs`.
    fn start_with(addr: Option<&str>, envs: &[(&str, &OsStr)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fly-mock"));
        command
            .env_remove("FLY_MOCK_ADDR")
            .env_remove("FLY_MOCK_FAMILY_APP_BIN")
            .env_remove("FLY_MOCK_DATA_DIR")
            .env_remove("FLY_MOCK_MACHINE_PORT")
            .stderr(Stdio::piped());
        if let Some(addr) = addr {
            command.env("FLY_MOCK_ADDR", addr);
        }
        command.envs(envs.iter().copied());
        let mut child = command.spawn().expect("cannot start fly-mock");
        let stderr = child.stderr.take().unwrap();
        let (lines, first) = mpsc::channel();
        // Keep reading until the process exits, so stderr never fills up.
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = lines.send(line);
            }
        });
        let first_line = first.recv_timeout(TIMEOUT).unwrap_or_default();
        Self { child, first_line }
    }

    /// The address from the "listening on" line.
    fn addr(&self) -> &str {
        self.first_line
            .strip_prefix(LISTENING)
            .unwrap_or_else(|| panic!("fly-mock did not start: {:?}", self.first_line))
    }

    /// Sends a signal by name (`TERM`, `INT`) with the shell's `kill`.
    fn signal(&self, name: &str) {
        let pid = self.child.id().to_string();
        let status = Command::new("sh")
            .args(["-c", r#"kill -s "$1" "$2""#, "sh", name, &pid])
            .status()
            .unwrap();
        assert!(status.success(), "kill -s {name} {pid} failed");
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "fly-mock did not exit");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn get(addr: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn serves_healthz_on_fly_mock_addr_and_stops_cleanly_on_sigterm() {
    let mut server = Server::start(Some("127.0.0.1:0"));
    let addr = server.addr().to_owned();
    assert_ne!(addr, "127.0.0.1:4280", "FLY_MOCK_ADDR was ignored");

    let response = get(&addr, "/healthz");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.ends_with("\r\n\r\nok"), "{response}");

    server.signal("TERM");
    assert_eq!(server.wait().code(), Some(0));
}

#[test]
fn stops_cleanly_on_ctrl_c() {
    let mut server = Server::start(Some("127.0.0.1:0"));
    server.addr();
    server.signal("INT");
    assert_eq!(server.wait().code(), Some(0));
}

#[test]
fn uses_the_default_address_without_fly_mock_addr() {
    // The port may be in use on this machine, so either line shows the address.
    let server = Server::start(None);
    let line = &server.first_line;
    assert!(
        line == "fly-mock: listening on 127.0.0.1:4280"
            || line.starts_with("fly-mock: cannot listen on 127.0.0.1:4280: "),
        "unexpected first line: {line:?}"
    );
}

#[test]
fn exits_with_an_error_on_a_bad_address() {
    let mut server = Server::start(Some("not-an-address"));
    assert!(
        server
            .first_line
            .starts_with("fly-mock: cannot listen on not-an-address: "),
        "unexpected first line: {:?}",
        server.first_line
    );
    assert_eq!(server.wait().code(), Some(1));
}

/// Creates an app, a volume and a Machine over raw HTTP. Returns the volume id
/// and the Machine.
fn create_machine(addr: &str) -> (String, Value) {
    let app = json!({"name": "fam", "org_slug": "personal"}).to_string();
    let response = raw_http(addr, "POST", "/v1/apps", &app);
    assert!(response.starts_with("HTTP/1.1 201 "), "{response}");
    let volume = json!({"name": "data", "region": "jnb", "size_gb": 1}).to_string();
    let response = raw_http(addr, "POST", "/v1/apps/fam/volumes", &volume);
    let volume = json_body(&response)["id"].as_str().unwrap().to_owned();
    let body = machine_body(&volume).to_string();
    let response = raw_http(addr, "POST", "/v1/apps/fam/machines", &body);
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    (volume, json_body(&response))
}

#[test]
fn sigterm_stops_the_family_apps_and_exits_zero() {
    let data = TempDir::new();
    let bin = family_app_bin();
    let mut server = Server::start_with(
        Some("127.0.0.1:0"),
        &[
            ("FLY_MOCK_FAMILY_APP_BIN", bin.as_os_str()),
            ("FLY_MOCK_DATA_DIR", data.path().as_os_str()),
            ("FLY_MOCK_MACHINE_PORT", OsStr::new("18081")),
        ],
    );
    let (volume, machine) = create_machine(server.addr());
    assert_eq!(machine["state"], "started");
    let app_addr = machine_addr(&machine, 18081);
    assert_healthy(&app_addr);
    assert!(data.path().join(&volume).join("family.db").is_file());

    server.signal("TERM");
    assert_eq!(server.wait().code(), Some(0));
    assert_stops_accepting(&app_addr);
}

#[test]
fn defaults_find_the_binary_beside_the_mock_and_a_new_temp_directory() {
    // Only a binary built next to the mock can be found by default.
    if std::env::var_os("FLY_MOCK_FAMILY_APP_BIN").is_some() {
        return;
    }
    family_app_bin();
    // Something on this host's wildcard address and 8080 would block the default port.
    if std::net::TcpListener::bind("0.0.0.0:8080").is_err() {
        return;
    }
    let tmp = TempDir::new();
    let mut server = Server::start_with(Some("127.0.0.1:0"), &[("TMPDIR", tmp.path().as_os_str())]);
    let (volume, machine) = create_machine(server.addr());
    assert_healthy(&machine_addr(&machine, 8080));
    let roots: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
    assert_eq!(roots.len(), 1, "{roots:?}");
    let root = roots.into_iter().next().unwrap().unwrap().path();
    assert!(root.join(volume).join("family.db").is_file());
    server.signal("TERM");
    assert_eq!(server.wait().code(), Some(0));
}

#[test]
fn bad_machine_port_is_a_startup_error() {
    for port in ["not-a-port", "0"] {
        let mut server = Server::start_with(
            Some("127.0.0.1:0"),
            &[("FLY_MOCK_MACHINE_PORT", OsStr::new(port))],
        );
        assert!(
            server.first_line.contains("FLY_MOCK_MACHINE_PORT"),
            "unexpected first line: {:?}",
            server.first_line
        );
        assert_eq!(server.wait().code(), Some(1));
    }
}
