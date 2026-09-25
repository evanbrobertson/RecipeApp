import { requireRecipe } from "../../lib/recipes"

export default defineEventHandler((event) => requireRecipe(idParam(event)))
