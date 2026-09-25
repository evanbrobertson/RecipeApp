import { recipeCookbookIds } from "../../../lib/recipes"

export default defineEventHandler((event) => recipeCookbookIds(idParam(event)))
