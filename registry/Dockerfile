# The registry app image. Build from the repo root, which holds the workspace
# Cargo.toml and Cargo.lock: docker build -f registry/Dockerfile .
# CI builds the `toolchain` stage and runs the tests in it, so the tests and the
# release build use the same Rust. CI's rust-lint job builds the `lint` stage.
FROM rust:1.93-slim-trixie AS toolchain

# The same Rust plus clippy and rustfmt, for CI's workspace-wide cargo fmt and
# clippy. The image's minimal rustup profile leaves both out. The release build
# below does not use this stage.
FROM toolchain AS lint
RUN rustup component add clippy rustfmt

FROM toolchain AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
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
