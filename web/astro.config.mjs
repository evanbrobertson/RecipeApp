import { readdir, readFile, writeFile } from "node:fs/promises"
import { brotliCompressSync, constants, gzipSync } from "node:zlib"
import sentry from "@sentry/astro"
import svelte from "@astrojs/svelte"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "astro/config"

/**
 * Writes `.br` and `.gz` siblings for every compressible file in the build, so the
 * Rust server can hand them out as-is (ServeDir's precompressed mode) instead of
 * compressing the same bytes on every request.
 */
const precompress = () => ({
  name: "crumb:precompress",
  hooks: {
    "astro:build:done": async ({ dir }) => {
      const root = dir.pathname
      const files = await readdir(root, { recursive: true, withFileTypes: true })
      await Promise.all(
        files
          .filter((f) => f.isFile() && /\.(html|js|css|svg|json|webmanifest|txt|xml)$/.test(f.name))
          .map(async (f) => {
            const path = `${f.parentPath}/${f.name}`
            const body = await readFile(path)
            if (body.length < 512) return
            await writeFile(
              `${path}.br`,
              brotliCompressSync(body, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }),
            )
            await writeFile(`${path}.gz`, gzipSync(body, { level: 9 }))
          }),
      )
    },
  },
})

const outDir = process.env.CRUMB_OUT_DIR ?? "./dist"

/**
 * Browser error reporting. The SDK only starts when the Rust server names a DSN on the page
 * load (src/lib/sentry-boot.ts), so nothing here is environment-specific. With
 * SENTRY_AUTH_TOKEN set (CI), the build emits hidden source maps, uploads them to Sentry and
 * deletes them, so they are never served; without it, no maps are made.
 */
const sentryIntegration = () =>
  sentry({
    enabled: { client: true, server: false },
    clientInitPath: "src/lib/sentry-boot.ts",
    org: process.env.SENTRY_ORG,
    project: process.env.SENTRY_PROJECT,
    authToken: process.env.SENTRY_AUTH_TOKEN,
    telemetry: false,
    // The release comes from the server at runtime; CI creates and finalizes it
    release: { name: process.env.SENTRY_RELEASE, inject: false, finalize: false },
    sourcemaps: {
      disable: !process.env.SENTRY_AUTH_TOKEN,
      filesToDeleteAfterUpload: [`${outDir}/**/*.map`],
    },
    bundleSizeOptimizations: {
      excludeDebugStatements: true,
      excludeReplayIframe: true,
      excludeReplayShadowDom: true,
    },
  })

export default defineConfig({
  output: "static",
  // Parallel local builds (one per worktree or agent) can write to their own folder
  outDir,
  trailingSlash: "ignore",
  build: { format: "directory", inlineStylesheets: "auto" },
  // The Rust server owns routing; in dev, proxy the API and dynamic pages to it
  server: { port: 4321 },
  vite: {
    plugins: [tailwindcss()],
    build: {
      rollupOptions: {
        output: {
          // One small icons chunk instead of a request per icon
          manualChunks: (id) => (id.includes("@lucide/svelte") ? "icons" : undefined),
        },
      },
    },
    server: {
      proxy: {
        "/api": "http://127.0.0.1:3000",
        "/mcp": "http://127.0.0.1:3000",
        "/oauth": "http://127.0.0.1:3000",
      },
    },
  },
  integrations: [svelte(), sentryIntegration(), precompress()],
})
