import type { H3Event } from "h3"

/**
 * OAuth consent screen. Claude sends the user here when the connector is added;
 * the user confirms with the app password (or an existing session) and is sent back.
 */

interface AuthorizeParams {
  client_id: string
  redirect_uri: string
  state: string
  code_challenge: string
  code_challenge_method: string
  response_type: string
}

const escapeHtml = (s: string) =>
  s.replace(
    /[&<>"']/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!,
  )

function page(title: string, body: string) {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${escapeHtml(title)} · Just the Recipe</title>
<style>
  :root { color-scheme: light dark; --bg:#fafaf9; --card:#fff; --fg:#1c1917; --muted:#78716c; --line:#e7e5e4; --accent:#16a34a; }
  @media (prefers-color-scheme: dark) { :root { --bg:#0c0a09; --card:#1c1917; --fg:#fafaf9; --muted:#a8a29e; --line:#292524; } }
  * { box-sizing: border-box } body { margin:0; min-height:100vh; display:grid; place-items:center; padding:16px;
    background:var(--bg); color:var(--fg); font:16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif }
  main { width:100%; max-width:400px; background:var(--card); border:1px solid var(--line); border-radius:16px; padding:28px }
  h1 { font-size:1.25rem; margin:0 0 8px } p { color:var(--muted); margin:0 0 20px }
  label { display:block; font-size:.875rem; font-weight:600; margin-bottom:6px }
  input[type=password] { width:100%; padding:10px 12px; border-radius:10px; border:1px solid var(--line); background:transparent; color:inherit; font:inherit; margin-bottom:16px }
  .row { display:flex; gap:10px } button { flex:1; padding:10px 14px; border-radius:10px; border:1px solid var(--line); font:inherit; font-weight:600; cursor:pointer; background:transparent; color:inherit }
  button.primary { background:var(--accent); border-color:var(--accent); color:#fff }
  .error { color:#dc2626; font-size:.875rem; margin:-8px 0 12px } code { font-size:.85em }
</style></head><body><main>${body}</main></body></html>`
}

function readParams(source: Record<string, unknown>): AuthorizeParams {
  const get = (k: string) => (typeof source[k] === "string" ? (source[k] as string) : "")
  return {
    client_id: get("client_id"),
    redirect_uri: get("redirect_uri"),
    state: get("state"),
    code_challenge: get("code_challenge"),
    code_challenge_method: get("code_challenge_method") || "S256",
    response_type: get("response_type") || "code",
  }
}

function fail(event: H3Event, message: string) {
  setResponseStatus(event, 400)
  setHeader(event, "content-type", "text/html; charset=utf-8")
  return page("Can't connect", `<h1>Can't connect</h1><p>${escapeHtml(message)}</p>`)
}

function redirectBack(event: H3Event, redirectUri: string, params: Record<string, string>) {
  const url = new URL(redirectUri)
  for (const [k, v] of Object.entries(params)) if (v) url.searchParams.set(k, v)
  return sendRedirect(event, url.toString(), 302)
}

export default defineEventHandler(async (event) => {
  const method = event.method
  if (method !== "GET" && method !== "POST") {
    throw createError({ statusCode: 405, statusMessage: "Method not allowed" })
  }

  const form = method === "POST" ? ((await readBody(event)) as Record<string, unknown>) : {}
  const params = readParams(method === "POST" ? form : getQuery(event))

  const client = params.client_id ? getClient(params.client_id) : undefined
  if (!client)
    return fail(event, "Unknown client. Remove the connector in Claude and add it again.")
  if (!client.redirectUris.includes(params.redirect_uri)) {
    return fail(event, "The redirect URI doesn't match this client's registration.")
  }
  if (params.response_type !== "code") {
    return redirectBack(event, params.redirect_uri, {
      error: "unsupported_response_type",
      state: params.state,
    })
  }
  if (!params.code_challenge || params.code_challenge_method !== "S256") {
    return redirectBack(event, params.redirect_uri, {
      error: "invalid_request",
      error_description: "PKCE with S256 is required",
      state: params.state,
    })
  }

  let error = ""
  if (method === "POST") {
    if (form.action === "deny") {
      return redirectBack(event, params.redirect_uri, {
        error: "access_denied",
        state: params.state,
      })
    }
    const loggedIn = await isLoggedIn(event)
    const password = typeof form.password === "string" ? form.password : ""
    if (loggedIn || checkPassword(password)) {
      if (!loggedIn) await logIn(event)
      const code = createAuthorizationCode(client.id, params.code_challenge, params.redirect_uri)
      return redirectBack(event, params.redirect_uri, { code, state: params.state })
    }
    error = "Incorrect password."
  }

  const loggedIn = await isLoggedIn(event)
  const clientName = client.name || "An application"
  const host = new URL(params.redirect_uri).host
  const hidden = Object.entries(params)
    .map(([k, v]) => `<input type="hidden" name="${k}" value="${escapeHtml(v)}">`)
    .join("")

  setHeader(event, "content-type", "text/html; charset=utf-8")
  setHeader(event, "cache-control", "no-store")
  setHeader(event, "x-frame-options", "DENY")
  return page(
    "Connect",
    `<h1>Connect ${escapeHtml(clientName)}?</h1>
    <p><strong>${escapeHtml(clientName)}</strong> (<code>${escapeHtml(host)}</code>) wants to read, add and edit recipes in Just the Recipe.</p>
    <form method="post" action="/oauth/authorize">
      ${hidden}
      ${
        loggedIn
          ? ""
          : `<label for="password">App password</label>
             <input id="password" name="password" type="password" autocomplete="current-password" required autofocus>`
      }
      ${error ? `<div class="error">${escapeHtml(error)}</div>` : ""}
      <div class="row">
        <button type="submit" name="action" value="deny" formnovalidate>Cancel</button>
        <button type="submit" name="action" value="allow" class="primary">Allow</button>
      </div>
    </form>`,
  )
})
