import Anthropic from "@anthropic-ai/sdk"
import { betaZodOutputFormat } from "@anthropic-ai/sdk/helpers/beta/zod"
import { z } from "zod"
import type { RecipeFields } from "#shared/utils/recipe"
import { normalizeSections } from "#shared/utils/recipe"

const section = z.object({
  name: z.string().nullable().describe('Sub-heading such as "For the sauce", or null'),
  items: z.array(z.string()),
})

const extractedRecipe = z.object({
  isRecipe: z.boolean().describe("False if the text does not contain a recipe"),
  title: z.string(),
  description: z.string().nullable(),
  author: z.string().nullable(),
  prepTime: z.string().nullable().describe('Human readable, e.g. "15m" or "1h 10m"'),
  cookTime: z.string().nullable(),
  totalTime: z.string().nullable(),
  recipeYield: z.string().nullable().describe('e.g. "4 servings"'),
  recipeCategory: z.string().nullable().describe('e.g. "Dessert", "Main course"'),
  recipeCuisine: z.string().nullable(),
  ingredients: z.array(section),
  instructions: z.array(section),
  notes: z.string().nullable(),
})

const SYSTEM = `You extract recipes from messy pasted text (web pages, emails, notes, OCR).
Return the recipe exactly as written: keep every ingredient and every step, with quantities, temperatures and times unchanged.
Drop ads, life stories, comments, navigation and other non-recipe content.
Group ingredients and steps into named sections only when the source does; otherwise use a single section with name null.
Each instruction item is one step. Do not invent ingredients, steps or metadata that are not in the text.`

let client: Anthropic | null = null

export function claudeAvailable(): boolean {
  return Boolean(useRuntimeConfig().anthropicApiKey)
}

/**
 * Uses Claude to turn pasted text into structured recipe fields.
 * Returns null when Claude is not configured, fails, or finds no recipe,
 * so callers can fall back to the built-in parser.
 */
export async function extractRecipeWithClaude(text: string): Promise<RecipeFields | null> {
  const { anthropicApiKey, anthropicModel } = useRuntimeConfig()
  if (!anthropicApiKey) return null
  client ??= new Anthropic({ apiKey: anthropicApiKey, timeout: 60_000, maxRetries: 1 })

  try {
    const response = await client.beta.messages.parse({
      model: anthropicModel,
      max_tokens: 16000,
      betas: ["server-side-fallback-2026-07-01"],
      fallbacks: "default",
      output_config: { effort: "low", format: betaZodOutputFormat(extractedRecipe) },
      system: SYSTEM,
      messages: [{ role: "user", content: text }],
    })

    if (response.stop_reason === "refusal" || response.stop_reason === "max_tokens") return null
    const out = response.parsed_output
    if (!out?.isRecipe) return null

    return {
      title: out.title.trim() || "Untitled recipe",
      description: out.description,
      author: out.author,
      prepTime: out.prepTime,
      cookTime: out.cookTime,
      totalTime: out.totalTime,
      recipeYield: out.recipeYield,
      recipeCategory: out.recipeCategory,
      recipeCuisine: out.recipeCuisine,
      ingredients: normalizeSections(out.ingredients),
      instructions: normalizeSections(out.instructions),
      notes: out.notes,
    }
  } catch (error) {
    if (error instanceof Anthropic.APIError) {
      console.warn(`[claude-extract] API error ${error.status}: ${error.message}`)
    } else {
      console.warn("[claude-extract] failed:", error)
    }
    return null
  }
}
