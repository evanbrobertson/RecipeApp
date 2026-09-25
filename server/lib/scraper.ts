import * as cheerio from "cheerio"
import type { RecipeFields, RecipeSection } from "#shared/utils/recipe"
import { normalizeSections } from "#shared/utils/recipe"
import { browserAvailable, fetchWithBrowser } from "./browser"

export type ScrapedRecipe = RecipeFields

interface JsonLdNutrition {
  "@type"?: string
  [key: string]: string | undefined
}

interface JsonLdRecipe {
  "@type"?: string | string[]
  name?: string
  description?: string
  image?: string | Array<string | { url: string }> | { url: string }
  author?: string | { name: string } | Array<string | { name: string }>
  prepTime?: string
  cookTime?: string
  totalTime?: string
  recipeYield?: string | string[]
  recipeCategory?: string | string[]
  recipeCuisine?: string | string[]
  recipeIngredient?: string[]
  recipeInstructions?: string | Array<string | JsonLdInstruction | JsonLdSection>
  nutrition?: JsonLdNutrition
  [key: string]: unknown
}

interface JsonLdInstruction {
  "@type"?: string
  text?: string
  name?: string
  itemListElement?: JsonLdInstruction[]
}

interface JsonLdSection {
  "@type"?: string
  name?: string
  itemListElement?: JsonLdInstruction[]
  text?: string
}

const USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"

const PASTE_HINT = "Try copying the recipe text and pasting it instead."

/**
 * Scrapes a recipe page. Plain HTTP first (fast, cheap); if the site blocks it or the
 * recipe is rendered by JavaScript, retries in headless Chromium when it's installed.
 */
export async function scrapeRecipe(url: string): Promise<ScrapedRecipe> {
  const parsed = new URL(url)
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw createError({ statusCode: 400, statusMessage: "Only http(s) links are supported." })
  }

  let fetchProblem = ""
  try {
    const html = await $fetch<string>(url, {
      timeout: 15_000,
      responseType: "text",
      headers: {
        "User-Agent": USER_AGENT,
        Accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
      },
    })
    const recipe = parseRecipeHtml(html, url)
    if (recipe) return recipe
    fetchProblem = "Couldn't find a recipe on that page."
  } catch (err: any) {
    const status = err?.response?.status
    fetchProblem = status ? `The site responded with ${status}.` : "Couldn't reach that site."
  }

  if (browserAvailable()) {
    try {
      const recipe = parseRecipeHtml(await fetchWithBrowser(url), url)
      if (recipe) return recipe
      fetchProblem = "Couldn't find a recipe on that page, even in a real browser."
    } catch (err) {
      console.warn(`[scraper] browser fallback failed for ${url}:`, (err as Error).message)
      fetchProblem = `${fetchProblem} A real browser was blocked too.`
    }
  }

  throw createError({ statusCode: 422, statusMessage: `${fetchProblem} ${PASTE_HINT}` })
}

/** Extracts a recipe from page HTML (JSON-LD first, then microdata/selectors). */
export function parseRecipeHtml(html: string, url: string): ScrapedRecipe | null {
  const $ = cheerio.load(html)
  const jsonLd = extractJsonLd($)
  const recipe = jsonLd ? normalizeJsonLd(jsonLd, url, $) : extractFromHtml($, url)
  return finish(recipe, url)
}

