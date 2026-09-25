export default defineEventHandler(async (event) => {
  setHeader(event, "access-control-allow-origin", "*")
  setHeader(event, "cache-control", "no-store")
  const body = ((await readBody(event).catch(() => null)) ?? {}) as Record<string, string>

  try {
    if (body.grant_type === "authorization_code") {
      return exchangeAuthorizationCode({
        code: body.code,
        clientId: body.client_id,
        redirectUri: body.redirect_uri,
        codeVerifier: body.code_verifier,
      })
    }
    if (body.grant_type === "refresh_token") {
      return exchangeRefreshToken(body.refresh_token)
    }
    setResponseStatus(event, 400)
    return { error: "unsupported_grant_type" }
  } catch (err: any) {
    // OAuth clients expect { error, error_description } rather than h3's error shape
    setResponseStatus(event, err?.statusCode ?? 400)
    return err?.data ?? { error: "invalid_request" }
  }
})
