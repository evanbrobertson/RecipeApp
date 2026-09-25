import { z } from "zod"
import { BOOK_COLORS } from "#shared/utils/recipe"
import { createCookbook } from "../../lib/recipes"

const schema = z.object({
  name: z.string().trim().min(1, "Name is required").max(100),
  description: z.string().trim().max(500).nullish(),
  color: z.enum(BOOK_COLORS).nullish(),
})

export default defineEventHandler(async (event) => {
  const { name, description, color } = await readValidatedBody(event, schema.parse)
  setResponseStatus(event, 201)
  return createCookbook(name, description, color)
})