/** Converts a schema.org Recipe object (e.g. from an export file) into recipe fields. */
export function recipeFromJsonLd(data: unknown, url = ""): ScrapedRecipe | null {
  const jsonLd = findRecipeInJsonLd(data)
  if (!jsonLd) return null
  const source =
    url || (typeof jsonLd.url === "string" && /^https?:\/\//.test(jsonLd.url) ? jsonLd.url : "")
  const recipe = finish(normalizeJsonLd(jsonLd, source, cheerio.load("")), source)
  if (recipe && !source) recipe.url = null
  return recipe
}

function finish(recipe: ScrapedRecipe, url: string): ScrapedRecipe | null {
  recipe.ingredients = normalizeSections(recipe.ingredients.map(decodeSection))
  recipe.instructions = normalizeSections(recipe.instructions.map(decodeSection))
  recipe.title = decodeText(recipe.title) || "Untitled recipe"
  recipe.description = recipe.description ? decodeText(recipe.description) : null
  recipe.image = url ? absoluteUrl(recipe.image, url) : recipe.image || null
  if (recipe.ingredients.length === 0 && recipe.instructions.length === 0) return null
  return recipe
}

/** Decodes HTML entities and strips any stray markup from scraped strings. */
function decodeText(value: string): string {
  if (!/[<&]/.test(value)) return value.replace(/\s+/g, " ").trim()
  return cheerio.load(`<body>${value}</body>`)("body").text().replace(/\s+/g, " ").trim()
}

function decodeSection(section: RecipeSection): RecipeSection {
  return {
    name: section.name ? decodeText(section.name) : null,
    items: section.items.map(decodeText),
  }
}

function absoluteUrl(value: string | null | undefined, base: string): string | null {
  if (!value) return null
  try {
    return new URL(value, base).toString()
  } catch {
    return null
  }
}

function extractJsonLd($: cheerio.CheerioAPI): JsonLdRecipe | null {
  const scripts = $('script[type="application/ld+json"]')

  for (let i = 0; i < scripts.length; i++) {
    try {
      const text = $(scripts[i]).html()
      if (!text) continue

      const data = JSON.parse(text)
      const recipe = findRecipeInJsonLd(data)
      if (recipe) return recipe
    } catch {
      continue
    }
  }

  return null
}

function findRecipeInJsonLd(data: unknown): JsonLdRecipe | null {
  if (!data || typeof data !== "object") return null

  if (Array.isArray(data)) {
    for (const item of data) {
      const result = findRecipeInJsonLd(item)
      if (result) return result
    }
    return null
  }

  const obj = data as Record<string, unknown>

  // Check if this object is a Recipe
  const type = obj["@type"]
  if (type) {
    const types = Array.isArray(type) ? type : [type]
    if (types.includes("Recipe")) return obj as unknown as JsonLdRecipe
  }

  // Check @graph wrapper
  if (Array.isArray(obj["@graph"])) {
    return findRecipeInJsonLd(obj["@graph"])
  }

  return null
}

function normalizeJsonLd(recipe: JsonLdRecipe, url: string, $: cheerio.CheerioAPI): ScrapedRecipe {
  // Try JSON-LD flat list first, fall back to HTML ingredient groups
  let ingredients = normalizeIngredientSections(recipe.recipeIngredient)
  const hasOnlyOneUnnamedSection = ingredients.length === 1 && ingredients[0]!.name === null
  if (hasOnlyOneUnnamedSection) {
    const htmlGroups = extractIngredientGroupsFromHtml($)
    if (htmlGroups.length > 0) {
      ingredients = htmlGroups
    }
  }

  return {
    url,
    title: recipe.name || "Untitled recipe",
    description: recipe.description || null,
    image: normalizeImage(recipe.image),
    author: normalizeAuthor(recipe.author),
    prepTime: formatDuration(recipe.prepTime),
    cookTime: formatDuration(recipe.cookTime),
    totalTime: formatDuration(recipe.totalTime),
    freezeTime: computeAdditionalTime(recipe.prepTime, recipe.cookTime, recipe.totalTime),
    recipeYield: normalizeStringOrArray(recipe.recipeYield),
    recipeCategory: normalizeStringOrArray(recipe.recipeCategory),
    recipeCuisine: normalizeStringOrArray(recipe.recipeCuisine),
    ingredients,
    instructions: normalizeInstructions(recipe.recipeInstructions),
    nutrition: normalizeNutrition(recipe.nutrition),
    notes: null,
  }
}

function normalizeImage(image: JsonLdRecipe["image"]): string | null {
  if (!image) return null
  if (typeof image === "string") return image
  if (Array.isArray(image)) return normalizeImage(image[0] as JsonLdRecipe["image"])
  if (typeof image === "object" && "url" in image) return image.url
  return null
}

function normalizeAuthor(author: JsonLdRecipe["author"]): string | null {
  if (!author) return null
  if (typeof author === "string") return author
  if (Array.isArray(author)) {
    return author.map((a) => (typeof a === "string" ? a : a.name)).join(", ")
  }
  if (typeof author === "object" && "name" in author) return author.name
  return null
}

function normalizeStringOrArray(value: string | string[] | undefined): string | null {
  if (!value) return null
  if (Array.isArray(value)) return value.join(", ")
  return value
}

/**
 * Extracts ingredient groups from HTML markup.
 * Supports common recipe plugins: WPRM, Tasty Recipes, and generic
 * patterns where ingredient items are grouped under headings.
 * Returns an empty array if no grouped structure is found.
 */
function extractIngredientGroupsFromHtml($: cheerio.CheerioAPI): RecipeSection[] {
  const sections: RecipeSection[] = []

  // WPRM (WP Recipe Maker) — most common WordPress recipe plugin
  const wprmGroups = $(".wprm-recipe-ingredient-group")
  if (wprmGroups.length > 0) {
    wprmGroups.each((_, group) => {
      const name = $(group).find(".wprm-recipe-group-name").first().text().trim() || null
      const items: string[] = []
      $(group)
        .find(".wprm-recipe-ingredient")
        .each((_, li) => {
          const text = $(li).text().trim()
          if (text) items.push(text)
        })
      if (items.length > 0) sections.push({ name, items })
    })
    if (sections.length > 1 || (sections.length === 1 && sections[0]!.name)) {
      return sections
    }
    sections.length = 0
  }

  // Tasty Recipes plugin
  const tastyBody = $(".tasty-recipes-ingredients-body")
  if (tastyBody.length > 0) {
    tastyBody.find("h4, h3, h2").each((_, heading) => {
      const name = $(heading).text().trim() || null
      const items: string[] = []
      let next = $(heading).next()
      while (next.length > 0 && !next.is("h4, h3, h2")) {
        if (next.is("ul, ol")) {
          next.find("li").each((_, li) => {
            const text = $(li).text().trim()
            if (text) items.push(text)
          })
        }
        next = next.next()
      }
      if (items.length > 0) sections.push({ name, items })
    })
    if (sections.length > 0) return sections
  }

  // Generic: look for ingredient containers with internal headings
  const containers = $(".recipe-ingredients, .ingredients-section, [class*='ingredient-group']")
  if (containers.length > 0) {
    containers.each((_, container) => {
      $(container)
        .find("h2, h3, h4, h5")
        .each((_, heading) => {
          const name = $(heading).text().trim() || null
          const items: string[] = []
          let next = $(heading).next()
          while (next.length > 0 && !next.is("h2, h3, h4, h5")) {
            if (next.is("ul, ol")) {
              next.find("li").each((_, li) => {
                const text = $(li).text().trim()
                if (text) items.push(text)
              })
            }
            next = next.next()
          }
          if (items.length > 0) sections.push({ name, items })
        })
    })
    if (sections.length > 0) return sections
  }

  return []
}

/**
 * Detects whether a flat ingredient string looks like a section header.
 * Common patterns: "For the sauce:", "Cake:", "FROSTING", etc.
 */
function isIngredientSectionHeader(text: string): boolean {
  const trimmed = text.trim()
  // Ends with ":" and is short (no measurement-like content)
  if (trimmed.endsWith(":") && trimmed.length < 60) {
    // Shouldn't contain typical measurement patterns
    if (!/\d/.test(trimmed)) return true
  }
  return false
}

/**
 * Splits a flat ingredient list into sections by detecting header items.
 */
function normalizeIngredientSections(ingredients: string[] | undefined): RecipeSection[] {
  if (!ingredients || ingredients.length === 0) return [{ name: null, items: [] }]

  const sections: RecipeSection[] = []
  let current: RecipeSection = { name: null, items: [] }

  for (const item of ingredients) {
    if (isIngredientSectionHeader(item)) {
      // Push the current section if it has items
      if (current.items.length > 0) {
        sections.push(current)
      }
      // Start a new section with the header (strip trailing colon)
      current = { name: item.trim().replace(/:$/, "").trim(), items: [] }
    } else {
      current.items.push(item)
    }
  }

  // Push the last section
  if (current.items.length > 0 || sections.length > 0) {
    sections.push(current)
  }

  return sections.length > 0 ? sections : [{ name: null, items: [] }]
}

function normalizeInstructions(instructions: JsonLdRecipe["recipeInstructions"]): RecipeSection[] {
  if (!instructions) return [{ name: null, items: [] }]
  if (typeof instructions === "string") {
    const items = instructions
      .split(/\n+/)
      .map((s) => s.trim())
      .filter(Boolean)
    return [{ name: null, items }]
  }
  if (!Array.isArray(instructions)) return [{ name: null, items: [] }]

  // Check if there are any HowToSection entries
  const hasSections = instructions.some(
    (item) => typeof item !== "string" && item["@type"] === "HowToSection",
  )

  if (hasSections) {
    const sections: RecipeSection[] = []
    let current: RecipeSection = { name: null, items: [] }

    for (const item of instructions) {
      if (typeof item === "string") {
        current.items.push(item.trim())
      } else if (item["@type"] === "HowToStep" && item.text) {
        current.items.push(item.text.trim())
      } else if (item["@type"] === "HowToSection") {
        // Push previous section if it has items
        if (current.items.length > 0) {
          sections.push(current)
        }
        // Start new section
        const sectionItems: string[] = []
        if (item.itemListElement) {
          for (const step of item.itemListElement) {
            if (step.text) sectionItems.push(step.text.trim())
          }
        }
        sections.push({ name: item.name || null, items: sectionItems })
        current = { name: null, items: [] }
      }
    }

    // Push any trailing steps
    if (current.items.length > 0) {
      sections.push(current)
    }

    return sections.length > 0 ? sections : [{ name: null, items: [] }]
  }

  // No sections — flat list of steps
  const items: string[] = []
  for (const item of instructions) {
    if (typeof item === "string") {
      items.push(item.trim())
    } else if (item["@type"] === "HowToStep" && item.text) {
      items.push(item.text.trim())
    }
  }
  return [{ name: null, items }]
}

function normalizeNutrition(nutrition: JsonLdNutrition | undefined): Record<string, string> | null {
  if (!nutrition || typeof nutrition !== "object") return null

  const result: Record<string, string> = {}
  const fields = [
    "calories",
    "fatContent",
    "saturatedFatContent",
    "unsaturatedFatContent",
    "transFatContent",
    "carbohydrateContent",
    "sugarContent",
    "fiberContent",
    "proteinContent",
    "cholesterolContent",
    "sodiumContent",
    "servingSize",
  ]

  for (const field of fields) {
    const value = nutrition[field]
    if (value && typeof value === "string") {
      result[field] = value
    }
  }

  return Object.keys(result).length > 0 ? result : null
}

function parseDurationMinutes(iso: string | undefined): number | null {
  if (!iso) return null
  const match = iso.match(/^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$/i)
  if (!match) return null
  return parseInt(match[1] || "0") * 60 + parseInt(match[2] || "0")
}

function formatMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  const parts = []
  if (h) parts.push(`${h}h`)
  if (m) parts.push(`${m}m`)
  return parts.join(" ")
}

