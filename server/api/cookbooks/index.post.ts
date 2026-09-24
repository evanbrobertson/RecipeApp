import { z } from "zod"
import { createCookbook } from "../../lib/recipes"

const schema = z.object({
  name: z.string().trim().min(1, "Name is required").max(100),
  description: z.string().trim().max(500).nullish(),
})

export default defineEventHandler(async (event) => {
  const { name, description } = await readValidatedBody(event, schema.parse)
  setResponseStatus(event, 201)
  return createCookbook(name, description)
})
