import { z } from "zod"
import { listRecipes } from "../../lib/recipes"

const query = z.object({
  q: z.string().max(200).optional(),
  limit: z.coerce.number().int().min(1).max(500).optional(),
})

export default defineEventHandler(async (event) => {
  const { q, limit } = await getValidatedQuery(event, query.parse)
  return listRecipes({ q, limit })
})
