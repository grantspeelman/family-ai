# registry

The registry app: a small GraphQL API over HTTP, written in Rust (axum and async-graphql). The schema has one query, `apiVersion`, and one mutation, `createAccount`. Accounts live in a SQLite database.

## Run it

From the repo root:

```sh
REGISTRY_DB_PATH=target/registry.db cargo run -p registry
```

`REGISTRY_DB_PATH` is required, including for `cargo run`: it is the SQLite database file, created if it does not exist. If it is unset or empty, or the database cannot be opened, the registry prints the cause to stderr and exits non-zero before it starts listening.

It listens on `127.0.0.1:8080`. Set `REGISTRY_ADDR` to change that, for example `REGISTRY_ADDR=0.0.0.0:9000 cargo run -p registry`. Ctrl-C or SIGTERM stops it.

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
  -d '{"query":"mutation { createAccount(email: \"ana@example.com\", familyName: \"Silva\") { id email familyName } }"}' \
  http://127.0.0.1:8080/graphql
# {"data":{"createAccount":{"id":"1","email":"ana@example.com","familyName":"Silva"}}}
```

Errors come back as GraphQL errors with `data` null and a stable `extensions.code`. Messages never contain the submitted values.

| Code | When |
| --- | --- |
| `INVALID_EMAIL` | Not exactly one `@`, nothing before it, no `.` inside the domain (not first or last), any whitespace, or over 254 characters. |
| `INVALID_FAMILY_NAME` | Empty after trimming, or over 200 characters. |
| `EMAIL_TAKEN` | Another account has this email, ignoring letter case. |
| `INTERNAL` | Any other failure. The message is generic. |

## Tests

```sh
cargo test --locked
```

The API tests send requests straight into the router, with no network, each on its own database file under Cargo's target temp directory. Run `cargo deny --locked check` from the repo root to check dependency licenses, advisories, bans and sources for every workspace crate.

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
