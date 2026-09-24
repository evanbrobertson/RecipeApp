import { z } from "zod"
import { importFromText, importFromUrl } from "../../lib/recipes"

const schema = z.union([
  z.object({ url: z.url("Please enter a valid URL") }),
  z.object({ text: z.string().trim().min(10, "Paste a bit more of the recipe").max(100_000) }),
])

export default defineEventHandler(async (event) => {
  const body = await readValidatedBody(event, schema.parse)
  const { recipe, isNew } =
    "url" in body ? await importFromUrl(body.url) : await importFromText(body.text)
  return { id: recipe.id, title: recipe.title, isNew }
})
