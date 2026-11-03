# fly-mock

The Fly mock: a local stand-in for Fly's Machines API, written in Rust (axum). Local dev and CI talk to it instead of real Fly, so they never need a Fly token. It answers a health check and Fly's create-app, create-volume and create-Machine calls, keeping its records in memory. Each start begins with an empty store. Each Machine it creates is the real family app, running as a local child process (see [Machines](#machines)).

## Run it

From the repo root:

```sh
cargo build -p family-app
cargo run -p fly-mock
```

Build the family app first, or set `FLY_MOCK_FAMILY_APP_BIN` to its binary: `cargo run -p fly-mock` builds only the mock, and without the family app create-Machine returns `500`.

It listens on `127.0.0.1:4280`, the port Fly's own Machines API uses on its private network, so it does not clash with the registry on `8080`. Set `FLY_MOCK_ADDR` to change that, for example `FLY_MOCK_ADDR=0.0.0.0:9001 cargo run -p fly-mock`. It prints the address it is listening on. A port of `0` lets the OS pick a free port. Ctrl-C or SIGTERM stops it with exit code 0. The API has no authentication and each create-Machine starts a process, so keep the mock on a loopback address unless the network is trusted.

## How it is used

fly-mock runs as its own process, started with `cargo run -p fly-mock` and with its address set by `FLY_MOCK_ADDR`. It stands in for Fly's Machines API in end-to-end tests and local dev. The registry reaches it the same way it reaches real Fly: through its Fly API base URL, set to the mock's address. No crate depends on it, and registry tests do not link against it: `registry/tests/fly_client.rs` runs it as a process.

## Endpoints

Paths and JSON follow Fly's Machines API (base `https://api.machines.dev`), so pointing a client at real Fly changes only the base URL. `tests/machines_api.rs` checks every request it sends and every response it gets against Fly's OpenAPI spec (see [Contract](#contract)). The mock keeps only the fields listed here and ignores any other request field. It accepts requests with or without an `Authorization` header.

| Endpoint | Request body | Response |
| --- | --- | --- |
| `GET /healthz` | none | `200` with the body `ok`. |
| `POST /v1/apps` | `{"name": "...", "org_slug": "..."}` | `201` with `{"id": "app_...", "created_at": <Unix seconds>}`. Records the app. A name that is already taken returns `422` with `{"error": "...", "status": "name_taken"}`. |
| `POST /v1/apps/{app_name}/volumes` | `{"name": "...", "region": "...", "size_gb": <int>}` | `200` with a Volume: `id` (`vol_` and a unique suffix), `name`, `region`, `size_gb`, `state` (`"created"`). |
| `POST /v1/apps/{app_name}/machines` | `{"name"?: "...", "region"?: "...", "config": {"image": "...", "env"?: {...}, "mounts"?: [{"volume": "vol_...", "path": "..."}]}}` | `200` with a Machine: `id` (14 lowercase hex characters, unique), `name` (from the request, or generated), `state` (`"started"`), `region` (from the request, or `mock`), `private_ip` (a unique loopback address where the family app listens, see [Machines](#machines)), `config` (the request's `config`, echoed back). |

Any other path returns `404`.

## Machines

Create-Machine starts the family app binary as a child process and answers once the app is healthy. `config.image` and `config.env` are ignored: every Machine runs the family app with the mock's own environment, plus `FAMILY_APP_ADDR` and `FAMILY_APP_DATA_DIR`, described below. There are no containers or Firecracker.

| Variable | Default | What |
| --- | --- | --- |
| `FLY_MOCK_ADDR` | `127.0.0.1:4280` | The address the mock itself listens on. |
| `FLY_MOCK_FAMILY_APP_BIN` | `family-app` next to the mock's executable | The family app binary. `target/debug/fly-mock` finds `target/debug/family-app`. Build it first with `cargo build -p family-app`, or set this variable. |
| `FLY_MOCK_DATA_DIR` | A new directory under the system temp directory | The data root, which holds the volume directories. |
| `FLY_MOCK_MACHINE_PORT` | `8080` | The port of every Machine. A value that is not a port from 1 to 65535 stops the mock at startup. |

An empty variable counts as unset. `fly_mock::app()` reads these settings from the environment. `fly_mock::app_with(Settings { .. })` takes the binary path, data root, machine port and time limit explicitly. The time limit is 10 seconds in `app()`.

**Address.** Each Machine gets its own `private_ip`, an IPv4 loopback address that is never `127.0.0.1`, and the mock sets `FAMILY_APP_ADDR` to `<private_ip>:<machine port>`. A caller builds the app's address from `private_ip` and the machine port, as it would on Fly. Linux routes all of `127.0.0.0/8` to loopback, so this needs no setup; other hosts are not supported. The address comes from the mock's process id, a per-process instance counter and a per-instance Machine number, so mocks running at once use separate ranges. The mock skips any address where the machine port is already taken. That also means something listening on `0.0.0.0:<machine port>` blocks every address, and create-Machine fails until the port is freed or `FLY_MOCK_MACHINE_PORT` changes.

**Volume.** The Machine's volume maps to `<data root>/<volume id>`. The mock creates that directory if it is missing and sets `FAMILY_APP_DATA_DIR` to it. The family app takes one data directory, so only the first entry in `config.mounts` is used, and its `path` is not used locally. The mock never deletes a volume's directory, so the data outlives the mock.

**States.** The Machine is `starting` while the mock polls the app's `GET /healthz` for a `200`, for up to the time limit. Create-Machine then returns it as `started`. No endpoint reads a Machine yet, so callers only ever see `started`.

**Failure.** Create-Machine stops the process, if there is one, and returns `500` with `{"error": "<message>"}` when the binary can't be started, the process exits before it is healthy, the time limit passes, the Machine has no mounts, the volume directory can't be created, or no address is free. The message says which. The family app's own output goes to the mock's stdout and stderr, so look there when it exits early.

**Stopping.** Family app processes stop when the mock stops on Ctrl-C or SIGTERM, and when the router and every clone of it are dropped. A SIGKILL can't be caught and leaves them running, so stop the mock with SIGTERM.

### Errors

Errors use Fly's `ErrorResponse` body, `{"error": "<message>"}`, plus `"status"` where Fly sets one:

- `404` when the app in the path was never created (volumes and Machines).
- `404` when a Machine's `mounts` name a volume the app does not have.
- `422` with `"status": "name_taken"` when `POST /v1/apps` names an app that already exists.

Fly's spec lists no `404` for these paths. The mock uses `404` with the same body for an unknown app or volume.

Create-Machine also returns `500` with this body when a Machine can't start; see [Machines](#machines).

The mock checks only the fields it needs. It rejects a body that is not JSON. For apps, it rejects a missing or non-string `name` or `org_slug`. For volumes, it rejects a missing or wrongly typed `name`, `region` or `size_gb`. For Machines, it rejects a missing `config`, a `config` that is not an object, a non-string `name` or `region`, and a `mounts` that is not a list of objects each with a string `volume`. These get axum's default rejection, a `4xx` with a plain-text body, not Fly's error shape. Everything else is accepted: for example, a Machine `config` with no `image`, or unknown fields.

```sh
curl -s http://127.0.0.1:4280/healthz
# ok
curl -s -XPOST http://127.0.0.1:4280/v1/apps -H 'content-type: application/json' \
  -d '{"name": "fam-1", "org_slug": "personal"}'
# {"created_at":...,"id":"app_..."}
curl -s -XPOST http://127.0.0.1:4280/v1/apps/fam-1/volumes -H 'content-type: application/json' \
  -d '{"name": "data", "region": "jnb", "size_gb": 1}'
# {"id":"vol_...","name":"data","region":"jnb","size_gb":1,"state":"created"}
# Pass the volume's id in mounts[].volume:
curl -s -XPOST http://127.0.0.1:4280/v1/apps/fam-1/machines -H 'content-type: application/json' \
  -d '{"config": {"image": "family-app:dev", "mounts": [{"volume": "vol_...", "path": "/data"}]}}'
# The family app is now at http://<private_ip>:8080.
# {"id":"...","name":"machine-...","state":"started","region":"mock","private_ip":"127....","config":{...}}
```

## Contract

`tests/fixtures/openapi.json` is a snapshot of Fly's Machines OpenAPI spec (OpenAPI 3.1.0), fetched from <https://docs.fly.io/api/machines/openapi.json> on 2026-11-01. `tests/machines_api.rs` uses the `jsonschema` crate to check requests and responses against the spec's schema for each path, method and status. `$ref`s are resolved against the whole spec. Responses with a status the spec does not list are checked against `ErrorResponse`. To refresh the snapshot, download the file again, update the date here, and run the tests.

## Tests

```sh
cargo build -p family-app
cargo test --locked -p fly-mock
```

The tests boot the real family app, so build it first. They look for it in the same Cargo target directory as the test executable, or at `FLY_MOCK_FAMILY_APP_BIN` if that is set, and fail with a message naming the path if it is missing. `cargo test --locked` at the workspace root builds it.

`tests/health.rs` and `tests/machines_api.rs` send requests straight into the router. `tests/booting.rs` and `tests/start_failures.rs` do too, and check the family app processes that result. `tests/server.rs` runs the built binary, queries it over TCP and stops it with a signal. In CI, `cargo test --locked` at the workspace root runs them all. Run `cargo deny --locked check` from the repo root to check dependency licenses, advisories, bans and sources for every workspace crate.
