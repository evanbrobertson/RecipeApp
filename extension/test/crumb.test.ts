import { describe, expect, test } from "bun:test"
import { crumbOrigin, mayOfferItself, normalize, previewUrl, readable, siteOf } from "../src/crumb"

describe("crumbOrigin", () => {
  test("a bare host is https", () => {
    expect(crumbOrigin(" crumb.example.com ")).toBe("https://crumb.example.com")
  })

  test("keeps the scheme and port, drops any path", () => {
    expect(crumbOrigin("http://localhost:3000/recipes")).toBe("http://localhost:3000")
    expect(crumbOrigin("https://192.168.1.4:8443")).toBe("https://192.168.1.4:8443")
  })

  test("refuses what isn't an address", () => {
    expect(crumbOrigin("")).toBeNull()
    expect(crumbOrigin("crumb")).toBeNull()
    expect(crumbOrigin("javascript:alert(1)")).toBeNull()
    expect(crumbOrigin("ftp://crumb.example.com")).toBeNull()
  })
})

test("previewUrl encodes the whole page address", () => {
  expect(previewUrl("https://c.example", "https://site.example/soup?x=1&y=2#top")).toBe(
    "https://c.example/preview?url=https%3A%2F%2Fsite.example%2Fsoup%3Fx%3D1%26y%3D2%23top",
  )
})

test("readable: http(s) pages other than the Crumb", () => {
  const crumb = "https://c.example"
  expect(readable("https://site.example/soup", crumb)).toBe(true)
  expect(readable("https://c.example/recipes/1", crumb)).toBe(false)
  expect(readable("chrome://extensions", crumb)).toBe(false)
  expect(readable("file:///tmp/a.html", crumb)).toBe(false)
  expect(readable(undefined, crumb)).toBe(false)
})

test("siteOf drops www. and case", () => {
  expect(siteOf("WWW.Seriouseats.com")).toBe("seriouseats.com")
})

test("normalize fills defaults and cleans what was stored", () => {
  expect(normalize({})).toEqual({ crumb: "", prompt: true, muted: [] })
  expect(normalize({ crumb: "c.example/x", prompt: false, muted: ["a.com", 3] })).toEqual({
    crumb: "https://c.example",
    prompt: false,
    muted: ["a.com"],
  })
  expect(normalize({ crumb: "nope", prompt: "yes", muted: "a.com" })).toEqual({
    crumb: "",
    prompt: true,
    muted: [],
  })
})

describe("mayOfferItself", () => {
  test("https, or a local http Crumb; not plain http elsewhere", () => {
    expect(mayOfferItself("https://crumb.example.com")).toBe(true)
    expect(mayOfferItself("http://localhost:3000")).toBe(true)
    expect(mayOfferItself("http://127.0.0.1:3000")).toBe(true)
    expect(mayOfferItself("http://crumb.example.com")).toBe(false)
    expect(mayOfferItself("file:///x")).toBe(false)
    expect(mayOfferItself("nope")).toBe(false)
  })
})
