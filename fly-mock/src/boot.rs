//! Starting a Machine: the family app as a child process, on its own loopback
//! address, with its volume mapped to a local directory.
//!
//! This crate does not declare tokio's `time` or `io-util` features, so waiting
//! uses short `std::thread::sleep`s, and the health check blocking `std` I/O
//! with timeouts, both on the blocking pool.

use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::process::{Child, Command};

/// The family app's own default port.
const DEFAULT_MACHINE_PORT: u16 = 8080;
const DEFAULT_START_TIMEOUT: Duration = Duration::from_secs(10);
/// The pause between health checks, and the longest single sleep.
const TICK: Duration = Duration::from_millis(25);
/// A status line longer than this is not a healthy family app.
const MAX_STATUS_LINE: u64 = 256;
/// The longest one health check may block for, at each of connect, write and
/// read. A check that times out fails, and the next one starts after a `TICK`.
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(1);

/// Passed from the mock to the family app when set: the coverage output path,
/// so child runs count in the `coverage` CI job, and panic backtraces.
const PASSED_THROUGH_ENV: [&str; 2] = ["LLVM_PROFILE_FILE", "RUST_BACKTRACE"];

/// What the mock needs to start family apps.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The family app binary.
    pub family_app_bin: PathBuf,
    /// Holds a directory per volume.
    pub data_root: PathBuf,
    /// The port every Machine's family app listens on.
    pub machine_port: u16,
    /// How long a family app gets to become healthy.
    pub start_timeout: Duration,
}

impl Settings {
    /// Reads `FLY_MOCK_FAMILY_APP_BIN`, `FLY_MOCK_DATA_DIR` and
    /// `FLY_MOCK_MACHINE_PORT`, using the defaults for any that are unset or
    /// empty. The time limit is the default.
    ///
    /// # Errors
    ///
    /// If `FLY_MOCK_MACHINE_PORT` is not a port from 1 to 65535.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            family_app_bin: env_var("FLY_MOCK_FAMILY_APP_BIN")
                .map_or_else(default_family_app_bin, PathBuf::from),
            data_root: env_var("FLY_MOCK_DATA_DIR").map_or_else(new_temp_root, PathBuf::from),
            machine_port: machine_port_from(env_var("FLY_MOCK_MACHINE_PORT"))?,
            start_timeout: DEFAULT_START_TIMEOUT,
        })
    }
}

fn env_var(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

fn machine_port_from(value: Option<OsString>) -> Result<u16, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_MACHINE_PORT);
    };
    value
        .to_str()
        .and_then(|text| text.parse().ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| format!("FLY_MOCK_MACHINE_PORT is not a port from 1 to 65535: {value:?}"))
}

/// `family-app` in the same directory as this executable.
fn default_family_app_bin() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default();
    dir.join("family-app")
}

/// A path for a new directory under the temp dir. It is created with the first
/// volume directory, so a mock that starts no Machine leaves nothing behind.
fn new_temp_root() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("fly-mock-{}-{n}-{nanos}", std::process::id()))
}

/// Hands out addresses within this mock instance. An address is the low 24 bits
/// of `127.x.y.z`: 10 bits of process id, 8 of block and 6 of slot. Each
/// instance takes a block from a per-process counter, so instances in a process
/// differ by block, and processes differ by id. A process id can match another
/// modulo 1024, and the blocks can wrap, so an address in use is skipped.
struct Addresses {
    block: u32,
    slot: u32,
}

/// Slots 0, 1 and 63 are never used, so no address is `127.0.0.0`, `127.0.0.1`
/// or `127.255.255.255`.
const FIRST_SLOT: u32 = 2;
const LAST_SLOT: u32 = 62;
/// How many addresses to try for one Machine before giving up.
const MAX_ADDRESS_TRIES: u32 = 256;

fn next_block() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Addresses {
    fn new() -> Self {
        Self {
            block: next_block(),
            slot: FIRST_SLOT,
        }
    }

    /// The next address with `port` free.
    fn allocate(&mut self, port: u16) -> Result<Ipv4Addr, String> {
        for _ in 0..MAX_ADDRESS_TRIES {
            if self.slot > LAST_SLOT {
                self.block = next_block();
                self.slot = FIRST_SLOT;
            }
            let ip = address(std::process::id(), self.block, self.slot);
            self.slot += 1;
            if std::net::TcpListener::bind((ip, port)).is_ok() {
                return Ok(ip);
            }
        }
        Err(format!(
            "no free loopback address for port {port}; is something listening on 0.0.0.0:{port}?"
        ))
    }
}

fn address(pid: u32, block: u32, slot: u32) -> Ipv4Addr {
    let low = ((pid & 0x3ff) << 14) | ((block & 0xff) << 6) | (slot & 0x3f);
    Ipv4Addr::from((127 << 24) | low)
}

