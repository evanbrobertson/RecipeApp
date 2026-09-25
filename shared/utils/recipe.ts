import { z } from "zod"

export interface RecipeSection {
  name: string | null
  items: string[]
}

export type RecipeSource = "url" | "text" | "claude" | "manual" | "import"

export const sectionSchema = z.object({
  name: z.string().nullable().default(null),
  items: z.array(z.string()),
})

const optionalText = z.string().trim().nullish()

const recipeShape = {
  title: z.string().trim().min(1).max(300),
  description: optionalText,
  url: z.url().nullish(),
  image: z.url().nullish(),
  author: optionalText,
  prepTime: optionalText,
  cookTime: optionalText,
  totalTime: optionalText,
  freezeTime: optionalText,
  recipeYield: optionalText,
  recipeCategory: optionalText,
  recipeCuisine: optionalText,
  ingredients: z.array(sectionSchema),
  instructions: z.array(sectionSchema),
  nutrition: z.record(z.string(), z.string()).nullish(),
  notes: optionalText,
}

/** Fields a client (web UI or Claude) may set when creating a recipe. */
export const recipeFieldsSchema = z.object({
  ...recipeShape,
  ingredients: recipeShape.ingredients.default([]),
  instructions: recipeShape.instructions.default([]),
})

export type RecipeFields = z.infer<typeof recipeFieldsSchema>

/** Partial update. Built from the raw shape so no defaults sneak in and wipe fields. */
export const recipePatchSchema = z.object(recipeShape).partial()

/**
 * Normalizes raw JSON from the database (a legacy flat string[] or a RecipeSection[])
 * into RecipeSection[], dropping blank items and empty sections.
 */
export function normalizeSections(data: unknown): RecipeSection[] {
  if (!Array.isArray(data) || data.length === 0) return []
  if (data.every((d) => typeof d === "string")) {
    const items = (data as string[]).map((s) => s.trim()).filter(Boolean)
    return items.length ? [{ name: null, items }] : []
  }
  return (data as RecipeSection[])
    .map((s) => ({
      name: s?.name?.trim() || null,
      items: (Array.isArray(s?.items) ? s.items : [])
        .filter((i): i is string => typeof i === "string")
        .map((i) => i.trim())
        .filter(Boolean),
    }))
    .filter((s) => s.items.length > 0)
}

export function countItems(sections: RecipeSection[]): number {
  return sections.reduce((n, s) => n + s.items.length, 0)
}

export const nutritionLabels: Record<string, string> = {
  calories: "Calories",
  fatContent: "Fat",
  saturatedFatContent: "Saturated fat",
  unsaturatedFatContent: "Unsaturated fat",
  transFatContent: "Trans fat",
  carbohydrateContent: "Carbs",
  sugarContent: "Sugar",
  fiberContent: "Fiber",
  proteinContent: "Protein",
  cholesterolContent: "Cholesterol",
  sodiumContent: "Sodium",
  servingSize: "Serving size",
}

/** Cloth colours for cookbooks on the shelf. */
export const BOOK_COLORS = [
  "tomato",
  "sage",
  "mustard",
  "plum",
  "ocean",
  "terracotta",
  "forest",
  "navy",
  "rose",
  "charcoal",
] as const

export type BookColor = (typeof BOOK_COLORS)[number]
