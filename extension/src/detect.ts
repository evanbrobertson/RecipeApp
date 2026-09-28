/**
 * Whether a page is a recipe, from what it tells search engines: a schema.org Recipe in its
 * JSON-LD (on its own, in a list, in an `@graph`, or with several types), else Recipe
 * microdata. The same signals Crumb's scraper reads first (src/scraper.rs), so a page that
 * prompts is one Crumb can read.
 *
 * Pure (strings in, a name out), so it's tested without a browser; the content script
 * gathers the strings.
 */

/** What a page's structured data says about it. */
export interface Found {
  /** The recipe's name, when the page gives one. */
  name: string
}

const RECIPE_TYPES = new Set([
  "recipe",
  "schema:recipe",
  "http://schema.org/recipe",
  "https://schema.org/recipe",
])

function isRecipeType(type: unknown): boolean {
  const types = Array.isArray(type) ? type : [type]
  return types.some((t) => typeof t === "string" && RECIPE_TYPES.has(t.trim().toLowerCase()))
}

function nameOf(node: Record<string, unknown>): string {
  const name = node.name ?? node.headline
  return typeof name === "string" ? name.trim() : ""
}

/** The first Recipe node anywhere in a JSON-LD value (depth-limited: pages nest oddly). */
function findRecipe(value: unknown, depth = 0): Found | null {
  if (depth > 6 || value === null || typeof value !== "object") return null
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findRecipe(item, depth + 1)
      if (found) return found
    }
    return null
  }
  const node = value as Record<string, unknown>
  if (isRecipeType(node["@type"])) return { name: nameOf(node) }
  for (const key of ["@graph", "mainEntity", "mainEntityOfPage", "itemListElement", "item"]) {
    const found = findRecipe(node[key], depth + 1)
    if (found) return found
  }
  return null
}

/** A Recipe in any of a page's `<script type="application/ld+json">` blocks. */
export function recipeInJsonLd(blocks: readonly string[]): Found | null {
  for (const text of blocks) {
    let value: unknown
    try {
      value = JSON.parse(text)
    } catch {
      // Some sites leave raw control characters in their JSON-LD
      try {
        // oxlint-disable-next-line no-control-regex -- exactly what this strips
        value = JSON.parse(text.replace(/[\u0000-\u001f]+/g, " "))
      } catch {
        continue
      }
    }
    const found = findRecipe(value)
    if (found) return found
  }
  return null
}

/** Microdata: `itemtype="https://schema.org/Recipe"` (any of the forms above). */
export function isRecipeItemType(itemtype: string | null): boolean {
  if (!itemtype) return false
  return itemtype.split(/\s+/).some((t) => isRecipeType(t.replace(/\/$/, "")))
}