/// Plans new Machines' processes.
pub(crate) struct Booter {
    settings: Settings,
    addresses: Addresses,
}

impl Booter {
    pub(crate) fn new(settings: Settings) -> Self {
        Self {
            settings,
            addresses: Addresses::new(),
        }
    }

    /// Picks the address and paths of a Machine mounting `volume`.
    pub(crate) fn plan(&mut self, volume: &str) -> Result<Plan, String> {
        let ip = self.addresses.allocate(self.settings.machine_port)?;
        Ok(Plan {
            addr: SocketAddrV4::new(ip, self.settings.machine_port),
            binary: self.settings.family_app_bin.clone(),
            data_dir: self.settings.data_root.join(volume),
            start_timeout: self.settings.start_timeout,
        })
    }
}

/// One Machine's process, not started yet.
pub(crate) struct Plan {
    addr: SocketAddrV4,
    binary: PathBuf,
    data_dir: PathBuf,
    start_timeout: Duration,
}

impl Plan {
    pub(crate) fn ip(&self) -> Ipv4Addr {
        *self.addr.ip()
    }
}

/// A running family app. Dropping this kills the process.
pub(crate) struct Process {
    _child: Child,
}

/// Starts the process and waits for it to be healthy. On failure, the process
/// is stopped and the message says what happened.
pub(crate) async fn start(plan: Plan) -> Result<Process, String> {
    std::fs::create_dir_all(&plan.data_dir).map_err(|err| {
        format!(
            "cannot create volume directory {}: {err}",
            plan.data_dir.display()
        )
    })?;
    // `kill_on_drop` also covers this future being dropped, as when the client
    // goes away during the wait.
    let mut child = Command::new(&plan.binary)
        .env_clear()
        .envs(PASSED_THROUGH_ENV.iter().filter_map(|name| {
            std::env::var_os(name).map(|value| (name, value))
        }))
        .env("FAMILY_APP_ADDR", plan.addr.to_string())
        .env("FAMILY_APP_DATA_DIR", &plan.data_dir)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| {
            format!(
                "cannot start family app binary {}: {err}; build it with `cargo build -p family-app` or set FLY_MOCK_FAMILY_APP_BIN",
                plan.binary.display()
            )
        })?;
    let deadline = Instant::now() + plan.start_timeout;
    let failure = tokio::select! {
        () = poll_until_healthy(plan.addr.into()) => return Ok(Process { _child: child }),
        // If waiting fails, this branch is disabled and the time limit applies.
        Ok(status) = child.wait() => format!(
            "family app exited before it was healthy: {status}; see the mock's output"
        ),
        () = sleep_until(deadline) => format!(
            "family app not healthy at {} within {:?}",
            plan.addr, plan.start_timeout
        ),
    };
    // A process that already exited makes this a no-op.
    let _ = child.kill().await;
    Err(failure)
}

async fn poll_until_healthy(addr: SocketAddr) {
    while !healthz_ok(addr).await {
        sleep(TICK).await;
    }
}

/// Whether the family app at `addr` answers `GET /healthz` with a `200`.
/// Runs on the blocking pool, like `sleep`, and is bounded by
/// `HEALTH_CHECK_TIMEOUT` so shutting down the runtime never waits long.
async fn healthz_ok(addr: SocketAddr) -> bool {
    tokio::task::spawn_blocking(move || check_healthz(addr).unwrap_or(false))
        .await
        .unwrap_or(false)
}

/// Sends `GET /healthz` over plain HTTP/1.1 and checks for a `200` status line.
/// Reads up to the first line end, the end of the stream or `MAX_STATUS_LINE`.
fn check_healthz(addr: SocketAddr) -> std::io::Result<bool> {
    let mut stream = TcpStream::connect_timeout(&addr, HEALTH_CHECK_TIMEOUT)?;
    stream.set_write_timeout(Some(HEALTH_CHECK_TIMEOUT))?;
    stream.set_read_timeout(Some(HEALTH_CHECK_TIMEOUT))?;
    let request = format!("GET /healthz HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    let mut line = Vec::new();
    BufReader::new(stream.take(MAX_STATUS_LINE)).read_until(b'\n', &mut line)?;
    Ok(line.starts_with(b"HTTP/1.1 200 "))
}

async fn sleep_until(deadline: Instant) {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        sleep(left.min(TICK)).await;
    }
}

/// Sleeps on the blocking pool. Kept short, because shutting down the runtime
/// waits for a running sleep.
async fn sleep(duration: Duration) {
    let _ = tokio::task::spawn_blocking(move || std::thread::sleep(duration)).await;
}
