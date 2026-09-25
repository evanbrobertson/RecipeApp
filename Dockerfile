# syntax=docker/dockerfile:1

# ---- Build ----
FROM node:22-bookworm-slim AS build
WORKDIR /app
# Bun for fast installs. Node is on PATH, so better-sqlite3's install script fetches the
# prebuilt binary for the Node 22 ABI that the runtime image uses.
RUN npm install -g bun@1.3
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

# ---- Run ----
FROM node:22-bookworm-slim
WORKDIR /app

# Headless Chromium for sites that block plain HTTP scraping (adds ~250 MB).
# Build with --build-arg WITH_CHROMIUM=false for a slimmer image without the fallback.
ARG WITH_CHROMIUM=true
RUN if [ "$WITH_CHROMIUM" = "true" ]; then \
      apt-get update \
      && apt-get install -y --no-install-recommends chromium fonts-liberation ca-certificates \
      && rm -rf /var/lib/apt/lists/*; \
    fi

ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000 \
    CHROMIUM_PATH=/usr/bin/chromium
COPY --from=build /app/.output ./.output
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
