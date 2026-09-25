import { z } from "zod"
import { BOOK_COLORS } from "#shared/utils/recipe"
import { updateCookbook } from "../../lib/recipes"

const schema = z.object({
  name: z.string().trim().min(1).max(100).optional(),
  description: z.string().trim().max(500).nullish(),
  color: z.enum(BOOK_COLORS).optional(),
})

export default defineEventHandler(async (event) => {
  const id = idParam(event)
  const patch = await readValidatedBody(event, schema.parse)
  if (patch.description !== undefined) patch.description ||= null
  return updateCookbook(id, patch)
})
