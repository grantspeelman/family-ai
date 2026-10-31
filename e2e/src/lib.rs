//! A harness that runs the registry, the Fly mock and the family app together.
//!
//! [`Services::start`] runs the three built binaries as child processes, each on
//! a free local port, waits for each `/healthz`, and gives back their base URLs.
//! Dropping [`Services`] kills every child, also when a test panics.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long each service has to answer `/healthz` after it starts.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(20);

/// One running service binary. Killed and reaped on drop.
struct Running {
    child: Child,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A temporary directory removed on drop.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The three services, running and healthy.
pub struct Services {
    registry_url: String,
    fly_mock_url: String,
    family_app_url: String,
    // Children before the data directory, so the family app stops before its
    // directory is removed.
    _children: Vec<Running>,
    _family_app_data: TempDir,
}

impl Services {
    /// Starts all three services and waits for each to be healthy. The error
    /// names the service that failed to start or did not become healthy.
    pub async fn start() -> Result<Self, String> {
        Self::start_from(&bin_dir(), HEALTH_TIMEOUT).await
    }

    /// Like [`Services::start`], with the binaries taken from `bin_dir` and
    /// `timeout` for each health check.
    pub async fn start_from(bin_dir: &Path, timeout: Duration) -> Result<Self, String> {
        let s = Starter { bin_dir, timeout };
        let data = temp_dir()?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|err| format!("building the HTTP client: {err}"))?;
        let mut children = Vec::new();

        let registry_url = s
            .start_one(&client, &mut children, "registry", "REGISTRY_ADDR", &[])
            .await?;
        let fly_mock_url = s
            .start_one(&client, &mut children, "fly-mock", "FLY_MOCK_ADDR", &[])
            .await?;
        let data_env = [("FAMILY_APP_DATA_DIR", data.0.as_os_str())];
        let family_app_url = s
            .start_one(
                &client,
                &mut children,
                "family-app",
                "FAMILY_APP_ADDR",
                &data_env,
            )
            .await?;

        Ok(Self {
            registry_url,
            fly_mock_url,
            family_app_url,
            _children: children,
            _family_app_data: data,
        })
    }

    /// The registry's base URL, such as `http://127.0.0.1:41234`.
    pub fn registry_url(&self) -> &str {
        &self.registry_url
    }

    /// The Fly mock's base URL.
    pub fn fly_mock_url(&self) -> &str {
        &self.fly_mock_url
    }

    /// The family app's base URL.
    pub fn family_app_url(&self) -> &str {
        &self.family_app_url
    }
}

struct Starter<'a> {
    bin_dir: &'a Path,
    timeout: Duration,
}

impl Starter<'_> {
    /// Starts one binary on a free port and waits for its `/healthz`. The child
    /// is pushed to `children` before waiting, so it is killed on any failure.
    async fn start_one(
        &self,
        client: &reqwest::Client,
        children: &mut Vec<Running>,
        name: &str,
        addr_var: &str,
        extra_env: &[(&str, &std::ffi::OsStr)],
    ) -> Result<String, String> {
        let port = free_port().map_err(|err| format!("{name}: finding a free port: {err}"))?;
        let addr = format!("127.0.0.1:{port}");
        let binary = self.bin_dir.join(name);
        let mut command = Command::new(&binary);
        command.env(addr_var, &addr).stdin(Stdio::null());
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let child = spawn_retrying_busy(&mut command).await.map_err(|err| {
            format!(
                "{name}: starting {}: {err} (build it first, see e2e/README.md)",
                binary.display()
            )
        })?;
        children.push(Running { child });
        let running = children.last_mut().expect("just pushed");

        let base = format!("http://{addr}");
        let health = format!("{base}/healthz");
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Ok(Some(status)) = running.child.try_wait() {
                return Err(format!("{name}: exited before it was healthy ({status})"));
            }
            if let Ok(response) = client.get(&health).send().await
                && response.status().is_success()
            {
                return Ok(base);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "{name}: did not answer {health} within {:.1}s",
                    self.timeout.as_secs_f32()
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Spawns `command`, retrying briefly on "Text file busy" (ETXTBSY), which a
/// just-written executable can hit while a fork elsewhere still holds its
/// write handle.
async fn spawn_retrying_busy(command: &mut Command) -> std::io::Result<Child> {
    const ETXTBSY: i32 = 26;
    let mut attempts = 0;
    loop {
        match command.spawn() {
            Err(err) if err.raw_os_error() == Some(ETXTBSY) && attempts < 50 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            result => return result,
        }
    }
}

/// A port the OS reports free on 127.0.0.1, released for the service to bind.
fn free_port() -> std::io::Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Where the built binaries are: `E2E_BIN_DIR`, else `debug/` under
/// `CARGO_TARGET_DIR`, else the workspace's `target/debug`.
fn bin_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("E2E_BIN_DIR") {
        return dir.into();
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target"));
    target.join("debug")
}

/// A fresh, empty directory for the family app's data.
fn temp_dir() -> Result<TempDir, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("e2e-family-app-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&dir)
        .map_err(|err| format!("family-app: creating {}: {err}", dir.display()))?;
    Ok(TempDir(dir))
}
