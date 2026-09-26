# syntax=docker/dockerfile:1

# ---- Frontend: Astro static build (HTML, CSS, JS, fonts, precompressed) ----
FROM node:22-bookworm-slim AS web
WORKDIR /web
RUN npm install -g bun@1.3
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
# With a Sentry token (CI passes it as a BuildKit secret, never a layer), hidden source maps
# are uploaded to Sentry for this release. Maps are never shipped: any the upload left behind
# are deleted here. Without a token the build makes none.
ARG SENTRY_ORG SENTRY_PROJECT SENTRY_RELEASE
RUN --mount=type=secret,id=sentry_auth_token,env=SENTRY_AUTH_TOKEN \
    bun run build \
    && find dist -name '*.map' -delete

# ---- Server: Rust ----
FROM rust:1-slim-bookworm AS server
WORKDIR /app
# wreq builds BoringSSL from source: CMake, make and a C++ compiler for the library, libclang for
# its bindings, git to apply its patches. Build stage only; the binary links it statically.
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ git libclang-dev make \
    && rm -rf /var/lib/apt/lists/*
# Build dependencies against stub sources first, so code-only changes reuse this layer
COPY Cargo.toml Cargo.lock ./
COPY crates/crumb-core/Cargo.toml crates/crumb-core/Cargo.toml
COPY crates/crumb-client/Cargo.toml crates/crumb-client/Cargo.toml
# The desktop app is a workspace member but isn't built here; stubs let Cargo load the workspace
COPY desktop/linux/Cargo.toml desktop/linux/Cargo.toml
RUN mkdir -p src crates/crumb-core/src crates/crumb-client/src desktop/linux/src \
    && echo 'fn main() {}' | tee src/main.rs desktop/linux/src/main.rs >/dev/null \
    && touch src/lib.rs crates/crumb-core/src/lib.rs crates/crumb-client/src/lib.rs desktop/linux/src/lib.rs \
    && cargo build --release --locked && rm -rf src crates
COPY src ./src
COPY crates ./crates
RUN find src crates -name '*.rs' -exec touch {} + && cargo build --release --locked

# ---- Run ----
FROM debian:bookworm-slim
WORKDIR /app

# Headless Chromium for sites that block plain HTTP scraping (adds ~250 MB).
# Build with --build-arg WITH_CHROMIUM=false for a slimmer image without the fallback.
ARG WITH_CHROMIUM=true
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
       $(if [ "$WITH_CHROMIUM" = "true" ]; then echo chromium fonts-liberation; fi) \
    && rm -rf /var/lib/apt/lists/*

# Railway injects PORT at runtime; 3000 is the local default
ENV HOST=0.0.0.0 \
    PORT=3000 \
    WEB_DIST=/app/web \
    CHROMIUM_PATH=/usr/bin/chromium \
    RUST_LOG=info
# glibc otherwise keeps up to 8 arenas per core of freed resize buffers (~220 MB that never
# shrinks); these bring steady-state RSS to ~20 MB with no measurable speed cost
ENV MALLOC_ARENA_MAX=2 \
    MALLOC_MMAP_THRESHOLD_=131072 \
    MALLOC_TRIM_THRESHOLD_=131072
# The version CI built (Sentry's release name); error reporting stays off unless SENTRY_DSN is set
ARG CRUMB_VERSION=""
ENV SENTRY_RELEASE=$CRUMB_VERSION
COPY --from=server /app/target/release/crumb /usr/local/bin/crumb
COPY --from=web /web/dist /app/web
EXPOSE 3000
# tini as PID 1 reaps Chromium's orphaned helper processes
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["crumb"]
