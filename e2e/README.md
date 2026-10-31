# e2e

The end-to-end suite. A test-only crate (no binary, not published) that runs the registry, the Fly mock and the family app side by side.

`e2e::Services` (in `src/lib.rs`) starts the three built binaries as child processes, each on a free local port set through its listen-address variable (`REGISTRY_ADDR`, `FLY_MOCK_ADDR`, `FAMILY_APP_ADDR`), with a fresh temporary `FAMILY_APP_DATA_DIR`. It waits up to 20 seconds for each `/healthz`, and fails with an error naming the service if one does not start or answer in time. It gives back each base URL (`registry_url()`, `fly_mock_url()`, `family_app_url()`) and kills every child when dropped, also when a test panics. New test files in `tests/` can reuse it.

## Tests

- `tests/health.rs`: all three services answer their health check. It needs the built binaries, so it is `#[ignore]`d and the workspace's plain `cargo test` skips it.
- `tests/harness.rs`: the harness's failure paths, with stand-in scripts. These run in any `cargo test`.

## Run locally

From the repo root:

```sh
cargo build --locked -p registry -p fly-mock -p family-app
cargo test --locked -p e2e -- --include-ignored
```

The harness looks for the binaries in `E2E_BIN_DIR` if set, else `$CARGO_TARGET_DIR/debug`, else the workspace's `target/debug`. CI runs the same two commands in the `e2e` job of `.github/workflows/devcontainer-ci.yml`.
