import { describe, expect, test } from "bun:test"
import { termsNote, termsSite } from "../src/terms"

describe("termsSite", () => {
  test("a listed host, with or without www or another subdomain", () => {
    for (const host of [
      "allrecipes.com",
      "www.allrecipes.com",
      "m.allrecipes.com",
      "WWW.SeriousEats.com",
    ]) {
      expect(termsSite(host)).not.toBeNull()
    }
    expect(termsSite("www.allrecipes.com")?.name).toBe("Allrecipes")
  })

  test("the People Inc recipe brands are all there", () => {
    for (const host of [
      "allrecipes.com",
      "seriouseats.com",
      "simplyrecipes.com",
      "foodandwine.com",
      "eatingwell.com",
      "marthastewart.com",
      "bhg.com",
      "realsimple.com",
      "southernliving.com",
      "myrecipes.com",
      "thespruceeats.com",
    ]) {
      expect(termsSite(host), host).not.toBeNull()
    }
  })

  test("other sites, look-alikes and hosts that only end the same way are not", () => {
    for (const host of [
      "cooking.nytimes.com",
      "notallrecipes.com",
      "allrecipes.com.example.org",
      "",
    ]) {
      expect(termsSite(host), host).toBeNull()
    }
  })

  test("an explicit list is used as given", () => {
    const all = [{ name: "Example", hosts: ["example.com"] }]
    expect(termsSite("www.example.com", all)?.name).toBe("Example")
    expect(termsSite("allrecipes.com", all)).toBeNull()
  })
})

describe("termsNote", () => {
  test("says Crumb reads it from this page, by the site's name", () => {
    expect(termsNote("www.foodandwine.com")).toBe(
      "Food & Wine's terms don't allow Crumb's server to fetch it, so Crumb will read it from this page.",
    )
  })

  test("nothing on an ordinary site", () => {
    expect(termsNote("example.com")).toBeNull()
  })
})
