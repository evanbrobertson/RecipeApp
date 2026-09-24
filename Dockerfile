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
ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000
COPY --from=build /app/.output ./.output
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
