# registry

The registry app: a small GraphQL API over HTTP, written in Rust (axum and async-graphql). It is a scaffold for now. The schema has one query, `apiVersion`, and there is no account logic yet.

## Run it

From the repo root:

```sh
cargo run -p registry
```

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

## Tests

```sh
cargo test --locked
```

The tests send requests straight into the router, with no network.

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

The image listens on `0.0.0.0:8080` inside the container and runs as UID 10001.
