/**
 * Builds the extension into dist/chrome and dist/firefox (unpacked, loadable as they are).
 *
 *   bun run build                  version from package.json
 *   EXTENSION_VERSION=1.2.0 bun run build -- --zip    (CI) also dist/crumb-{target}-{version}.zip
 */
import { cp, mkdir, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { $ } from "bun"
import pkg from "./package.json"
import { manifest, TARGETS } from "./src/manifest"

const version = process.env.EXTENSION_VERSION || pkg.version
const zip = process.argv.includes("--zip")
const root = import.meta.dir
const dist = join(root, "dist")

await rm(dist, { recursive: true, force: true })

for (const target of TARGETS) {
  const out = join(dist, target)
  await mkdir(out, { recursive: true })
  const result = await Bun.build({
    entrypoints: ["src/content.ts", "src/background.ts", "src/options.ts"].map((p) =>
      join(root, p),
    ),
    outdir: out,
    target: "browser",
    // Classic scripts: content scripts and Firefox's background page can't be modules.
    // Unminified, so store reviewers read what runs.
    format: "iife",
    minify: false,
  })
  if (!result.success) {
    for (const log of result.logs) console.error(log)
    process.exit(1)
  }
  await cp(join(root, "public"), out, { recursive: true })
  await cp(join(root, "src/options.html"), join(out, "options.html"))
  await cp(join(root, "src/options.css"), join(out, "options.css"))
  await writeFile(
    join(out, "manifest.json"),
    `${JSON.stringify(manifest(target, version), null, 2)}\n`,
  )
  if (zip) {
    const file = join(dist, `crumb-${target}-${version}.zip`)
    await $`zip -qrX ${file} .`.cwd(out)
    console.log(`${file}`)
  }
}
console.log(`Crumb extension ${version}: ${TARGETS.map((t) => `dist/${t}`).join(", ")}`)
