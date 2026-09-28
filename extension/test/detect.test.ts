import { describe, expect, test } from "bun:test"
import { isRecipeItemType, recipeInJsonLd } from "../src/detect"

const ld = (value: unknown) => JSON.stringify(value)

describe("recipeInJsonLd", () => {
  test("a Recipe on its own", () => {
    expect(recipeInJsonLd([ld({ "@type": "Recipe", name: " Leek Soup " })])).toEqual({
      name: "Leek Soup",
    })
  })

  test("in a Yoast-style @graph, beside other nodes", () => {
    const graph = {
      "@context": "https://schema.org",
      "@graph": [
        { "@type": "WebSite", name: "A Blog" },
        { "@type": ["Article", "BlogPosting"], headline: "My trip" },
        { "@type": ["Recipe", "NewsArticle"], name: "Pad Thai" },
      ],
    }
    expect(recipeInJsonLd([ld(graph)])?.name).toBe("Pad Thai")
  })

  test("in a top-level list, and after a block that isn't one", () => {
    const blocks = [
      ld({ "@type": "Organization" }),
      ld([{ "@type": "BreadcrumbList" }, { "@type": "recipe" }]),
    ]
    expect(recipeInJsonLd(blocks)).toEqual({ name: "" })
  })

  test("as a page's mainEntity, and in full-URL form", () => {
    const page = {
      "@type": "WebPage",
      mainEntity: { "@type": "https://schema.org/Recipe", name: "Dal" },
    }
    expect(recipeInJsonLd([ld(page)])?.name).toBe("Dal")
  })

  test("survives raw control characters and skips broken JSON", () => {
    const raw = '{"@type": "Recipe", "name": "Tab\tSoup"}'
    expect(recipeInJsonLd(["{not json", raw])?.name).toBe("Tab Soup")
  })

  test("nothing on pages without one", () => {
    expect(recipeInJsonLd([])).toBeNull()
    expect(recipeInJsonLd([ld({ "@type": "Article", name: "Recipes I love" })])).toBeNull()
    expect(recipeInJsonLd([ld({ "@type": "RecipeCollection" })])).toBeNull()
  })

  test("stops at absurd nesting", () => {
    let deep: unknown = { "@type": "Recipe", name: "Deep" }
    for (let i = 0; i < 20; i++) deep = { "@graph": [deep] }
    expect(recipeInJsonLd([ld(deep)])).toBeNull()
  })
})

describe("isRecipeItemType", () => {
  test("schema.org's forms", () => {
    expect(isRecipeItemType("http://schema.org/Recipe")).toBe(true)
    expect(isRecipeItemType("https://schema.org/Recipe/")).toBe(true)
    expect(isRecipeItemType("https://schema.org/Thing https://schema.org/Recipe")).toBe(true)
  })

  test("not other types", () => {
    expect(isRecipeItemType(null)).toBe(false)
    expect(isRecipeItemType("https://schema.org/Article")).toBe(false)
  })
})
