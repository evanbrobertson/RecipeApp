# syntax=docker/dockerfile:1
#
# crumb-relay for linux/amd64, linux/arm64 and linux/arm/v7 (see docs/RELAY.md).
# Build context: the repository root.
#
#   Build it here (on the machine that will run it; slow the first time, BoringSSL is compiled):
#     docker build -f crates/crumb-relay/Dockerfile -t crumb-relay .
#   Or, as CI does, around binaries built beforehand (nothing is compiled or emulated):
#     docker buildx build -f crates/crumb-relay/Dockerfile --build-arg BINARY_SOURCE=prebuilt \
#       --platform linux/amd64,linux/arm64,linux/arm/v7 .   # with dist/crumb-relay-linux-{amd64,arm64,arm}
ARG BINARY_SOURCE=build

# ---- Compile ----
FROM rust:1-slim-bookworm AS build
WORKDIR /app
# wreq builds BoringSSL from source: CMake, make and a C++ compiler for the library, libclang for
# its bindings, git to apply its patches. Build stage only.
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ git libclang-dev make \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
RUN cargo build --release --locked -p crumb-relay \
    && install -D target/release/crumb-relay /out/crumb-relay

# ---- Binaries built beforehand ----
FROM scratch AS prebuilt
ARG TARGETARCH
COPY dist/crumb-relay-linux-${TARGETARCH} /out/crumb-relay

FROM ${BINARY_SOURCE} AS binary

# ---- Run ----
# glibc, libstdc++ and the CA certificates, no shell; runs as a non-root user
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=binary /out/crumb-relay /usr/local/bin/crumb-relay
# The port is reached through Docker's port mapping: publish it on the Tailscale address only,
# e.g. -p 100.101.102.103:8787:8787
ENV RELAY_LISTEN=0.0.0.0:8787 \
    RUST_LOG=info
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/crumb-relay"]
