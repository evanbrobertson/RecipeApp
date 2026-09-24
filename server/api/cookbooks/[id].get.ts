import { getCookbook } from "../../lib/recipes"

export default defineEventHandler((event) => getCookbook(idParam(event)))
