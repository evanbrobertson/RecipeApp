import { and, count, desc, eq, inArray, or, sql, type SQLWrapper } from "drizzle-orm"
import { useDB } from "../database"
import { cookbookRecipes, cookbooks, recipes, type Recipe } from "../database/schema"
import {
  BOOK_COLORS,
  normalizeSections,
  type RecipeFields,
  type RecipeSource,
  recipeFieldsSchema,
} from "#shared/utils/recipe"
import { scrapeRecipe } from "./scraper"
import { parseRecipeText } from "./text-parser"
import { extractRecipeWithClaude } from "./claude-extract"
import { recipesFromFile, type BackupFile, type ImportedRecipe } from "./importers"

/** Service layer shared by the REST API and the MCP connector. */

export type RecipeSummary = Pick<
  Recipe,
  | "id"
  | "title"
  | "image"
  | "totalTime"
  | "recipeYield"
  | "recipeCategory"
  | "recipeCuisine"
  | "source"
  | "createdAt"
>

const summaryColumns = {
  id: recipes.id,
  title: recipes.title,
  image: recipes.image,
  totalTime: recipes.totalTime,
  recipeYield: recipes.recipeYield,
  recipeCategory: recipes.recipeCategory,
  recipeCuisine: recipes.recipeCuisine,
  source: recipes.source,
  createdAt: recipes.createdAt,
}

function contains(column: SQLWrapper, term: string) {
  const pattern = `%${term.replace(/[\\%_]/g, (c) => `\\${c}`)}%`
  return sql`${column} LIKE ${pattern} ESCAPE '\\'`
}

export function listRecipes(opts: { q?: string; limit?: number; offset?: number } = {}) {
  const db = useDB()
  const terms = (opts.q ?? "").trim().split(/\s+/).filter(Boolean)

  // Every term must match the title, ingredients, category or cuisine
  const where = terms.length
    ? and(
        ...terms.map((term) =>
          or(
            contains(recipes.title, term),
            contains(recipes.ingredients, term),
            contains(recipes.recipeCategory, term),
            contains(recipes.recipeCuisine, term),
          ),
        ),
      )
    : undefined

  return db
    .select(summaryColumns)
    .from(recipes)
    .where(where)
    .orderBy(desc(recipes.createdAt), desc(recipes.id))
    .limit(opts.limit ?? 500)
    .offset(opts.offset ?? 0)
    .all()
}

export function getRecipe(id: number) {
  const recipe = useDB().select().from(recipes).where(eq(recipes.id, id)).get()
  if (!recipe) return null
  return {
    ...recipe,
    ingredients: normalizeSections(recipe.ingredients),
    instructions: normalizeSections(recipe.instructions),
  }
}

export type FullRecipe = NonNullable<ReturnType<typeof getRecipe>>

export function requireRecipe(id: number): FullRecipe {
  const recipe = getRecipe(id)
  if (!recipe) throw createError({ statusCode: 404, statusMessage: "Recipe not found" })
  return recipe
}

function findByUrl(url: string) {
  return useDB().select({ id: recipes.id }).from(recipes).where(eq(recipes.url, url)).get()
}

export function createRecipe(input: RecipeFields, source: RecipeSource) {
  const fields = recipeFieldsSchema.parse(input)
  if (fields.url) {
    const existing = findByUrl(fields.url)
    if (existing) return { recipe: requireRecipe(existing.id), isNew: false }
  }
  const inserted = useDB()
    .insert(recipes)
    .values({
      ...fields,
      source,
      ingredients: normalizeSections(fields.ingredients),
      instructions: normalizeSections(fields.instructions),
    })
    .returning({ id: recipes.id })
    .get()
  return { recipe: requireRecipe(inserted.id), isNew: true }
}

export function updateRecipe(id: number, patch: Partial<RecipeFields>) {
  const values: Partial<typeof recipes.$inferInsert> = { ...patch, updatedAt: new Date() }
  if (patch.ingredients) values.ingredients = normalizeSections(patch.ingredients)
  if (patch.instructions) values.instructions = normalizeSections(patch.instructions)
  if (patch.url) {
    const existing = findByUrl(patch.url)
    if (existing && existing.id !== id) {
      throw createError({ statusCode: 409, statusMessage: "Another recipe already uses that URL" })
    }
  }
  const updated = useDB()
    .update(recipes)
    .set(values)
    .where(eq(recipes.id, id))
    .returning({ id: recipes.id })
    .get()
  if (!updated) throw createError({ statusCode: 404, statusMessage: "Recipe not found" })
  return requireRecipe(id)
}

