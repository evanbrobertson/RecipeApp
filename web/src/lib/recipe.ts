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
  /** Saved from another Crumb's share (`url` is that link): where the recipe came from. */
  originalUrl?: string | null
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

/** A recipe's share link (`/s/{token}`): anyone with it can read that recipe. */
export interface ShareLink {
  token: string
  url: string
  includeNotes: boolean
  createdAt: string
}

/** A live share link, as More lists them (`GET /api/shares`). */
export interface SharedLink {
  kind: "recipe" | "cookbook"
  /** The recipe's or cookbook's id (the share routes take it, never the token). */
  id: number
  title: string
  url: string
  includeNotes: boolean
  createdAt: string
  lastOpenedAt: string | null
}

/** What saving another Crumb's shared cookbook did (`POST /api/recipes/import`). */
export interface BookImported {
  id: number
  name: string
  added: number
  duplicates: number
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
  /** Wee Chef (the AI helper) is on: an AI key is configured. */
  weeChef: boolean
  /** Older name for `weeChef`. */
  claudeParsing: boolean
  /** The API behind Wee Chef, for diagnostics only: the UI always says "Wee Chef". */
  aiProvider: string | null
  /** Wee Chef reads recipe photos; without it the browser runs OCR itself. */
  vision: boolean
  browserScraping: boolean
  /** Wee Chef checks imported recipes (the server has a key for it). */
  weeChefChecks?: boolean
}

/** One line Wee Chef looked at twice: fixed on import, or left for the cook to review. */
export interface CheckFlag {
  id: number
  /** "recipe" for the one-off clean-up of stray symbols, codes and repeats, and the category. */
  field: "ingredients" | "instructions" | "recipe"
  /** The line as it was when checked (flags follow the text, not the position). */
  itemText: string
  kind:
    | "heading"
    | "junk"
    | "fragment"
    | "not_instruction"
    | "merged"
    | "step"
    | "ingredient"
    | "tidy"
    | "category"
  state: "fixed" | "review"
  detail: {
    p?: number
    fix?: "heading" | "removed" | "notes" | "joined" | "tidy" | "category"
    /** For "tidy": how many small things were cleaned up. */
    count?: number
    /** For "category": the category now (null: cleared) and the one it replaced. */
    category?: string | null
    was?: string | null
    heading?: string
    with?: string
  } | null
}

/** Wee Chef's check of an imported recipe. */
export interface RecipeChecks {
  /** `skipped`: restored from a backup, not checked yet; `tidied`: only tidied on import. */
  status: "pending" | "done" | "failed" | "skipped" | "tidied"
  /** The fixes can still be undone (the recipe wasn't edited since). */
  canUndo: boolean
  flags: CheckFlag[]
}

/** Progress of "Check all recipes" on the More page. */
export interface ChecksStatus {
  enabled: boolean
  eligible: number
  checked: number
  pending: number
  failed: number
  tidied: number
  /** Recipes with suggestions waiting. */
  toCheck: number
  /** How many "Check all" would queue now, */
  due: number
  /** ...of which restored from a backup, */
  restored: number
  /** ...and edited since their last check. */
  edited: number
  queued?: number
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
  /** The reason was written by Wee Chef. */
  ai: boolean
}

/** A dish Wee Chef thinks you'd like that isn't in the box yet. */
export interface RecipeIdea {
  title: string
  why: string
  /** A web search for the recipe, to find a page to add. */
  searchUrl: string
}

export interface Suggestions {
  items: Suggestion[]
  ai: "ready" | "pending" | "off"
  /** Some days, one idea from Wee Chef, shown after the items. */
  idea?: RecipeIdea | null
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

/** `url` when it's an http(s) address, else null: never link a `javascript:` or `data:` URL. */
export function webLink(url: string | null | undefined): string | null {
  if (!url) return null
  try {
    const u = new URL(url)
    return (u.protocol === "http:" || u.protocol === "https:") && u.hostname ? url : null
  } catch {
    return null
  }
}

/** An http(s) address's host without `www.`; null for anything else. */
export function hostOf(url: string | null | undefined): string | null {
  const link = webLink(url)
  return link ? new URL(link).hostname.replace(/^www\./, "") : null
}
