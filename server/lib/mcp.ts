import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js"
import { z } from "zod"
import { recipeFieldsSchema, recipePatchSchema, sectionSchema } from "#shared/utils/recipe"
import { recipeToMarkdown } from "#shared/utils/format"
import {
  addToCookbook,
  createCookbook,
  createRecipe,
  deleteRecipes,
  getRecipe,
  importFromText,
  importFromUrl,
  listCookbooks,
  listRecipes,
  updateRecipe,
  type FullRecipe,
} from "./recipes"

const INSTRUCTIONS = `This connector is the user's personal recipe box ("Just the Recipe").
- To save a recipe the user pasted or described, structure it yourself and call save_recipe. Keep every ingredient and step exactly as written.
- If the user shares only a link, call import_recipe_from_url.
- Use search_recipes to find recipes by name or ingredient, then get_recipe for the full text.
- When the user asks to tweak a saved recipe (scale it, substitute, fix steps), call update_recipe with only the changed fields.
- Always share the recipe link returned by the tools.`

const sections = z
  .array(sectionSchema)
  .describe(
    'Groups of items. Use one group with name null unless the recipe has sub-headings like "For the sauce".',
  )

const recipeInput = {
  title: z.string().min(1).describe("Recipe name"),
  description: z.string().nullish().describe("One or two sentence summary"),
  ingredients: sections.describe(
    "Ingredient groups; each item is one ingredient line with quantity",
  ),
  instructions: sections.describe("Step groups; each item is one step"),
  prepTime: z.string().nullish().describe('e.g. "15m"'),
  cookTime: z.string().nullish().describe('e.g. "1h 10m"'),
  totalTime: z.string().nullish(),
  recipeYield: z.string().nullish().describe('e.g. "4 servings"'),
  recipeCategory: z.string().nullish().describe('e.g. "Dessert"'),
  recipeCuisine: z.string().nullish().describe('e.g. "Italian"'),
  author: z.string().nullish(),
  notes: z.string().nullish().describe("Tips, substitutions, storage notes"),
  url: z.string().url().nullish().describe("Original source URL, if known"),
  image: z.string().url().nullish().describe("Image URL, if known"),
}

function text(value: string) {
  return { content: [{ type: "text" as const, text: value }] }
}

function toolError(err: unknown) {
  const message =
    (err as { statusMessage?: string })?.statusMessage ||
    (err as Error)?.message ||
    "Something went wrong"
  return { ...text(message), isError: true }
}