export function deleteRecipes(ids: number[]) {
  if (ids.length === 0) return 0
  return useDB().delete(recipes).where(inArray(recipes.id, ids)).returning({ id: recipes.id }).all()
    .length
}

/**
 * Share links from Just the Recipe (and similar readers) wrap the original page URL,
 * e.g. https://www.justtherecipe.com/?url=https://site.com/recipe. Import the original.
 */
export function unwrapShareLink(input: string): string {
  try {
    const url = new URL(input)
    for (const key of ["url", "u", "link", "recipe"]) {
      const inner = url.searchParams.get(key)
      if (inner && /^https?:\/\//i.test(inner)) return inner
    }
    const path = decodeURIComponent(url.pathname.slice(1))
    if (/justtherecipe/i.test(url.hostname) && /^https?:\/\//i.test(path)) return path
  } catch {
    // not a URL; let the caller's validation handle it
  }
  return input
}

export async function importFromUrl(rawUrl: string) {
  const url = unwrapShareLink(rawUrl)
  const existing = findByUrl(url)
  if (existing) return { recipe: requireRecipe(existing.id), isNew: false }
  return createRecipe(await scrapeRecipe(url), "url")
}

const URL_ONLY = /^\s*(https?:\/\/\S+)\s*$/i

/**
 * Imports a recipe from pasted text. A lone URL is scraped; otherwise Claude parses
 * the text when configured, with the built-in heuristic parser as the fallback.
 */
export async function importFromText(text: string, opts: { useClaude?: boolean } = {}) {
  const urlOnly = text.match(URL_ONLY)
  if (urlOnly) return importFromUrl(urlOnly[1]!)

  let fields: RecipeFields | null = null
  if (opts.useClaude !== false) fields = await extractRecipeWithClaude(text)
  if (!fields) {
    const parsed = parseRecipeText(text)
    fields = parsed.recipe
  }

  if (fields.ingredients.length === 0 && fields.instructions.length === 0) {
    throw createError({
      statusCode: 422,
      statusMessage:
        "Couldn't find ingredients or steps in that text. Add “Ingredients” and “Instructions” headings and try again.",
    })
  }

  // Keep a source link if one was pasted alongside the text, unless it's already saved
  if (fields.url && findByUrl(fields.url)) fields.url = null
  return createRecipe(fields, "text")
}

// ─── Files & backups ─────────────────────────────────────────────────────────

export interface ImportSummary {
  file: string
  created: { id: number; title: string }[]
  duplicates: number
  error?: string
}

function findOrCreateCookbook(name: string) {
  const trimmed = name.trim()
  const existing = useDB()
    .select({ id: cookbooks.id })
    .from(cookbooks)
    .where(sql`lower(${cookbooks.name}) = lower(${trimmed})`)
    .get()
  return existing?.id ?? createCookbook(trimmed).id
}

export async function importFile(name: string, bytes: Uint8Array): Promise<ImportSummary> {
  const summary: ImportSummary = { file: name, created: [], duplicates: 0 }
  let found: ImportedRecipe[]
  try {
    found = await recipesFromFile(name, bytes)
  } catch (err) {
    return { ...summary, error: `Couldn't read this file (${(err as Error).message})` }
  }
  if (found.length === 0) return { ...summary, error: "No recipes found in this file" }

  for (const { fields, cookbooks: books } of found) {
    try {
      const { recipe, isNew } = createRecipe(fields, "import")
      if (!isNew) summary.duplicates++
      else summary.created.push({ id: recipe.id, title: recipe.title })
      for (const book of books ?? []) {
        if (book.trim()) addToCookbook(findOrCreateCookbook(book), [recipe.id])
      }
    } catch (err) {
      console.warn(`[import] skipped a recipe in ${name}:`, (err as Error).message)
    }
  }
  return summary
}

/** Everything in one JSON file that importFile() can read back. */
export function exportBackup(): BackupFile {
  const db = useDB()
  const all = db.select().from(recipes).orderBy(recipes.id).all()
  const books = db.select().from(cookbooks).orderBy(cookbooks.name).all()
  const links = db.select().from(cookbookRecipes).all()
  const bookName = new Map(books.map((b) => [b.id, b.name]))
  return {
    format: "just-the-recipe",
    version: 1,
    cookbooks: books.map((b) => ({ name: b.name, description: b.description })),
    recipes: all.map((r) => ({
      title: r.title,
      description: r.description,
      url: r.url,
      image: r.image,
      author: r.author,
      prepTime: r.prepTime,
      cookTime: r.cookTime,
      totalTime: r.totalTime,
      freezeTime: r.freezeTime,
      recipeYield: r.recipeYield,
      recipeCategory: r.recipeCategory,
      recipeCuisine: r.recipeCuisine,
      ingredients: normalizeSections(r.ingredients),
      instructions: normalizeSections(r.instructions),
      nutrition: r.nutrition,
      notes: r.notes,
      cookbooks: links.filter((l) => l.recipeId === r.id).map((l) => bookName.get(l.cookbookId)!),
    })),
  }
}

// ─── Cookbooks ──────────────────────────────────────────────────────────────

export function listCookbooks() {
  return useDB()
    .select({
      id: cookbooks.id,
      name: cookbooks.name,
      description: cookbooks.description,
      color: cookbooks.color,
      createdAt: cookbooks.createdAt,
      recipeCount: count(cookbookRecipes.id),
    })
    .from(cookbooks)
    .leftJoin(cookbookRecipes, eq(cookbooks.id, cookbookRecipes.cookbookId))
    .groupBy(cookbooks.id)
    .orderBy(cookbooks.name)
    .all()
}

export function getCookbook(id: number) {
  const db = useDB()
  const cookbook = db.select().from(cookbooks).where(eq(cookbooks.id, id)).get()
  if (!cookbook) throw createError({ statusCode: 404, statusMessage: "Cookbook not found" })
  const items = db
    .select(summaryColumns)
    .from(cookbookRecipes)
    .innerJoin(recipes, eq(cookbookRecipes.recipeId, recipes.id))
    .where(eq(cookbookRecipes.cookbookId, id))
    .orderBy(recipes.title)
    .all()
  return { ...cookbook, recipes: items }
}

export function createCookbook(name: string, description?: string | null, color?: string | null) {
  const pick = BOOK_COLORS[Math.floor(Math.random() * BOOK_COLORS.length)]
  return useDB()
    .insert(cookbooks)
    .values({ name: name.trim(), description: description?.trim() || null, color: color ?? pick })
    .returning()
    .get()
}

export function updateCookbook(
  id: number,
  patch: { name?: string; description?: string | null; color?: string | null },
) {
  const updated = useDB().update(cookbooks).set(patch).where(eq(cookbooks.id, id)).returning().get()
  if (!updated) throw createError({ statusCode: 404, statusMessage: "Cookbook not found" })
  return updated
}

export function deleteCookbook(id: number) {
  const deleted = useDB().delete(cookbooks).where(eq(cookbooks.id, id)).returning().get()
  if (!deleted) throw createError({ statusCode: 404, statusMessage: "Cookbook not found" })
}

export function addToCookbook(cookbookId: number, recipeIds: number[]) {
  const db = useDB()
  const cookbook = db
    .select({ id: cookbooks.id })
    .from(cookbooks)
    .where(eq(cookbooks.id, cookbookId))
    .get()
  if (!cookbook) throw createError({ statusCode: 404, statusMessage: "Cookbook not found" })
  const valid = db
    .select({ id: recipes.id })
    .from(recipes)
    .where(inArray(recipes.id, recipeIds))
    .all()
  if (valid.length === 0) return 0
  return db
    .insert(cookbookRecipes)
    .values(valid.map((r) => ({ cookbookId, recipeId: r.id })))
    .onConflictDoNothing()
    .returning()
    .all().length
}

export function removeFromCookbook(cookbookId: number, recipeId: number) {
  useDB()
    .delete(cookbookRecipes)
    .where(and(eq(cookbookRecipes.cookbookId, cookbookId), eq(cookbookRecipes.recipeId, recipeId)))
    .run()
}

export function recipeCookbookIds(recipeId: number) {
  return useDB()
    .select({ id: cookbookRecipes.cookbookId })
    .from(cookbookRecipes)
    .where(eq(cookbookRecipes.recipeId, recipeId))
    .all()
    .map((r) => r.id)
}
