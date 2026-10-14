# 01 — Dev container

**Status:** in progress

## Goal

One container holds the whole toolchain, so the registry app, the Fly mock and the family app can run and talk to each other over localhost. It gives the agent a fast dev loop. It does not imitate production's Firecracker isolation.

## Builds on

Nothing. This is the first plan.

## In scope

- Land the existing `.devcontainer/` (Dockerfile, compose files, start and entrypoint scripts, smoke test, container-structure test, README) after review against this plan.
- A single container, not one per service, with Rust and the tools the registry app will need.
- Entry from a terminal (`start.sh`) and from VS Code, sharing one Claude login volume.
- A `.gitignore` that keeps local secrets out of the repo, including `.devcontainer/.env`.

## Out of scope

- The CI workflow that builds and tests the image (plan 02).
- Hook installation beyond what `postStartCommand.sh` already does (plan 03).
- Registry app tooling, `cargo-llvm-cov` and `cargo-deny` (PRs 2 and 7).

## Acceptance checklist

- [x] Local credential files (including `.devcontainer/.env`) are gitignored and not in this PR. `git ls-files .devcontainer` shows no file containing a credential, and `gitleaks` over the tracked files is clean.
- [ ] Rotate any token previously stored in `.devcontainer/.env` and replace the file's contents (owner action, not done by this PR).
- [x] A `.env.example` (no values) documents what the container expects from the host environment.
- [ ] `.devcontainer/start.sh` builds and enters the container from a clean checkout.
- [ ] `.devcontainer/smoke-test.sh` passes.
- [ ] `container-structure-test` passes against the built image.
- [ ] `cargo`, `rustc`, `clippy` and `sqlite3` run inside the container.
- [x] The README matches what the container actually does (reach, privileges, toolchain).

## Review depth

**Close read** of anything touching secrets, SSH, `sudo` and the privileged `--docker` mode, since the repo is public. **Skim** the Dockerfile boilerplate and test configs.
