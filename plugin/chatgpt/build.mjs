/**
 * Packages the ChatGPT plugin as a ZIP for upload (or for a local marketplace).
 *
 *   bun plugin/chatgpt/build.mjs --origin https://crumb.example.com --developer "Name" --demo-url https://...
 *
 * The MCP origin can never change once a plugin is published, so it is always passed in, never
 * written in the templates. A development origin (a *.railway.app address or localhost) makes a
 * `-dev` ZIP that the script refuses to treat as submittable. For anything else, every
 * placeholder must be filled and the listing must meet OpenAI's limits.
 */
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { crc32, deflateRawSync } from "node:zlib"

const here = dirname(fileURLToPath(import.meta.url))
const repo = resolve(here, "../..")

/** Files copied into the package from elsewhere in the repo. */
const ASSETS = {
  "assets/logo.png": join(repo, "web/public/icon-512.png"),
  "assets/icon.svg": join(repo, "web/public/favicon.svg"),
}

export function isDevOrigin(origin) {
  const host = new URL(origin).hostname
  return host === "localhost" || host === "127.0.0.1" || host.endsWith(".railway.app")
}

/** Replaces `{{NAME}}` in every string of a parsed JSON value. */
export function fill(value, vars) {
  if (typeof value === "string") return value.replace(/\{\{([A-Z_]+)\}\}/g, (m, k) => vars[k] ?? m)
  if (Array.isArray(value)) return value.map((v) => fill(v, vars))
  if (value && typeof value === "object")
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, fill(v, vars)]))
  return value
}

const limit = (problems, label, text, max) => {
  if (typeof text !== "string" || !text) problems.push(`${label} is required`)
  else if ([...text].length > max) problems.push(`${label} is over ${max} characters`)
}

/** What OpenAI requires of a listing; returns the problems found. */
export function check(plugin, mcp, { submittable }) {
  const problems = []
  const ui = plugin.extensions?.["com.openai"]?.interface ?? {}
  limit(problems, "displayName", ui.displayName, 30)
  limit(problems, "shortDescription", ui.shortDescription, 30)
  limit(problems, "longDescription", ui.longDescription, 4000)
  limit(problems, "developerName", ui.developerName, 80)
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(plugin.name ?? "") || plugin.name.length > 64)
    problems.push("name must be kebab-case, at most 64 characters")
  if (!/^\d+\.\d+\.\d+$/.test(plugin.version ?? "")) problems.push("version must be semver")
  if ((ui.defaultPrompt ?? []).length > 3) problems.push("at most three default prompts")
  for (const p of ui.defaultPrompt ?? []) limit(problems, "a default prompt", p, 128)
  for (const k of ["brandColor", "brandColorDark"])
    if (ui[k] && !/^#[0-9A-Fa-f]{6}$/.test(ui[k])) problems.push(`${k} must be #RRGGBB`)
  const servers = Object.keys(mcp.mcpServers ?? {})
  if (servers.length !== 1) problems.push("exactly one MCP server is allowed")
  for (const s of Object.values(mcp.mcpServers ?? {}))
    if (!/\/mcp\/chatgpt$/.test(s.url ?? "")) problems.push("the MCP url must end in /mcp/chatgpt")

  if (submittable) {
    const review = plugin.extensions["com.openai"].review ?? {}
    const cases = review.test_cases ?? {}
    if (cases.positive?.length !== 5) problems.push("review needs exactly five positive cases")
    if (cases.negative?.length !== 3) problems.push("review needs exactly three negative cases")
    if (!review.demo_recording_url) problems.push("review needs a demo recording URL")
    for (const k of ["websiteURL", "supportURL", "privacyPolicyURL", "termsOfServiceURL"])
      if (!String(ui[k] ?? "").startsWith("https://")) problems.push(`${k} must be an https URL`)
    if (/example\.(com|invalid)/.test(JSON.stringify(plugin)))
      problems.push("placeholder URL left in")
  }
  const left = JSON.stringify([plugin, mcp]).match(/\{\{[A-Z_]+\}\}/g)
  if (left) problems.push(`unfilled placeholders: ${[...new Set(left)].join(", ")}`)
  return problems
}

