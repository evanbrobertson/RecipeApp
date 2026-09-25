import { gunzipSync, strFromU8, unzipSync } from "fflate"
import type { RecipeFields, RecipeSection } from "#shared/utils/recipe"
import { normalizeSections } from "#shared/utils/recipe"
import { parseRecipeHtml, recipeFromJsonLd } from "./scraper"
import { parseRecipeText } from "./text-parser"
import { extractRecipeWithClaude } from "./claude-extract"

/**
 * Turns uploaded files into recipes. Supports:
 * - Just the Recipe PDFs (its only export: Print → Save as PDF, one recipe per file)
 * - Paprika (.paprikarecipes zip of gzipped JSON, or a single .paprikarecipe)
 * - schema.org Recipe JSON (Mealie, Tandoor, Nextcloud Cookbook...) and this app's own backup
 * - Saved web pages (.html) and plain text / Markdown (recipes separated by "---")
 * - .zip archives containing any of the above
 */

export interface ImportedRecipe {
  fields: RecipeFields
  cookbooks?: string[]
}

export interface BackupFile {
  format: "just-the-recipe"
  version: number
  recipes: (RecipeFields & { cookbooks?: string[] })[]
  cookbooks?: { name: string; description?: string | null }[]
}

const lines = (value: unknown): string[] =>
  typeof value === "string"
    ? value
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter(Boolean)
    : []

const str = (value: unknown): string | null =>
  typeof value === "string" && value.trim() ? value.trim() : null

const httpUrl = (value: unknown): string | null => {
  const v = str(value)
  return v && /^https?:\/\//i.test(v) ? v : null
}

