export default defineEventHandler(async (event) => {
  setHeader(event, "access-control-allow-origin", "*")
  const body = ((await readBody(event).catch(() => null)) ?? {}) as Record<string, string>
  if (body.token) revokeToken(body.token)
  return {}
})
