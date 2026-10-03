import { expect, test } from "bun:test"
import { mkdtemp, readFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { build, check, fill, isDevOrigin, zip } from "./build.mjs"

const out = () => mkdtemp(join(tmpdir(), "crumb-plugin-"))

test("a development origin makes a -dev package that unzips to the filled manifests", async () => {
  const { file, dev } = await build({
    origin: "https://crumb-dev.up.railway.app/",
    outDir: await out(),
  })
  expect(dev).toBe(true)
  expect(file.endsWith("-dev.zip")).toBe(true)
  const listing = Bun.spawnSync(["unzip", "-l", file]).stdout.toString()
  for (const name of ["plugin.json", "mcp.json", "assets/logo.png", "assets/icon.svg"])
    expect(listing).toContain(name)
  const mcp = JSON.parse(Bun.spawnSync(["unzip", "-p", file, "mcp.json"]).stdout.toString())
  expect(mcp.mcpServers.crumb.url).toBe("https://crumb-dev.up.railway.app/mcp/chatgpt")
  const test = Bun.spawnSync(["unzip", "-t", file])
  expect(test.exitCode).toBe(0)
})

test("the same inputs make the same bytes", async () => {
  const a = await build({ origin: "http://localhost:3000", outDir: await out() })
  const b = await build({ origin: "http://localhost:3000", outDir: await out() })
  expect(await readFile(a.file)).toEqual(await readFile(b.file))
})

test("a real origin must be https and complete before it is packaged", async () => {
  await expect(build({ origin: "http://crumb.example.org", outDir: await out() })).rejects.toThrow(
    "https",
  )
  // No developer, no demo recording: not ready to submit
  await expect(build({ origin: "https://crumb.example.org", outDir: await out() })).rejects.toThrow(
    /unfilled placeholders: .*DEVELOPER/,
  )
  const ok = await build({
    origin: "https://crumb.example.org",
    developer: "A. Person",
    demoUrl: "https://videos.example.org/crumb",
    outDir: await out(),
  })
  expect(ok.dev).toBe(false)
  expect(ok.file.endsWith("0.1.0.zip")).toBe(true)
})

test("the listing limits are checked", () => {
  const plugin = {
    name: "Bad Name",
    version: "1",
    extensions: {
      "com.openai": {
        interface: {
          displayName: "x".repeat(31),
          shortDescription: "ok",
          longDescription: "ok",
          developerName: "me",
          defaultPrompt: ["a", "b", "c", "d"],
          brandColor: "green",
        },
      },
    },
  }
  const problems = check(plugin, { mcpServers: {} }, { submittable: false }).join("\n")
  for (const word of [
    "displayName",
    "kebab-case",
    "semver",
    "three default",
    "brandColor",
    "exactly one",
  ])
    expect(problems).toContain(word)
})

test("only the dev hosts count as development", () => {
  expect(isDevOrigin("https://crumb-dev.up.railway.app")).toBe(true)
  expect(isDevOrigin("http://localhost:3000")).toBe(true)
  expect(isDevOrigin("https://crumb.example.org")).toBe(false)
  expect(fill({ a: ["{{X}}-{{Y}}"] }, { X: "1" })).toEqual({ a: ["1-{{Y}}"] })
  expect(zip({ a: Buffer.from("hi") }).subarray(0, 4)).toEqual(Buffer.from([0x50, 0x4b, 3, 4]))
})
