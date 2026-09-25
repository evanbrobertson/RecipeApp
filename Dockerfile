# syntax=docker/dockerfile:1

# ---- Frontend: Astro static build (HTML, CSS, JS, fonts, precompressed) ----
FROM node:22-bookworm-slim AS web
WORKDIR /web
RUN npm install -g bun@1.3
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
RUN bun run build

# ---- Server: Rust ----
FROM rust:1-slim-bookworm AS server
WORKDIR /app
# Build dependencies against stub sources first, so code-only changes reuse this layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs \
    && cargo build --release --locked && rm -rf src
COPY src ./src
RUN find src -name '*.rs' -exec touch {} + && cargo build --release --locked

# ---- Run ----
FROM debian:bookworm-slim
WORKDIR /app

# Headless Chromium for sites that block plain HTTP scraping (adds ~250 MB).
# Build with --build-arg WITH_CHROMIUM=false for a slimmer image without the fallback.
ARG WITH_CHROMIUM=true
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
       $(if [ "$WITH_CHROMIUM" = "true" ]; then echo chromium fonts-liberation; fi) \
    && rm -rf /var/lib/apt/lists/*

# Railway injects PORT at runtime; 3000 is the local default
ENV HOST=0.0.0.0 \
    PORT=3000 \
    WEB_DIST=/app/web \
    CHROMIUM_PATH=/usr/bin/chromium \
    RUST_LOG=info
COPY --from=server /app/target/release/crumb /usr/local/bin/crumb
COPY --from=web /web/dist /app/web
EXPOSE 3000
CMD ["crumb"]
