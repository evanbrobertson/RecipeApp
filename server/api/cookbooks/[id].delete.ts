import { deleteCookbook } from "../../lib/recipes"

export default defineEventHandler((event) => {
  deleteCookbook(idParam(event))
  return { ok: true }
})
