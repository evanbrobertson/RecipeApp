import { z } from "zod"
import { updateCookbook } from "../../lib/recipes"

const schema = z.object({
  name: z.string().trim().min(1).max(100).optional(),
  description: z.string().trim().max(500).nullish(),
})

export default defineEventHandler(async (event) => {
  const id = idParam(event)
  const patch = await readValidatedBody(event, schema.parse)
  return updateCookbook(id, { ...patch, description: patch.description || null })
})
