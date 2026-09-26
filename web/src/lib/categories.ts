/** Crumb's fixed recipe categories, in display order ("Other" last). Mirrors src/categories.rs. */
export const CATEGORIES = [
  "Breakfast",
  "Main",
  "Side",
  "Soup",
  "Salad",
  "Baking",
  "Dessert",
  "Snack",
  "Sauce",
  "Drink",
  "Other",
] as const

export type Category = (typeof CATEGORIES)[number]

export const isCategory = (c: string | null | undefined): c is Category =>
  (CATEGORIES as readonly string[]).includes(c ?? "")

/** The categories that have at least one of `recipes`, in display order. */
export function categoriesIn(recipes: { recipeCategory?: string | null }[]): Category[] {
  const have = new Set(recipes.map((r) => r.recipeCategory))
  return CATEGORIES.filter((c) => have.has(c))
}
