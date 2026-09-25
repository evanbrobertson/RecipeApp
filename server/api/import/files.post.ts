import { importFile, type ImportSummary } from "../../lib/recipes"

const MAX_BYTES = 50 * 1024 * 1024

// Multipart upload of export files (JTR PDFs, Paprika, JSON, HTML, text, zip)
export default defineEventHandler(async (event) => {
  const parts = (await readMultipartFormData(event)) ?? []
  const files = parts.filter((p) => p.filename && p.data?.length)
  if (files.length === 0) throw createError({ statusCode: 400, statusMessage: "No files uploaded" })

  const results: ImportSummary[] = []
  for (const file of files) {
    if (file.data.length > MAX_BYTES) {
      results.push({
        file: file.filename!,
        created: [],
        duplicates: 0,
        error: "File is over 50 MB",
      })
      continue
    }
    results.push(await importFile(file.filename!, new Uint8Array(file.data)))
  }
  return results
})
