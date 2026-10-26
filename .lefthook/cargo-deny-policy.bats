#!/usr/bin/env bats
# Real cargo-deny checks against local crates; no registry or advisory downloads.
bats_require_minimum_version 1.5.0

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
    cd "$BATS_TEST_TMPDIR"
    cp "$REPO_ROOT/deny.toml" .
    mkdir -p src dependency/src
    printf '' > src/lib.rs
    printf '' > dependency/src/lib.rs
    cat > Cargo.toml <<'TOML'
[package]
name = "private-workspace"
version = "0.1.0"
edition = "2024"
license = "AGPL-3.0-only"
publish = false
[dependencies]
third-party = { path = "dependency" }
TOML
    cat > dependency/Cargo.toml <<'TOML'
[package]
name = "third-party"
version = "0.1.0"
edition = "2024"
license = "MIT"
TOML
    cargo generate-lockfile --offline
}

@test "private AGPL workspace with permissive dependencies passes" {
    run cargo deny --locked --offline check licenses
    [ "$status" -eq 0 ]
}

@test "removing MIT from the allowlist rejects an MIT dependency" {
    sed -i '/"MIT",/d' deny.toml
    run cargo deny --locked --offline check licenses
    [ "$status" -ne 0 ]
    [[ "$output" == *"license is not explicitly allowed"* ]]
}

@test "AGPL third-party dependencies are not exempted with the workspace" {
    sed -i 's/license = "MIT"/license = "AGPL-3.0-only"/' dependency/Cargo.toml
    run cargo deny --locked --offline check licenses
    [ "$status" -ne 0 ]
    [[ "$output" == *"license is not explicitly allowed"* ]]
}
