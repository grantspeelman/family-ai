# registry

The registry app: a small GraphQL API over HTTP, written in Rust (axum and async-graphql). The schema has two queries, `apiVersion` and `accountStatus`, and one mutation, `createAccount`. Accounts live in a SQLite database. Creating an account provisions the family's app, volume and Machine through the Fly API.

## Run it

From the repo root:

```sh
REGISTRY_DB_PATH=target/registry.db cargo run -p registry
```

`REGISTRY_DB_PATH` is required, including for `cargo run`: it is the SQLite database file, created if it does not exist. If it is unset or empty, or the database cannot be opened, the registry prints the cause to stderr and exits non-zero before it starts listening.

Without `FLY_API_HOSTNAME`, the registry starts but logs that provisioning is disabled, and `createAccount` returns `PROVISIONING_UNAVAILABLE`. See [Provisioning](#provisioning) for running it against the Fly mock.

It listens on `127.0.0.1:8080`. Set `REGISTRY_ADDR` to change that, for example `REGISTRY_ADDR=0.0.0.0:9000 cargo run -p registry`. It logs the address it bound, so a port of `0` shows the port the OS picked. Ctrl-C or SIGTERM stops it with exit code 0. Any startup or server failure logs one line and exits with code 1.

## Endpoints

| Endpoint | What it does |
| --- | --- |
| `POST /graphql` | The GraphQL API. There is no GraphiQL or playground. |
| `GET /healthz` | Returns `200` with the body `ok`. |

```sh
curl -s -H 'Content-Type: application/json' \
  -d '{"query":"{ apiVersion }"}' http://127.0.0.1:8080/graphql
# {"data":{"apiVersion":"0.1.0"}}
```

### createAccount

Creates an account from an email and a family name. Surrounding whitespace is trimmed from both, and the trimmed values are what is checked and stored. The email check is a sanity check only, with no confirmation mail.

```sh
curl -s -H 'Content-Type: application/json' \
  -d '{"query":"mutation { createAccount(email: \"ana@example.com\", familyName: \"Silva\") { id email familyName status address } }"}' \
  http://127.0.0.1:8080/graphql
# {"data":{"createAccount":{"id":"1","email":"ana@example.com","familyName":"Silva","status":"PROVISIONING","address":null}}}
```

The account is always returned as `PROVISIONING`. Poll `accountStatus` to learn when its family app is ready.

Errors come back as GraphQL errors with `data` null and a stable `extensions.code`. Messages never contain the submitted values.

| Code | When |
| --- | --- |
| `INVALID_EMAIL` | Not exactly one `@`, nothing before it, no `.` inside the domain (not first or last), any whitespace, or over 254 characters. |
| `INVALID_FAMILY_NAME` | Empty after trimming, or over 200 characters. |
| `PROVISIONING_UNAVAILABLE` | `FLY_API_HOSTNAME` is not set, so the registry cannot provision. Nothing is stored. |
| `EMAIL_TAKEN` | Another account has this email, ignoring letter case. |
| `INTERNAL` | Any other failure. The message is generic. |

### accountStatus

The polling query for an account's provisioning. It returns only `id`, `status` and `address`, never the email or family name: there is no authentication yet and ids are guessable, so walking ids must reveal nothing personal. It returns `null` for an unknown or malformed id.

```sh
curl -s -H 'Content-Type: application/json' \
  -d '{"query":"{ accountStatus(id: \"1\") { id status address } }"}' \
  http://127.0.0.1:8080/graphql
# {"data":{"accountStatus":{"id":"1","status":"READY","address":"http://127.0.0.2:18080"}}}
```

## Provisioning

`createAccount` stores the account as `PROVISIONING` and returns. A background task then calls the Fly API: create the app, create the volume, then create the Machine with that volume mounted at `/data`.

| Status | Meaning |
| --- | --- |
| `PROVISIONING` | The task is still running. `address` is null. |
| `READY` | The family app runs at `address`. |
| `FAILED` | Any Fly error, a Machine that is not `started`, an unparsable `private_ip`, or the 60-second time limit. Also accounts left `PROVISIONING` by a restart, and accounts created before provisioning existed. `address` is null. |

**The address rule.** An account is `READY` only when create-Machine returns `state` `"started"` and a `private_ip` that parses as an IP address. The address is `http://<private_ip>:<REGISTRY_FAMILY_APP_PORT>`, with IPv6 in brackets, for example `http://[fdaa::3]:8080`.

On failure, the registry logs one stderr line with the account id and the error, never the email, family name or token. The Fly ids from the steps that succeeded stay stored, and created resources are not cleaned up. At startup, before listening, the registry marks every account still `PROVISIONING` as `FAILED`, because its task died with the previous process.

These are code constants in `src/provisioning.rs`:

| Constant | Value |
| --- | --- |
| App name | `family-<account id>` |
| Org slug | `personal` |
| Region (volume and Machine) | `jnb` |
| Volume | `family_data`, 1 GB, mounted at `/data` |
| Image | `family-app:latest` (the Fly mock ignores it) |
| Machine `env` | `FAMILY_APP_DATA_DIR=/data`, `FAMILY_APP_ADDR=[::]:<machine port>` (the Fly mock ignores it) |

### Configuration

| Variable | What |
| --- | --- |
| `FLY_API_HOSTNAME` | The Fly API base URL, such as `http://127.0.0.1:4280` for the Fly mock. Unset or empty disables provisioning. Any other value that is not an `http` or `https` URL makes the registry exit non-zero. |
| `FLY_API_TOKEN` | Optional. Sent as a bearer token. Never logged. |
| `REGISTRY_FAMILY_APP_PORT` | The family app's port on its Machine, default `8080`. It must match the Fly mock's `FLY_MOCK_MACHINE_PORT`. A value that is not a port from 1 to 65535 makes the registry exit non-zero. |

### Against a local Fly mock

Build the family app first, because the mock starts it for each Machine. Then run the mock and the registry in two shells, from the repo root:

```sh
cargo build -p family-app
FLY_MOCK_ADDR=127.0.0.1:4280 FLY_MOCK_MACHINE_PORT=18080 cargo run -p fly-mock
```

```sh
REGISTRY_DB_PATH=target/registry.db FLY_API_HOSTNAME=http://127.0.0.1:4280 \
  REGISTRY_FAMILY_APP_PORT=18080 cargo run -p registry
```

`fly-mock/README.md` covers the mock's own settings.

## Tests

```sh
cargo test --locked
```

The API tests send requests straight into the router, each on its own database file under Cargo's target temp directory. The provisioning tests run the Fly mock as a process, with `cargo run`, and small stub servers on `127.0.0.1`. Run `cargo deny --locked check` from the repo root to check dependency licenses, advisories, bans and sources for every workspace crate.

## The schema file

`schema.graphql` is the committed copy of the schema. A test fails if it differs from what the code generates. After changing the schema, regenerate it and commit the result:

```sh
UPDATE_SCHEMA=1 cargo test -p registry --test api schema_file_matches_the_code
```

## The image

Build from the repo root, because the Cargo workspace files live there:

```sh
docker build -f registry/Dockerfile -t family-ai-registry .
docker run --rm -p 127.0.0.1:8080:8080 family-ai-registry
```

The image listens on `0.0.0.0:8080` inside the container and runs as UID 10001. It sets `REGISTRY_DB_PATH=/data/registry.db`, and `/data` is owned by UID 10001. There is no volume, so the database is lost with the container unless you mount one at `/data`.