/** A ZIP with fixed timestamps, so the same inputs make the same bytes. */
export function zip(files) {
  const parts = []
  const central = []
  let offset = 0
  const u16 = (n) => Buffer.from([n & 255, (n >> 8) & 255])
  const u32 = (n) => Buffer.from([n & 255, (n >> 8) & 255, (n >> 16) & 255, (n >>> 24) & 255])
  const DATE = (1 << 5) | 1 // 1980-01-01
  for (const [name, data] of Object.entries(files)) {
    const nameBytes = Buffer.from(name)
    const body = deflateRawSync(data, { level: 9 })
    const crc = crc32(data)
    const head = Buffer.concat([
      u32(0x04034b50),
      u16(20),
      u16(0x0800),
      u16(8),
      u16(0),
      u16(DATE),
      u32(crc),
      u32(body.length),
      u32(data.length),
      u16(nameBytes.length),
      u16(0),
      nameBytes,
    ])
    central.push(
      Buffer.concat([
        u32(0x02014b50),
        u16(20),
        u16(20),
        u16(0x0800),
        u16(8),
        u16(0),
        u16(DATE),
        u32(crc),
        u32(body.length),
        u32(data.length),
        u16(nameBytes.length),
        u16(0),
        u16(0),
        u16(0),
        u16(0),
        u32(0),
        u32(offset),
        nameBytes,
      ]),
    )
    parts.push(head, body)
    offset += head.length + body.length
  }
  const dir = Buffer.concat(central)
  const end = Buffer.concat([
    u32(0x06054b50),
    u16(0),
    u16(0),
    u16(central.length),
    u16(central.length),
    u32(dir.length),
    u32(offset),
    u16(0),
  ])
  return Buffer.concat([...parts, dir, end])
}

function args(argv) {
  const out = {}
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1]
  return out
}

export async function build({ origin, developer, demoUrl, outDir }) {
  const url = new URL(origin)
  if (url.protocol !== "https:" && !isDevOrigin(origin)) throw new Error("--origin must be https")
  const base = url.origin
  const dev = isDevOrigin(base)
  const vars = { ORIGIN: base, DEVELOPER: developer ?? (dev ? "Crumb (dev)" : undefined) }
  if (demoUrl) vars.DEMO_URL = demoUrl

  let plugin = fill(JSON.parse(await readFile(join(here, "plugin.json"), "utf8")), vars)
  const mcp = fill(JSON.parse(await readFile(join(here, "mcp.json"), "utf8")), vars)
  if (!demoUrl) delete plugin.extensions["com.openai"].review.demo_recording_url
  const problems = check(plugin, mcp, { submittable: !dev })
  if (problems.length) throw new Error(`Not ready:\n- ${problems.join("\n- ")}`)

  const files = {
    "plugin.json": Buffer.from(`${JSON.stringify(plugin, null, 2)}\n`),
    "mcp.json": Buffer.from(`${JSON.stringify(mcp, null, 2)}\n`),
  }
  for (const [name, from] of Object.entries(ASSETS)) files[name] = await readFile(from)
  await mkdir(outDir, { recursive: true })
  const file = join(outDir, `crumb-chatgpt-${plugin.version}${dev ? "-dev" : ""}.zip`)
  await writeFile(file, zip(files))
  return { file, dev }
}

if (import.meta.main) {
  const a = args(process.argv.slice(2))
  if (!a.origin) {
    console.error(
      "usage: bun plugin/chatgpt/build.mjs --origin <https://host> [--developer <name>] [--demo-url <url>]",
    )
    process.exit(2)
  }
  try {
    const { file, dev } = await build({
      origin: a.origin,
      developer: a.developer,
      demoUrl: a["demo-url"],
      outDir: join(here, "dist"),
    })
    console.log(`wrote ${file}`)
    if (dev)
      console.log(
        "This is a development package: the MCP origin of a published plugin can never change, so don't submit it.",
      )
  } catch (err) {
    console.error(err.message)
    process.exit(1)
  }
}
