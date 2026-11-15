//! `fly_mock::app()` reads its settings from the environment. The process
//! environment is shared by every test in a binary, so the bad-setting case
//! runs this test binary again, as a child with its own environment.

use std::process::Command;

/// Set only in the child run.
const CHILD: &str = "FLY_MOCK_TEST_APP_FROM_ENV_CHILD";
const TEST: &str = "a_bad_machine_port_makes_app_panic_naming_the_variable";

#[test]
fn a_bad_machine_port_makes_app_panic_naming_the_variable() {
    if std::env::var_os(CHILD).is_some() {
        drop(fly_mock::app());
        return;
    }

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("FLY_MOCK_MACHINE_PORT", "not-a-port")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("FLY_MOCK_MACHINE_PORT is not a port from 1 to 65535"),
        "stderr: {stderr}"
    );
}
