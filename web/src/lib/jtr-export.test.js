import { describe, expect, test } from "bun:test"
import { bookmarklet, exportLibrary, IMPORT_URL, toLibrary } from "./jtr-export.js"

const raw = {
  collections: [
    { docId: "c1", name: " Weeknight ", recipes: ["r1"] },
    { docId: "c2", name: "", recipes: ["r1"] },
  ],
  recipes: [
    {
      docId: "r1",
      name: "Tomato Soup",
      sourceUrl: "https://example.com/soup",
      photoUrl: "https://img.example.com/soup.jpg",
      prepTime: 10 * 60_000_000,
      totalTime: 90 * 60_000_000,
      servings: 4,
      categories: ["Soup", " Dinner"],
      ingredients: [
        { type: "ingredient", name: "2 lb tomatoes" },
        { type: "group", name: "For the garnish:", steps: [{ name: "basil" }, { name: " " }] },
        { type: "ingredient", text: "salt" },
      ],
      instructions: [{ text: "Roast." }, { text: "Blend." }],
      notes: "  ",
    },
    { docId: "r2", name: "", sourceUrl: "app://mine", photoUrl: "gs://bucket/x.jpg" },
  ],
}

describe("toLibrary", () => {
  const library = toLibrary(raw)

  test("is the format Crumb's importer reads", () => {
    expect(library.format).toBe("just-the-recipe")
    expect(library.cookbooks).toEqual([{ name: "Weeknight", description: null }])
  })

  test("converts a recipe", () => {
    expect(library.recipes[0]).toEqual({
      title: "Tomato Soup",
      url: "https://example.com/soup",
      image: "https://img.example.com/soup.jpg",
      prepTime: "10m",
      cookTime: null,
      totalTime: "1h 30m",
      recipeYield: "4",
      recipeCategory: "Soup, Dinner",
      recipeCuisine: null,
      ingredients: [
        { name: null, items: ["2 lb tomatoes"] },
        { name: "For the garnish", items: ["basil"] },
        { name: null, items: ["salt"] },
      ],
      instructions: [{ name: null, items: ["Roast.", "Blend."] }],
      notes: null,
      cookbooks: ["Weeknight"],
    })
  })

  test("drops links that aren't http(s) and names untitled recipes", () => {
    expect(library.recipes[1]).toMatchObject({
      title: "Untitled recipe",
      url: null,
      image: null,
      ingredients: [],
      cookbooks: [],
    })
  })
})

describe("bookmarklet", () => {
  test("is one self-contained script with the Import page to fill in", () => {
    const href = bookmarklet()
    expect(href.startsWith("javascript:")).toBe(true)
    const source = decodeURIComponent(href.slice("javascript:".length))
    expect(source).toContain(`"${IMPORT_URL}"`)
    // Parses on its own: nothing it needs is left outside
    expect(() => new Function(source)).not.toThrow()
  })

  test("only runs on justtherecipe.com", async () => {
    const alerts = []
    globalThis.location = /** @type {any} */ ({ hostname: "crumb.example.com" })
    globalThis.window = /** @type {any} */ ({})
    globalThis.alert = (/** @type {string} */ m) => alerts.push(m)
    await exportLibrary(toLibrary, "https://crumb.example.com/import")
    expect(alerts[0]).toContain("justtherecipe.com")
  })
})
