import { z } from "zod"

const schema = z.object({ password: z.string().min(1) })

export default defineEventHandler(async (event) => {
  const { password } = await readValidatedBody(event, schema.parse)
  if (!authEnabled()) return { ok: true }
  if (!checkPassword(password)) {
    // Slow down guessing
    await new Promise((r) => setTimeout(r, 750))
    throw createError({ statusCode: 401, statusMessage: "Incorrect password" })
  }
  await logIn(event)
  return { ok: true }
})
