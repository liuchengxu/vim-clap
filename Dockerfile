# Dockerfile for building maple static binary

# Our first FROM statement declares the build environment.
FROM ekidd/rust-musl-builder AS builder

# Add our source code.
ADD --chown=rust:rust . ./

# Build our application.
RUN cargo build --release --locked
