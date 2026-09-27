# syntax=docker/dockerfile:1
# crumb-auth: Better Auth for the hosted edition (AUTH_MODE=hosted). A separate Railway
# service with its own volume; only Crumb's server talks to it, over the private network.
FROM oven/bun:1.3-slim
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile --production
COPY src ./src
ENV HOST=:: \
    PORT=3100 \
    NODE_ENV=production
EXPOSE 3100
CMD ["bun", "src/server.ts"]
