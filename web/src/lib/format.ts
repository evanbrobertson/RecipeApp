import type { RecipeSection } from "./recipe"

interface FormattableRecipe {
  title: string
  description?: string | null
  url?: string | null
  author?: string | null
  prepTime?: string | null
  cookTime?: string | null
  freezeTime?: string | null
  totalTime?: string | null
  recipeYield?: string | null
  recipeCategory?: string | null
  recipeCuisine?: string | null
  ingredients: RecipeSection[]
  instructions: RecipeSection[]
  notes?: string | null
}

/** Plain-text/Markdown rendering used for "copy recipe" and the Claude connector. */
export function recipeToMarkdown(r: FormattableRecipe): string {
  const meta = [
    r.prepTime && `Prep: ${r.prepTime}`,
    r.cookTime && `Cook: ${r.cookTime}`,
    r.freezeTime && `Additional: ${r.freezeTime}`,
    r.totalTime && `Total: ${r.totalTime}`,
    r.recipeYield && `Yield: ${r.recipeYield}`,
    r.recipeCategory && `Category: ${r.recipeCategory}`,
    r.recipeCuisine && `Cuisine: ${r.recipeCuisine}`,
    r.author && `By: ${r.author}`,
  ].filter(Boolean)

  const out: string[] = [`# ${r.title}`]
  if (r.description) out.push("", r.description)
  if (meta.length) out.push("", meta.join(" · "))

  out.push("", "## Ingredients")
  for (const s of r.ingredients) {
    if (s.name) out.push("", `### ${s.name}`)
    for (const item of s.items) out.push(`- ${item}`)
  }

  out.push("", "## Instructions")
  let step = 0
  for (const s of r.instructions) {
    if (s.name) out.push("", `### ${s.name}`)
    for (const item of s.items) out.push(`${++step}. ${item}`)
  }

  if (r.notes) out.push("", "## Notes", r.notes)
  if (r.url) out.push("", `Source: ${r.url}`)
  return out.join("\n")
}
