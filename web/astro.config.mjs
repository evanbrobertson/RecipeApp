import { readdir, readFile, writeFile } from "node:fs/promises"
import { brotliCompressSync, constants, gzipSync } from "node:zlib"
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

export default defineConfig({
  output: "static",
  // Parallel local builds (one per worktree or agent) can write to their own folder
  outDir: process.env.CRUMB_OUT_DIR ?? "./dist",
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
  integrations: [svelte(), precompress()],
})
