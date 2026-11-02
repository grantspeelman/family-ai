//! Reading the database path from configuration, with a fake environment.

use std::{ffi::OsString, path::PathBuf};

use registry::accounts::db_path_from;

#[test]
fn returns_the_path_when_the_variable_is_set() {
    let path = db_path_from(|name| {
        assert_eq!(name, "REGISTRY_DB_PATH");
        Some(OsString::from("/data/registry.db"))
    });

    assert_eq!(path.unwrap(), PathBuf::from("/data/registry.db"));
}

#[test]
fn unset_is_an_error_that_names_the_variable() {
    let error = db_path_from(|_| None).unwrap_err();

    assert!(
        error.to_string().contains("REGISTRY_DB_PATH"),
        "got {error}"
    );
}

#[test]
fn empty_is_an_error_that_names_the_variable() {
    let error = db_path_from(|_| Some(OsString::new())).unwrap_err();

    assert!(
        error.to_string().contains("REGISTRY_DB_PATH"),
        "got {error}"
    );
}
