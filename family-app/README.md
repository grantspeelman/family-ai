# family-app

The family app: the Rust service that will run on each family's Machine, with SQLite on that Machine's volume. So far it has a health check and its SQLite database (`sqlx`, bundled). Its JSON dependencies (`serde`, `serde_json`) are declared for later work.

## Run

```sh
mkdir -p /tmp/family-data
FAMILY_APP_DATA_DIR=/tmp/family-data cargo run -p family-app
```

`FAMILY_APP_DATA_DIR` is required and names the data directory, the mount point of the family's volume. It has no default, and the app does not create the directory, so family data is never written anywhere but the volume. If the variable is unset, the directory is missing, or the database cannot be opened or migrated, the app prints a one-line `family-app:` error to stderr and exits non-zero without serving.

It listens on `127.0.0.1:8080`. Set `FAMILY_APP_ADDR` to listen elsewhere, for example `FAMILY_APP_ADDR=127.0.0.1:9001`. It stops cleanly on Ctrl-C or SIGTERM.

## Endpoints

- `GET /healthz` returns `200` with the body `ok`.

## Database

On start, before it listens, the app opens or creates `family.db` in `FAMILY_APP_DATA_DIR` and applies pending migrations (`family_app::db::open`). Each migration runs once, in its own transaction, and records the schema version in `PRAGMA user_version`. Migrations are listed in `src/db.rs`; append new ones, never edit a shipped one. A restart reuses the same file and data.

Schema version 1 has one table:

```sql
notes(id INTEGER PRIMARY KEY, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)
```

## Tests

```sh
cargo test --locked -p family-app
```

`tests/health.rs` sends requests straight into the router. `tests/storage.rs` opens databases in fresh temp directories. `tests/binary.rs` starts the built binary and checks its startup errors and `GET /healthz`. CI runs them as part of the workspace-wide `cargo test --locked` in the `registry` job.
