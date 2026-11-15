#!/usr/bin/env bats
# Tests rust-version-check.sh against fixture trees, and the repo's own files.
# Needs no Docker or network.
bats_require_minimum_version 1.5.0

setup() {
    CHECK="$BATS_TEST_DIRNAME/rust-version-check.sh"
    FIXTURE="$BATS_TEST_TMPDIR/repo"
    mkdir -p "$FIXTURE/.devcontainer" "$FIXTURE/registry"
    printf 'ARG RUST_VERSION=1.93\n' > "$FIXTURE/.devcontainer/Dockerfile"
    printf 'FROM rust:1.93-slim-trixie AS toolchain\n' > "$FIXTURE/registry/Dockerfile"
    printf '[workspace.package]\nrust-version = "1.93"\n' > "$FIXTURE/Cargo.toml"
}

@test "the repo's own pins match" {
    run "$CHECK" "$BATS_TEST_DIRNAME/.."
    [ "$status" -eq 0 ]
}

@test "matching pins pass" {
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 0 ]
}

@test "a different dev container version fails" {
    printf 'ARG RUST_VERSION=1.94\n' > "$FIXTURE/.devcontainer/Dockerfile"
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 1 ]
    [[ "$output" == *"1.94"* ]]
}

@test "a different registry image tag fails" {
    printf 'FROM rust:1.94-slim-trixie AS toolchain\n' > "$FIXTURE/registry/Dockerfile"
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 1 ]
}

@test "a different workspace rust-version fails" {
    printf 'rust-version = "1.94"\n' > "$FIXTURE/Cargo.toml"
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 1 ]
}

@test "a pin that cannot be read fails" {
    printf 'FROM rust:latest AS toolchain\n' > "$FIXTURE/registry/Dockerfile"
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 1 ]
    [[ "$output" == *"could not read"* ]]
}

@test "a missing file fails" {
    rm "$FIXTURE/Cargo.toml"
    run "$CHECK" "$FIXTURE"
    [ "$status" -eq 1 ]
}
