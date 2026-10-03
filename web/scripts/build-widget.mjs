/**
 * Builds the ChatGPT plugin's widget (web/widget) into one self-contained HTML file, written
 * into the Astro build as shell/chatgpt/shelf.html. The Rust server serves it as an MCP
 * resource after swapping __CRUMB_ORIGIN__ for its own address, which is where the widget's
 * fonts load from (it runs in a frame on another origin, so it can only name the server by
 * its full address). Runs after `astro build` (which empties the output folder).
 */
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { svelte } from "@sveltejs/vite-plugin-svelte"
import tailwindcss from "@tailwindcss/vite"
import { build } from "vite"

const web = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const out = resolve(web, process.env.CRUMB_OUT_DIR ?? "./dist")
const scratch = resolve(web, "node_modules/.widget-build")

await build({
  root: web,
  configFile: false,
  logLevel: "warn",
  plugins: [tailwindcss(), svelte()],
  build: {
    outDir: scratch,
    emptyOutDir: true,
    assetsInlineLimit: 0,
    cssCodeSplit: false,
    rollupOptions: { input: join(web, "widget/index.html") },
  },
})

const html = await readFile(join(scratch, "widget/index.html"), "utf8")
const files = async (path) => readFile(join(scratch, path.replace(/^\//, "")), "utf8")

let page = html
for (const [, href] of html.matchAll(/<link rel="stylesheet"[^>]*href="([^"]+)"[^>]*>/g)) {
  const css = await files(href)
  page = page.replace(new RegExp(`<link rel="stylesheet"[^>]*href="${href}"[^>]*>`), () => "")
  page = page.replace("</head>", () => `<style>${css}</style></head>`)
}
for (const [tag, src] of html.matchAll(
  /<script type="module"[^>]*src="([^"]+)"[^>]*><\/script>/g,
)) {
  const js = (await files(src)).replaceAll("</script", "<\\/script")
  page = page.replace(tag, () => "")
  page = page.replace("</body>", () => `<script type="module">${js}</script></body>`)
}
// Fonts come from the server's public /fonts/ (no sign-in), copied there below
const fonts = [
  ["Nunito Sans", "nunito-sans", "200 1000"],
  ["DM Serif Display", "dm-serif-display", "400"],
]
const faces = fonts
  .map(
    ([family, file, weight]) =>
      `@font-face{font-family:"${family}";src:url(__CRUMB_ORIGIN__/fonts/${file}.woff2) format("woff2");font-weight:${weight};font-display:swap}`,
  )
  .join("")
page = page.replace("</head>", () => `<style>${faces}</style></head>`)

await mkdir(join(out, "shell/chatgpt"), { recursive: true })
await writeFile(join(out, "shell/chatgpt/shelf.html"), page)
await mkdir(join(out, "fonts"), { recursive: true })
for (const [, file] of fonts) {
  await copyFile(
    join(web, "src/assets/fonts", `${file}.woff2`),
    join(out, "fonts", `${file}.woff2`),
  )
}
console.log(`widget: shell/chatgpt/shelf.html (${(page.length / 1024).toFixed(0)} KB)`)
