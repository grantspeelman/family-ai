//! Process tests: run the `fly-mock` binary as `cargo run -p fly-mock` would,
//! talk to it over TCP and stop it with a signal.
#![cfg(unix)]

use std::{
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_fly-mock"));
        command.env_remove("FLY_MOCK_ADDR").stderr(Stdio::piped());
        if let Some(addr) = addr {
            command.env("FLY_MOCK_ADDR", addr);
        }
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
