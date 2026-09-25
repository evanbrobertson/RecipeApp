/** Shapes returned by the Crumb API (see the Rust server in ../src). */

export interface RecipeSection {
  name: string | null
  items: string[]
}

export type RecipeSource = "url" | "text" | "claude" | "manual" | "import"

export interface RecipeFields {
  title: string
  description?: string | null
  url?: string | null
  image?: string | null
  author?: string | null
  prepTime?: string | null
  cookTime?: string | null
  totalTime?: string | null
  freezeTime?: string | null
  recipeYield?: string | null
  recipeCategory?: string | null
  recipeCuisine?: string | null
  ingredients: RecipeSection[]
  instructions: RecipeSection[]
  nutrition?: Record<string, string> | null
  notes?: string | null
}

export interface Recipe extends RecipeFields {
  id: number
  source: RecipeSource
  createdAt: string
  updatedAt: string
}

export interface RecipeSummary {
  id: number
  title: string
  image: string | null
  totalTime: string | null
  recipeYield: string | null
  recipeCategory: string | null
  recipeCuisine: string | null
  source: RecipeSource
  createdAt: string
}

export interface Cookbook {
  id: number
  name: string
  description: string | null
  color: string | null
  createdAt: string
  recipeCount: number
}

export interface CookbookDetail extends Omit<Cookbook, "recipeCount"> {
  recipes: RecipeSummary[]
}

export interface ConnectorInfo {
  mcpUrl: string
  authEnabled: boolean
  claudeParsing: boolean
  /** "Claude", "OpenAI" or "DeepSeek" when an AI API is configured. */
  aiProvider: string | null
  browserScraping: boolean
}

export interface CookStats {
  count: number
  lastCookedAt: string | null
}

export type ReasonKind =
  | "like"
  | "interest"
  | "rediscover"
  | "quick"
  | "project"
  | "new"
  | "season"
  | "kicker"
  | "ai"

export interface Suggestion {
  recipe: RecipeSummary
  reason: string
  reasonKind: ReasonKind
  /** The reason was written by the AI layer. */
  ai: boolean
}

export interface Suggestions {
  items: Suggestion[]
  ai: "ready" | "pending" | "off"
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

export function countItems(sections: RecipeSection[]): number {
  return sections.reduce((n, s) => n + s.items.length, 0)
}

export function kicker(r: { recipeCategory: string | null; recipeCuisine: string | null }) {
  return [r.recipeCategory, r.recipeCuisine].filter(Boolean).join(" · ")
}

export function hostOf(url: string | null | undefined): string | null {
  if (!url) return null
  try {
    return new URL(url).hostname.replace(/^www\./, "")
  } catch {
    return null
  }
}
