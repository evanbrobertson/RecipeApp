export default defineEventHandler(async (event) => {
  await logOut(event)
  return { ok: true }
})
