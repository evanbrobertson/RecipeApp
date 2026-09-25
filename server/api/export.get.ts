import { exportBackup } from "../lib/recipes"

export default defineEventHandler((event) => {
  const date = new Date().toISOString().slice(0, 10)
  setHeader(event, "content-type", "application/json; charset=utf-8")
  setHeader(event, "content-disposition", `attachment; filename="recipes-${date}.json"`)
  return JSON.stringify(exportBackup(), null, 2)
})
