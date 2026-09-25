import { recipePatchSchema } from "#shared/utils/recipe"
import { updateRecipe } from "../../lib/recipes"

export default defineEventHandler(async (event) => {
  const id = idParam(event)
  const patch = await readValidatedBody(event, recipePatchSchema.parse)
  return updateRecipe(id, patch)
})
