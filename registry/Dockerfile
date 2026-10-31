# The registry app image. Build from the repo root, which holds the workspace
# Cargo.toml and Cargo.lock: docker build -f registry/Dockerfile .
# CI builds the `toolchain` stage and runs the tests in it, so the tests and the
# release build use the same Rust.
FROM rust:1.93-slim-trixie AS toolchain

FROM toolchain AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
# cargo loads every workspace member, even for -p registry, so each member's
# directory must be here. Add new members to this list.
COPY family-app family-app
COPY fly-mock fly-mock
COPY registry registry
RUN cargo build --release --locked -p registry

FROM debian:trixie-slim
# A fixed UID, so volume ownership does not depend on the host.
RUN useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin registry
COPY --from=build /src/target/release/registry /usr/local/bin/registry
ENV REGISTRY_ADDR=0.0.0.0:8080
EXPOSE 8080
USER 10001
ENTRYPOINT ["/usr/local/bin/registry"]
