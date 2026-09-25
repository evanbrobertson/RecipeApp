import { deleteRecipes } from "../../lib/recipes"

export default defineEventHandler((event) => {
  if (!deleteRecipes([idParam(event)])) {
    throw createError({ statusCode: 404, statusMessage: "Recipe not found" })
  }
  return { ok: true }
})
