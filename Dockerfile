# This image uses cargo-chef to build the application in order to compile
# the dependencies apart from the main application. This allows the compiled
# dependencies to be cached in the Docker layer and greatly reduce the
# build time when there isn't any dependency changes.
#
# https://github.com/LukeMathWalker/cargo-chef

ARG SRC_DIR=/usr/local/src/rgb
ARG BUILDER_DIR=/srv/rgb

# Base image
FROM rust:1.80.0-slim-bookworm as chef

ARG SRC_DIR
ARG BUILDER_DIR

RUN rustup default stable
RUN rustup update
RUN cargo install cargo-chef --locked

WORKDIR $SRC_DIR

# Cargo chef step that analyzes the project to determine the minimum subset of
# files (Cargo.lock and Cargo.toml manifests) required to build it and cache
# dependencies
FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

ARG SRC_DIR
ARG BUILDER_DIR

COPY --from=planner "${SRC_DIR}/recipe.json" recipe.json

# Build dependencies - this is the caching Docker layer
RUN cargo chef cook --release --recipe-path recipe.json --target-dir "${BUILDER_DIR}"

# Copy all files and build application
COPY . .
RUN cargo build --release --target-dir "${BUILDER_DIR}" --bins --all-features

# Final image with binaries
FROM debian:bullseye-slim as final

ARG BUILDER_DIR
ARG DATA_DIR=/var/lib/rgb
ARG BIN_DIR=/usr/local/bin
ARG USER=rgb

RUN adduser --home "${DATA_DIR}" --shell /bin/bash --disabled-login \
        --gecos "${USER} user" ${USER}

COPY --from=builder --chown=${USER}:${USER} \
     "${BUILDER_DIR}/release" "${BIN_DIR}"

WORKDIR "${BIN_DIR}"

# Remove build artifacts in order to keep only the binaries
RUN rm -rf */ *.d .[^.] .??*

USER ${USER}

ENTRYPOINT ["rgb"]
