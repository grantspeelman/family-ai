# family-app

The family app: the Rust service that will run on each family's Machine, with SQLite on that Machine's volume. So far it has a health check, its SQLite database (`sqlx`, bundled), and two endpoints that write a note to that database and read it back.

## Run

```sh
mkdir -p /tmp/family-data
FAMILY_APP_DATA_DIR=/tmp/family-data cargo run -p family-app
```

`FAMILY_APP_DATA_DIR` is required and names the data directory, the mount point of the family's volume. It has no default, and the app does not create the directory, so family data is never written anywhere but the volume. If the variable is unset, the directory is missing, or the database cannot be opened or migrated, the app prints a one-line `family-app:` error to stderr and exits non-zero without serving.

It listens on `127.0.0.1:8080`. Set `FAMILY_APP_ADDR` to listen elsewhere, for example `FAMILY_APP_ADDR=127.0.0.1:9001`. It logs the address it bound, so a port of `0` shows the port the OS picked. Ctrl-C or SIGTERM stops it with exit code 0. Any startup or server failure logs one line and exits with code 1.

## Endpoints

- `GET /healthz` returns `200` with the body `ok`.
- `POST /notes` stores a note.
- `GET /notes/{id}` reads a note from the database.

### `POST /notes`

The request needs the header `Content-Type: application/json` and this body:

```json
{"body": "hello"}
```

`body` is any string, including an empty one. Other fields are ignored.

The response is the stored row, with `Content-Type: application/json`. SQLite assigns `id` and `created_at`, which is UTC in the form `YYYY-MM-DD HH:MM:SS`:

```json
{"id": 1, "body": "hello", "created_at": "2026-01-31 09:30:00"}
```

| Status | When |
| --- | --- |
| `201` | The note was stored. |
| `400` | The JSON is invalid. |
| `415` | The `Content-Type` header is missing or not JSON. |
| `422` | `body` is missing or not a string. |
| `500` | A database error. |

The `400`, `415` and `422` bodies are plain text describing the problem, not JSON. The `500` body is the plain text `internal server error`.

No row is written unless the status is `201`.

### `GET /notes/{id}`

The response has the same JSON and `Content-Type` as the write.

| Status | When |
| --- | --- |
| `200` | The note exists. |
| `400` | `id` is not a 64-bit integer. |
| `404` | No note has this `id`. |
| `500` | A database error. |

The `400` body is plain text describing the problem, not JSON. The `404` body is empty. The `500` body is the plain text `internal server error`.

### Example

With the app running on its default address:

```sh
curl -i -X POST http://127.0.0.1:8080/notes \
  -H 'Content-Type: application/json' \
  -d '{"body": "hello"}'
curl -i http://127.0.0.1:8080/notes/1
```

Use the `id` from the `POST` response in place of `/notes/1`.

## Database

On start, before it listens, the app opens or creates `family.db` in `FAMILY_APP_DATA_DIR` and applies pending migrations (`family_app::db::open`). Migrations run through sqlx's migrator. Each one runs once, in its own transaction, and sqlx records it in the `_sqlx_migrations` table. A restart reuses the same file and data.

### Adding a migration

Add a file to `migrations/` named with the next number and a short description, for example `migrations/0002_add_tags.sql`. `sqlx::migrate!` embeds the files at compile time, and `build.rs` makes Cargo rebuild when `migrations/` changes. Never edit or rename a migration that has shipped: sqlx stores each one's checksum and refuses to start if a file has changed.

Migration 1 (`migrations/0001_create_notes.sql`) creates the one table:

```sql
notes(id INTEGER PRIMARY KEY, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)
```

## Tests

```sh
cargo test --locked -p family-app
```

`tests/health.rs` and `tests/notes.rs` send requests straight into the router, the latter against a real database. `tests/storage.rs` opens databases in fresh temp directories. `tests/binary.rs` starts the built binary and checks its startup errors, `GET /healthz` and a note written and read back over the socket. CI runs them as part of the workspace-wide `cargo test --locked` in the `registry` job.
