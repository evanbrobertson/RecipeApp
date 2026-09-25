import { removeFromCookbook } from "../../../../lib/recipes"

export default defineEventHandler((event) => {
  removeFromCookbook(idParam(event), idParam(event, "recipeId"))
  return { ok: true }
})
