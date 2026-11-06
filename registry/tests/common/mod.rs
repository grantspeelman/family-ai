//! Helpers shared by the test files.

// Each test file compiles this module separately and uses only some helpers.
#![allow(dead_code)]

use std::path::PathBuf;

use registry::accounts::AccountStore;

/// A database path no other test uses, with nothing left over from an earlier run.
pub fn fresh_db(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.db"));
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not clear {}: {error}", path.display()),
    }
    path
}

/// Opens a store on a fresh database file.
pub async fn fresh_store(name: &str) -> AccountStore {
    AccountStore::open(&fresh_db(name)).await.unwrap()
}
