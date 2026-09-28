import { describe, expect, test } from "bun:test"
import { manifest, validVersion } from "../src/manifest"

describe("manifest", () => {
  test("Chrome runs a service worker; Firefox an event page with its id", () => {
    const chrome = manifest("chrome", "1.2.3")
    const firefox = manifest("firefox", "1.2.3")
    expect(chrome.background).toEqual({ service_worker: "background.js" })
    expect(chrome.browser_specific_settings).toBeUndefined()
    expect(firefox.background).toEqual({ scripts: ["background.js"] })
    expect(firefox.browser_specific_settings).toMatchObject({
      gecko: { id: "crumb@evanbrobertson.github.io" },
    })
  })

  test("asks for as little as it can", () => {
    const m = manifest("chrome", "1.0.0")
    expect(m.permissions).toEqual(["storage", "activeTab"])
    expect(m.host_permissions).toBeUndefined()
    expect(m.web_accessible_resources).toBeUndefined()
  })

  test("versions are what stores accept", () => {
    expect(validVersion("0.1.0")).toBe(true)
    expect(validVersion("1.2.3.4")).toBe(true)
    expect(validVersion("1.2.3-main.4")).toBe(false)
    expect(validVersion("01.2")).toBe(false)
    expect(() => manifest("firefox", "v1.0.0")).toThrow()
  })
})
