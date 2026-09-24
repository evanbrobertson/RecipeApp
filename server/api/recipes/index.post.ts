import { recipeFieldsSchema } from "#shared/utils/recipe"
import { createRecipe } from "../../lib/recipes"

export default defineEventHandler(async (event) => {
  const fields = await readValidatedBody(event, recipeFieldsSchema.parse)
  const { recipe, isNew } = createRecipe(fields, "manual")
  setResponseStatus(event, isNew ? 201 : 200)
  return { ...recipe, isNew }
})
