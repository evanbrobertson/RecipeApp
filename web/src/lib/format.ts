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

/**
 * Plain text for pasting into a message: the title, "•" ingredients under their section
 * names, numbered steps, and the share link when there is one. Not Markdown.
 */
export function recipeToText(r: FormattableRecipe, link?: string | null): string {
  const out: string[] = [r.title]
  const ingredients = r.ingredients.filter((s) => s.items.length)
  if (ingredients.length) {
    out.push("", "Ingredients")
    for (const s of ingredients) {
      if (s.name) out.push("", s.name)
      for (const item of s.items) out.push(`• ${item}`)
    }
  }
  const steps = r.instructions.filter((s) => s.items.length)
  if (steps.length) {
    out.push("", "Method")
    let n = 0
    for (const s of steps) {
      if (s.name) out.push("", s.name)
      for (const item of s.items) out.push(`${++n}. ${item}`)
    }
  }
  if (link) out.push("", link)
  return out.join("\n")
}

/** A cookbook as plain text: its name, its recipes' titles and the link. */
export function bookToText(name: string, titles: string[], link?: string | null): string {
  const out: string[] = [name]
  if (titles.length) out.push("", ...titles.map((t) => `• ${t}`))
  if (link) out.push("", link)
  return out.join("\n")
}

const count = (n: number) => `${n} recipe${n === 1 ? "" : "s"}`

/** The toast after saving another Crumb's shared cookbook: "Added 12 recipes to Weeknight dinners". */
export function bookImportedTitle(b: { name: string; added: number; duplicates: number }): string {
  if (b.added) return `Added ${count(b.added)} to ${b.name}`
  if (b.duplicates) return `Already in your recipes, now in ${b.name}`
  return `Added ${b.name}, an empty cookbook`
}
