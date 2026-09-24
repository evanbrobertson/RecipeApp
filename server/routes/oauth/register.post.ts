import { z } from "zod"

const schema = z.object({
  client_name: z.string().max(200).optional(),
  redirect_uris: z.array(z.string()).min(1).max(10),
})

// RFC 7591 dynamic client registration, used by Claude when the connector is added
export default defineEventHandler(async (event) => {
  setHeader(event, "access-control-allow-origin", "*")
  const parsed = schema.safeParse(await readBody(event).catch(() => null))
  if (!parsed.success || !parsed.data.redirect_uris.every(isAllowedRedirectUri)) {
    setResponseStatus(event, 400)
    return {
      error: "invalid_redirect_uri",
      error_description: "redirect_uris must be https (or http://localhost) URLs",
    }
  }
  const { client_name, redirect_uris } = parsed.data
  const clientId = registerClient(client_name ?? null, redirect_uris)
  setResponseStatus(event, 201)
  return {
    client_id: clientId,
    client_id_issued_at: Math.floor(Date.now() / 1000),
    client_name,
    redirect_uris,
    grant_types: ["authorization_code", "refresh_token"],
    response_types: ["code"],
    token_endpoint_auth_method: "none",
  }
})
