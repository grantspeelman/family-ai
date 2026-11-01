# fly-mock

The Fly mock: a local stand-in for Fly's Machines API, written in Rust (axum). Local dev and CI talk to it instead of real Fly, so they never need a Fly token. It answers a health check and Fly's create-app, create-volume and create-Machine calls, keeping everything in memory. Each start begins with an empty store.

## Run it

From the repo root:

```sh
cargo run -p fly-mock
```

It listens on `127.0.0.1:4280`, the port Fly's own Machines API uses on its private network, so it does not clash with the registry on `8080`. Set `FLY_MOCK_ADDR` to change that, for example `FLY_MOCK_ADDR=0.0.0.0:9001 cargo run -p fly-mock`. It prints the address it is listening on. A port of `0` lets the OS pick a free port. Ctrl-C or SIGTERM stops it with exit code 0.

## How it is used

fly-mock runs as its own process, started with `cargo run -p fly-mock` and with its address set by `FLY_MOCK_ADDR`. It stands in for Fly's Machines API in end-to-end tests and local dev. The registry reaches it the same way it reaches real Fly: through its Fly API base URL, set to the mock's address. No crate depends on it, and registry tests do not link against it.

## Endpoints

Paths and JSON follow Fly's Machines API (base `https://api.machines.dev`), so pointing a client at real Fly changes only the base URL. The mock keeps only the fields listed here. It accepts requests with or without an `Authorization` header.

| Endpoint | Request body | Response |
| --- | --- | --- |
| `GET /healthz` | none | `200` with the body `ok`. |
| `POST /v1/apps` | `{"app_name": "...", "org_slug": "..."}` | `201 Created`, empty body. Records the app. |
| `POST /v1/apps/{app_name}/volumes` | `{"name": "...", "region": "...", "size_gb": <int>}` | `200` with a Volume: `id` (`vol_` and a unique suffix), `name`, `region`, `size_gb`, `state` (`"created"`). |
| `POST /v1/apps/{app_name}/machines` | `{"name"?: "...", "region"?: "...", "config": {"image": "...", "env"?: {...}, "mounts"?: [{"volume": "vol_...", "path": "..."}]}}` | `200` with a Machine: `id` (14 lowercase hex characters, unique), `name` (from the request, or generated), `state` (`"created"`), `region` (from the request, or `mock`), `private_ip` (a fake, unique IPv6 address in `fdaa::/16`; nothing listens on it), `config` (the request's `config`, echoed back). |

Any other path returns `404`.

### Errors

Errors use Fly's error body, `{"error": "<message>"}`:

- `404` when the app in the path was never created (volumes and Machines).
- `404` when a Machine's `mounts` name a volume the app does not have.

Malformed request bodies get axum's default rejection, not Fly's error shape.

```sh
curl -s http://127.0.0.1:4280/healthz
# ok
curl -s -XPOST http://127.0.0.1:4280/v1/apps -H 'content-type: application/json' \
  -d '{"app_name": "fam-1", "org_slug": "personal"}' -w '%{http_code}\n'
# 201
```

## Tests

```sh
cargo test --locked -p fly-mock
```

`tests/health.rs` and `tests/machines_api.rs` send requests straight into the router, with no network. `tests/server.rs` runs the built binary, queries it over TCP and stops it with a signal. In CI, `cargo test --locked` at the workspace root runs both. Run `cargo deny --locked check` from the repo root to check dependency licenses, advisories, bans and sources for every workspace crate.