export function createMcpServer(origin: string) {
  const link = (id: number) => `${origin}/recipes/${id}`
  const saved = (r: FullRecipe, isNew: boolean) =>
    text(
      `${isNew ? "Saved" : "Already saved"}: "${r.title}" (id ${r.id})\n${link(r.id)}\n\n` +
        `${r.ingredients.reduce((n, s) => n + s.items.length, 0)} ingredients, ` +
        `${r.instructions.reduce((n, s) => n + s.items.length, 0)} steps.`,
    )

  const server = new McpServer(
    { name: "just-the-recipe", title: "Just the Recipe", version: "2.0.0" },
    { instructions: INSTRUCTIONS },
  )

  server.registerTool(
    "search_recipes",
    {
      title: "Search recipes",
      description:
        "Search saved recipes by title, ingredient, category or cuisine. Leave query empty to list the most recent recipes.",
      inputSchema: {
        query: z.string().optional().describe('Words to match, e.g. "chicken lemon"'),
        limit: z.number().int().min(1).max(100).default(20),
      },
      annotations: { readOnlyHint: true },
    },
    async ({ query, limit }) => {
      const rows = listRecipes({ q: query, limit })
      if (rows.length === 0)
        return text(query ? `No recipes match "${query}".` : "No recipes saved yet.")
      const lines = rows.map((r) => {
        const meta = [r.totalTime, r.recipeCategory, r.recipeCuisine].filter(Boolean).join(" · ")
        return `- [${r.id}] ${r.title}${meta ? ` (${meta})` : ""} — ${link(r.id)}`
      })
      return text(`${rows.length} recipe(s):\n${lines.join("\n")}`)
    },
  )

  server.registerTool(
    "get_recipe",
    {
      title: "Get recipe",
      description: "Get the full recipe (ingredients, steps, notes) by id.",
      inputSchema: { id: z.number().int().describe("Recipe id from search_recipes") },
      annotations: { readOnlyHint: true },
    },
    async ({ id }) => {
      const recipe = getRecipe(id)
      if (!recipe) return toolError({ statusMessage: `No recipe with id ${id}` })
      return text(`${recipeToMarkdown(recipe)}\n\nRecipe id: ${recipe.id} · ${link(recipe.id)}`)
    },
  )

  server.registerTool(
    "save_recipe",
    {
      title: "Save recipe",
      description:
        "Save a recipe that you have structured into ingredients and steps (for example from text the user pasted or dictated). Preserve quantities, temperatures and times exactly.",
      inputSchema: recipeInput,
    },
    async (input) => {
      try {
        const fields = recipeFieldsSchema.parse(input)
        const { recipe, isNew } = createRecipe(fields, "claude")
        return saved(recipe, isNew)
      } catch (err) {
        return toolError(err)
      }
    },
  )

  server.registerTool(
    "import_recipe_from_text",
    {
      title: "Import recipe from raw text",
      description:
        "Let the app parse raw recipe text itself (e.g. a long copy-pasted web page). Prefer save_recipe when you can structure the recipe yourself.",
      inputSchema: { text: z.string().min(20).describe("The raw recipe text") },
    },
    async ({ text: raw }) => {
      try {
        const { recipe, isNew } = await importFromText(raw, { useClaude: false })
        return saved(recipe, isNew)
      } catch (err) {
        return toolError(err)
      }
    },
  )

  server.registerTool(
    "import_recipe_from_url",
    {
      title: "Import recipe from URL",
      description:
        "Fetch a recipe web page and save just the recipe. Returns the existing recipe if that URL was already saved.",
      inputSchema: { url: z.string().url().describe("Recipe page URL") },
      annotations: { openWorldHint: true },
    },
    async ({ url }) => {
      try {
        const { recipe, isNew } = await importFromUrl(url)
        return saved(recipe, isNew)
      } catch (err) {
        return toolError(err)
      }
    },
  )

  server.registerTool(
    "update_recipe",
    {
      title: "Update recipe",
      description:
        "Change fields on a saved recipe. Only send the fields that change; ingredients/instructions replace the whole list, so send the complete updated list.",
      inputSchema: {
        id: z.number().int(),
        ...Object.fromEntries(Object.entries(recipeInput).map(([k, v]) => [k, v.optional()])),
      } as { id: z.ZodNumber } & {
        [K in keyof typeof recipeInput]: z.ZodOptional<(typeof recipeInput)[K]>
      },
      annotations: { idempotentHint: true },
    },
    async ({ id, ...patch }) => {
      try {
        const clean = recipePatchSchema.parse(
          Object.fromEntries(Object.entries(patch).filter(([, v]) => v !== undefined)),
        )
        const recipe = updateRecipe(id, clean)
        return text(`Updated "${recipe.title}" (id ${recipe.id})\n${link(recipe.id)}`)
      } catch (err) {
        return toolError(err)
      }
    },
  )

  server.registerTool(
    "delete_recipe",
    {
      title: "Delete recipe",
      description: "Permanently delete a saved recipe. Confirm with the user first.",
      inputSchema: { id: z.number().int() },
      annotations: { destructiveHint: true },
    },
    async ({ id }) => {
      const recipe = getRecipe(id)
      if (!recipe) return toolError({ statusMessage: `No recipe with id ${id}` })
      deleteRecipes([id])
      return text(`Deleted "${recipe.title}".`)
    },
  )

  server.registerTool(
    "list_cookbooks",
    {
      title: "List cookbooks",
      description: "List the user's cookbooks (collections of recipes).",
      inputSchema: {},
      annotations: { readOnlyHint: true },
    },
    async () => {
      const books = listCookbooks()
      if (books.length === 0) return text("No cookbooks yet.")
      return text(books.map((b) => `- [${b.id}] ${b.name} (${b.recipeCount} recipes)`).join("\n"))
    },
  )

  server.registerTool(
    "add_to_cookbook",
    {
      title: "Add recipes to cookbook",
      description:
        "Add recipes to a cookbook by id or name. A cookbook with a new name is created.",
      inputSchema: {
        cookbook: z.union([z.number().int(), z.string().min(1)]).describe("Cookbook id or name"),
        recipeIds: z.array(z.number().int()).min(1),
      },
    },
    async ({ cookbook, recipeIds }) => {
      try {
        const books = listCookbooks()
        const match =
          typeof cookbook === "number"
            ? books.find((b) => b.id === cookbook)
            : books.find((b) => b.name.toLowerCase() === cookbook.trim().toLowerCase())
        if (!match && typeof cookbook === "number") {
          return toolError({ statusMessage: `No cookbook with id ${cookbook}` })
        }
        const target = match ?? createCookbook(String(cookbook))
        const added = addToCookbook(target.id, recipeIds)
        return text(
          `Added ${added} recipe(s) to "${target.name}".\n${origin}/cookbooks/${target.id}`,
        )
      } catch (err) {
        return toolError(err)
      }
    },
  )

  return server
}
