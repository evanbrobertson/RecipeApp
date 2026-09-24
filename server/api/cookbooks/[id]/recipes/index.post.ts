import { z } from "zod"
import { addToCookbook } from "../../../../lib/recipes"

const schema = z.object({ recipeIds: z.array(z.number().int().positive()).min(1).max(1000) })

export default defineEventHandler(async (event) => {
  const id = idParam(event)
  const { recipeIds } = await readValidatedBody(event, schema.parse)
  return { added: addToCookbook(id, recipeIds) }
})
