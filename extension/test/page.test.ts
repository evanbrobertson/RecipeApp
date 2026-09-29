import { describe, expect, test } from "bun:test"
import { parseHTML } from "linkedom"
import { cardHtml, MAX_READING_BYTES, readRecipe } from "../src/page"

const docOf = (html: string) => parseHTML(html).document as unknown as Document
const ld = (value: unknown) =>
  `<script type="application/ld+json">${JSON.stringify(value)}</script>`

const RECIPE = { "@type": "Recipe", name: "Leek Soup", recipeIngredient: ["2 leeks"] }

describe("readRecipe", () => {
  test("JSON-LD in an @graph is sent whole, with the og tags", () => {
    const graph = {
      "@context": "https://schema.org",
      "@graph": [{ "@type": "WebSite", name: "A Blog" }, RECIPE],
    }
    const doc = docOf(`<html><head>
      <meta property="og:image" content="https://cdn.example/soup.jpg">
      <meta property="og:title" content="Leek Soup | A Blog">
      ${ld(graph)}${ld({ "@type": "Organization", name: "Unrelated" })}
      </head><body><p>Comments, cookies banner and the rest</p></body></html>`)
    const reading = readRecipe(doc)
    expect(reading).toEqual({
      jsonLd: [JSON.stringify(graph)],
      image: "https://cdn.example/soup.jpg",
      title: "Leek Soup | A Blog",
    })
  })

  test("without JSON-LD, the recipe card's HTML stands in, cleaned", () => {
    const doc = docOf(`<html><body>
      <header>Site nav and account menu</header>
      <div class="wprm-recipe-container" data-nonce="secret" onclick="steal()" id="r1">
        <h2 class="wprm-recipe-name">Leek Soup</h2>
        <ul><li class="wprm-recipe-ingredient">2 leeks</li></ul>
        <form action="/rate"><input name="token" value="abc"><button>Rate</button></form>
        <script>window.token = "abc"</script>
      </div>
      <section id="comments">A commenter's private words</section></body></html>`)
    const reading = readRecipe(doc)!
    expect(reading.jsonLd).toEqual([])
    const card = reading.card!
    expect(card).toContain("Leek Soup")
    expect(card).toContain("2 leeks")
    expect(card).toContain('class="wprm-recipe-container"')
    for (const gone of ["secret", "onclick", "steal", "token", "abc", "<form", "<script", "nav"]) {
      expect(card).not.toContain(gone)
    }
    expect(card).not.toContain("commenter")
  })

  test("every card plugin's container is found", () => {
    for (const [open, close] of [
      ['<div class="tasty-recipes">', "</div>"],
      ['<div class="mv-create-card">', "</div>"],
      ['<div id="zlrecipe-container">', "</div>"],
      ['<div class="wp-block-wpzoom-recipe-card-block-recipe-card">', "</div>"],
      ['<article itemscope itemtype="https://schema.org/Recipe">', "</article>"],
    ] as const) {
      const reading = readRecipe(docOf(`<body>nav ${open}Soup${close} footer</body>`))
      expect(reading?.card).toContain("Soup")
      expect(reading?.card).not.toContain("footer")
    }
  })

  test("microdata on the body isn't a card: that would be the whole page", () => {
    const doc = docOf(`<body itemscope itemtype="https://schema.org/Recipe">Everything</body>`)
    expect(readRecipe(doc)).toBeNull()
  })

  test("nothing to send when the page has no recipe", () => {
    const doc = docOf(
      `<html><head>${ld({ "@type": "Article" })}<meta property="og:title" content="x"></head><body>Hi</body></html>`,
    )
    expect(readRecipe(doc)).toBeNull()
  })

  test("only http(s) images are passed on", () => {
    const doc = docOf(
      `<head><meta property="og:image" content="javascript:alert(1)">${ld(RECIPE)}</head>`,
    )
    expect(readRecipe(doc)).toEqual({ jsonLd: [JSON.stringify(RECIPE)] })
  })

  test("the size cap holds: what doesn't fit isn't sent, and nothing is cut", () => {
    const big = { ...RECIPE, description: "x".repeat(MAX_READING_BYTES) }
    // Too big on its own: skipped, and a smaller one beside it is still sent
    const both = readRecipe(docOf(`<head>${ld(big)}${ld(RECIPE)}</head>`))!
    expect(both.jsonLd).toEqual([JSON.stringify(RECIPE)])
    // Only an oversize block and no card: nothing
    expect(readRecipe(docOf(`<head>${ld(big)}</head>`))).toBeNull()
    // An oversize card is dropped too
    const card = `<div class="tasty-recipes">${"y".repeat(MAX_READING_BYTES)}</div>`
    expect(readRecipe(docOf(`<body>${card}</body>`))).toBeNull()
    // Whatever is sent is within the cap
    const reading = readRecipe(docOf(`<head>${ld(RECIPE)}</head>`))!
    expect(new TextEncoder().encode(JSON.stringify(reading)).length).toBeLessThan(MAX_READING_BYTES)
  })

  test("only the documented fields ever leave the page", () => {
    const doc = docOf(`<html><head>
      <meta property="og:image" content="https://cdn.example/a.jpg">
      <meta name="csrf-token" content="secret-token">
      <link rel="canonical" href="https://example.com/soup">
      ${ld(RECIPE)}</head><body>Signed in as someone@example.com</body></html>`)
    const reading = readRecipe(doc)!
    expect(Object.keys(reading).sort()).toEqual(["image", "jsonLd"])
    expect(JSON.stringify(reading)).not.toContain("secret-token")
    expect(JSON.stringify(reading)).not.toContain("someone@example.com")
  })
})

describe("cardHtml", () => {
  test("keeps the attributes the parser reads and drops the rest", () => {
    const doc = docOf(
      `<div id="c" class="k" itemprop="recipeIngredient" data-track="1" aria-label="x" content="1 cup"><a href="/x" onclick="y()">z</a></div>`,
    )
    const html = cardHtml(doc.querySelector("#c")!)
    expect(html).toContain('itemprop="recipeIngredient"')
    expect(html).toContain('content="1 cup"')
    expect(html).toContain('href="/x"')
    expect(html).not.toMatch(/data-track|aria-label|onclick/)
  })
})
