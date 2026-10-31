//! The harness's failure paths, with stand-in shell scripts for the binaries.
//! These need no built services, so they run in a plain `cargo test`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use e2e::Services;

fn fake_bin_dir(test: &str, script: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("e2e-harness-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("registry");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn pid_alive(pid_file: &Path) -> bool {
    let pid = std::fs::read_to_string(pid_file).unwrap();
    Path::new("/proc").join(pid.trim()).exists()
}

#[tokio::test]
async fn unhealthy_service_times_out_naming_it_and_is_killed() {
    let dir = fake_bin_dir(
        "timeout",
        "#!/bin/sh\necho $$ > \"$(dirname \"$0\")/pid\"\nexec sleep 60\n",
    );
    let started = Instant::now();
    let err = match Services::start_from(&dir, Duration::from_millis(500)).await {
        Ok(_) => panic!("a service that never answers must not be reported healthy"),
        Err(err) => err,
    };
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
    assert!(err.starts_with("registry: did not answer"), "{err}");
    assert!(
        !pid_alive(&dir.join("pid")),
        "the registry child was left running"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn service_that_exits_early_is_named() {
    let dir = fake_bin_dir("exit", "#!/bin/sh\nexit 3\n");
    let err = match Services::start_from(&dir, Duration::from_secs(10)).await {
        Ok(_) => panic!("a service that exits must not be reported healthy"),
        Err(err) => err,
    };
    assert!(
        err.starts_with("registry: exited before it was healthy"),
        "{err}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn missing_binary_is_named() {
    let err = match Services::start_from(Path::new("/nonexistent-e2e-bin"), Duration::from_secs(1))
        .await
    {
        Ok(_) => panic!("a missing binary must fail"),
        Err(err) => err,
    };
    assert!(err.starts_with("registry: starting"), "{err}");
}