/** Splits flat lines into sections, treating short "Header:" lines as section names. */
function sectionsFromLines(items: string[]): RecipeSection[] {
  const sections: RecipeSection[] = [{ name: null, items: [] }]
  for (const item of items) {
    const header = item.match(/^(?:#+\s*)?([^:\d]{2,50}):$/)
    if (header) sections.push({ name: header[1]!.trim(), items: [] })
    else sections[sections.length - 1]!.items.push(item.replace(/^(?:[-*•]|\d+[.)])\s+/, ""))
  }
  return normalizeSections(sections)
}

// ─── Paprika ────────────────────────────────────────────────────────────────

function fromPaprika(p: Record<string, unknown>): ImportedRecipe | null {
  const title = str(p.name)
  if (!title) return null
  const notes = [str(p.notes), str(p.nutritional_info) && `Nutrition: ${str(p.nutritional_info)}`]
    .filter(Boolean)
    .join("\n\n")
  return {
    fields: {
      title,
      description: str(p.description),
      url: httpUrl(p.source_url),
      image: httpUrl(p.image_url),
      author: str(p.source),
      prepTime: str(p.prep_time),
      cookTime: str(p.cook_time),
      totalTime: str(p.total_time),
      recipeYield: str(p.servings),
      recipeCategory: Array.isArray(p.categories) ? str(p.categories[0]) : null,
      ingredients: sectionsFromLines(lines(p.ingredients)),
      instructions: sectionsFromLines(lines(p.directions)),
      notes: notes || null,
    },
    cookbooks: Array.isArray(p.categories)
      ? p.categories.filter((c): c is string => typeof c === "string")
      : [],
  }
}

function isPaprika(o: Record<string, unknown>) {
  return "directions" in o && "ingredients" in o && typeof o.ingredients === "string"
}

// ─── JSON (schema.org, Mealie, our backup) ─────────────────────────────────

/** Mealie stores ingredients as objects and steps as { text } without @type. */
function fromMealie(o: Record<string, unknown>): ImportedRecipe | null {
  const title = str(o.name)
  if (!title || !Array.isArray(o.recipeIngredient)) return null
  const ingredients = (o.recipeIngredient as unknown[])
    .map((i) =>
      typeof i === "string"
        ? i
        : (str((i as any)?.display) ??
          str((i as any)?.originalText) ??
          str((i as any)?.note) ??
          ""),
    )
    .filter(Boolean)
  const steps = Array.isArray(o.recipeInstructions)
    ? (o.recipeInstructions as unknown[])
        .map((s) => (typeof s === "string" ? s : (str((s as any)?.text) ?? "")))
        .filter(Boolean)
    : lines(o.recipeInstructions)
  return {
    fields: {
      title,
      description: str(o.description),
      url: httpUrl(o.orgURL) ?? httpUrl(o.url),
      prepTime: str(o.prepTime),
      cookTime: str(o.performTime) ?? str(o.cookTime),
      totalTime: str(o.totalTime),
      recipeYield: str(o.recipeYield),
      ingredients: normalizeSections([{ name: null, items: ingredients }]),
      instructions: normalizeSections([{ name: null, items: steps }]),
      notes: Array.isArray(o.notes)
        ? (o.notes as any[])
            .map((n) => [n?.title, n?.text].filter(Boolean).join(": "))
            .join("\n") || null
        : null,
    },
  }
}

function fromJsonValue(data: unknown): ImportedRecipe[] {
  if (Array.isArray(data)) return data.flatMap(fromJsonValue)
  if (!data || typeof data !== "object") return []
  const o = data as Record<string, unknown>

  if (o.format === "just-the-recipe" && Array.isArray(o.recipes)) {
    return (o as unknown as BackupFile).recipes.map(({ cookbooks, ...fields }) => ({
      fields,
      cookbooks,
    }))
  }
  if (isPaprika(o)) {
    const r = fromPaprika(o)
    return r ? [r] : []
  }
  const schema = recipeFromJsonLd(o)
  // Mealie's objects don't carry "@type": "Recipe"
  if (schema && (schema.ingredients.length || schema.instructions.length))
    return [{ fields: schema }]
  const mealie = fromMealie(o)
  if (mealie) return [mealie]
  // Containers: { recipes: [...] } or { items: [...] }
  for (const key of ["recipes", "items", "data"]) {
    if (Array.isArray(o[key])) return fromJsonValue(o[key])
  }
  return []
}

// ─── Text & PDF ─────────────────────────────────────────────────────────────

async function fromText(text: string): Promise<ImportedRecipe[]> {
  const chunks = text
    .split(/^\s*(?:-{3,}|={3,}|\f)\s*$/m)
    .map((c) => c.trim())
    .filter((c) => c.length > 20)
  const out: ImportedRecipe[] = []
  for (const chunk of chunks) {
    const fields = (await extractRecipeWithClaude(chunk)) ?? parseRecipeText(chunk).recipe
    if (fields.ingredients.length || fields.instructions.length) out.push({ fields })
  }
  return out
}

async function fromPdf(bytes: Uint8Array): Promise<ImportedRecipe[]> {
  const { extractText, getDocumentProxy } = await import("unpdf")
  const pdf = await getDocumentProxy(bytes)
  const { text } = await extractText(pdf, { mergePages: true })
  // JTR PDFs are one recipe per file, often with a footer; drop page furniture
  const cleaned = (Array.isArray(text) ? text.join("\n") : text)
    .split("\n")
    .filter(
      (l) =>
        !/^(page \d+( of \d+)?|justtherecipe\.com.*|made with just the recipe.*)$/i.test(l.trim()),
    )
    .join("\n")
  const fields = (await extractRecipeWithClaude(cleaned)) ?? parseRecipeText(cleaned).recipe
  return fields.ingredients.length || fields.instructions.length ? [{ fields }] : []
}

// ─── Dispatcher ─────────────────────────────────────────────────────────────

const ext = (name: string) => name.toLowerCase().match(/\.([a-z0-9]+)$/)?.[1] ?? ""

export async function recipesFromFile(
  name: string,
  bytes: Uint8Array,
  depth = 0,
): Promise<ImportedRecipe[]> {
  const kind = ext(name)
  const isZip = bytes[0] === 0x50 && bytes[1] === 0x4b
  const isGzip = bytes[0] === 0x1f && bytes[1] === 0x8b

  if (isZip && depth < 2) {
    const entries = unzipSync(bytes)
    const out: ImportedRecipe[] = []
    for (const [entryName, data] of Object.entries(entries)) {
      if (entryName.endsWith("/") || entryName.startsWith("__MACOSX")) continue
      // Paprika entries have no useful extension handling beyond gzip sniffing
      out.push(...(await recipesFromFile(entryName, data, depth + 1).catch(() => [])))
    }
    return out
  }
  if (isGzip) return recipesFromFile(name.replace(/\.gz$/, ""), gunzipSync(bytes), depth + 1)

  if (kind === "pdf") return fromPdf(bytes)

  const text = strFromU8(bytes)
  if (kind === "json" || kind === "paprikarecipe" || /^\s*[[{]/.test(text)) {
    try {
      return fromJsonValue(JSON.parse(text))
    } catch {
      if (kind === "json") return []
    }
  }
  if (kind === "html" || kind === "htm" || /^\s*<(!doctype|html)/i.test(text)) {
    const recipe = parseRecipeHtml(text, "")
    if (recipe) return [{ fields: { ...recipe, url: recipe.url || null } }]
    // Fall through: treat the page's visible text as a pasted recipe
    return fromText(
      text
        .replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/gi, "")
        .replace(/<[^>]+>/g, "\n"),
    )
  }
  if (["txt", "md", "markdown", "text", ""].includes(kind)) return fromText(text)
  return []
}
