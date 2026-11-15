#!/usr/bin/env bash
# Fails when the files that name the Rust version disagree. A single source is not
# practical: Cargo.toml's rust-version must be a literal, Dependabot reads the
# registry image's FROM tag, and a rust-toolchain.toml would make rustup in that
# image download a second toolchain. Usage: rust-version-check.sh [repo root]
set -uo pipefail

root="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
status=0

# read_pin LABEL FILE SED_SCRIPT: prints the version, or reports it and fails.
read_pin() {
    local version
    version="$(sed -nE "$3" "$root/$2" 2>/dev/null | head -n 1)"
    if [ -z "$version" ]; then
        echo "could not read the Rust version ($1) from $2" >&2
        return 1
    fi
    printf '%s\n' "$version"
}

devcontainer="$(read_pin RUST_VERSION .devcontainer/Dockerfile \
    's/^ARG RUST_VERSION=([0-9.]+)[[:space:]]*$/\1/p')" || status=1
registry="$(read_pin 'rust image tag' registry/Dockerfile \
    's/^FROM rust:([0-9.]+)-[^ ]+ .*$/\1/p')" || status=1
workspace="$(read_pin rust-version Cargo.toml \
    's/^rust-version = "([0-9.]+)"[[:space:]]*$/\1/p')" || status=1

[ "$status" -eq 0 ] || exit 1

echo ".devcontainer/Dockerfile RUST_VERSION: $devcontainer"
echo "registry/Dockerfile rust image tag:    $registry"
echo "Cargo.toml rust-version:               $workspace"

if [ "$devcontainer" != "$registry" ] || [ "$devcontainer" != "$workspace" ]; then
    echo "The Rust versions differ. Bump them together." >&2
    exit 1
fi