function computeAdditionalTime(
  prepTime: string | undefined,
  cookTime: string | undefined,
  totalTime: string | undefined,
): string | null {
  const total = parseDurationMinutes(totalTime)
  const prep = parseDurationMinutes(prepTime) || 0
  const cook = parseDurationMinutes(cookTime) || 0
  if (!total) return null
  const additional = total - prep - cook
  if (additional <= 0) return null
  return formatMinutes(additional) || null
}

function formatDuration(iso: string | undefined): string | null {
  if (!iso) return null
  const match = iso.match(/^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$/i)
  if (!match) return iso
  const hours = match[1] ? `${match[1]}h` : ""
  const minutes = match[2] ? `${match[2]}m` : ""
  const parts = [hours, minutes].filter(Boolean)
  return parts.length > 0 ? parts.join(" ") : null
}

function extractFromHtml($: cheerio.CheerioAPI, url: string): ScrapedRecipe {
  const title =
    $('[itemprop="name"]').first().text().trim() ||
    $("h1").first().text().trim() ||
    $("title").text().trim() ||
    "Untitled recipe"

  const description = $('[itemprop="description"]').first().text().trim() || null
  const image =
    $('[itemprop="image"]').first().attr("src") ||
    $('[itemprop="image"]').first().attr("content") ||
    $('meta[property="og:image"]').attr("content") ||
    null

  const author = $('[itemprop="author"]').first().text().trim() || null

  const rawIngredients: string[] = []
  $('[itemprop="recipeIngredient"], [itemprop="ingredients"]').each((_, el) => {
    const text = $(el).text().trim()
    if (text) rawIngredients.push(text)
  })

  const rawInstructions: string[] = []
  $('[itemprop="recipeInstructions"]').each((_, el) => {
    const tag = el.tagName?.toLowerCase()
    if (tag === "ol" || tag === "ul") {
      $(el)
        .find("li")
        .each((_, li) => {
          const text = $(li).text().trim()
          if (text) rawInstructions.push(text)
        })
    } else {
      const text = $(el).text().trim()
      if (text) {
        text
          .split(/\n+/)
          .map((s) => s.trim())
          .filter(Boolean)
          .forEach((s) => rawInstructions.push(s))
      }
    }
  })

  const prepTimeRaw = $('[itemprop="prepTime"]').first().attr("content") || undefined
  const cookTimeRaw = $('[itemprop="cookTime"]').first().attr("content") || undefined
  const freezeTimeRaw = $('[itemprop="freezeTime"]').first().attr("content") || undefined
  const totalTimeRaw = $('[itemprop="totalTime"]').first().attr("content") || undefined

  return {
    url,
    title,
    description,
    image,
    author,
    prepTime: formatDuration(prepTimeRaw),
    cookTime: formatDuration(cookTimeRaw),
    totalTime: formatDuration(totalTimeRaw),
    freezeTime: formatDuration(freezeTimeRaw),
    recipeYield: $('[itemprop="recipeYield"]').first().text().trim() || null,
    recipeCategory: $('[itemprop="recipeCategory"]').first().text().trim() || null,
    recipeCuisine: $('[itemprop="recipeCuisine"]').first().text().trim() || null,
    ingredients: normalizeIngredientSections(rawIngredients),
    instructions: [{ name: null, items: rawInstructions }],
    nutrition: null,
    notes: null,
  }
}
