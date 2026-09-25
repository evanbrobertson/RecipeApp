import { sql } from "drizzle-orm"
import { useDB } from "../database"

export default defineEventHandler(() => {
  useDB().run(sql`select 1`)
  return { ok: true }
})
