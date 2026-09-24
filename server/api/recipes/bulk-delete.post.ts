import { z } from "zod"
import { deleteRecipes } from "../../lib/recipes"

const schema = z.object({ ids: z.array(z.number().int().positive()).min(1).max(1000) })

export default defineEventHandler(async (event) => {
  const { ids } = await readValidatedBody(event, schema.parse)
  return { deleted: deleteRecipes(ids) }
})
