# fly-mock

The Fly mock: a local stand-in for Fly's Machines API, written in Rust (axum). Local dev and CI talk to it instead of real Fly, so they never need a Fly token. It is a skeleton for now and only answers a health check. The create-app, create-volume and create-Machine endpoints come in later PRs (see `first-build-pr-sequence.md`, PR 4).

## Run it

From the repo root:

```sh
cargo run -p fly-mock
```

It listens on `127.0.0.1:4280`, the port Fly's own Machines API uses on its private network, so it does not clash with the registry on `8080`. Set `FLY_MOCK_ADDR` to change that, for example `FLY_MOCK_ADDR=0.0.0.0:9001 cargo run -p fly-mock`. It prints the address it is listening on. A port of `0` lets the OS pick a free port. Ctrl-C or SIGTERM stops it with exit code 0.

## Endpoints

| Endpoint | What it does |
| --- | --- |
| `GET /healthz` | Returns `200` with the body `ok`. |

Any other path returns `404`.

```sh
curl -s http://127.0.0.1:4280/healthz
# ok
```

## Tests

```sh
cargo test --locked -p fly-mock
```

`tests/health.rs` sends requests straight into the router, with no network. `tests/server.rs` runs the built binary, queries it over TCP and stops it with a signal. In CI, `cargo test --locked` at the workspace root runs both. Run `cargo deny --locked check` from the repo root to check dependency licenses, advisories, bans and sources for every workspace crate.
