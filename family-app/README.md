# family-app

The family app: the Rust service that will run on each family's Machine, with SQLite on that Machine's volume. So far it is a scaffold with a health check. Its SQLite (`sqlx`, bundled) and JSON (`serde`, `serde_json`) dependencies are declared for later work but not used yet.

## Run

```sh
cargo run -p family-app
```

It listens on `127.0.0.1:8080`. Set `FAMILY_APP_ADDR` to listen elsewhere, for example `FAMILY_APP_ADDR=127.0.0.1:9001`. It stops cleanly on Ctrl-C or SIGTERM.

## Endpoints

- `GET /healthz` returns `200` with the body `ok`.

## Tests

```sh
cargo test --locked -p family-app
```

The tests in `tests/` send requests straight into the router, with no network. CI runs them as part of the workspace-wide `cargo test --locked` in the `registry` job.
