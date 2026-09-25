/**
 * Protects every page and API route behind the app password.
 * The MCP endpoint does its own bearer-token check; OAuth endpoints must stay public.
 */
const PUBLIC_PREFIXES = [
  "/_nuxt/",
  "/__nuxt",
  "/oauth/",
  "/.well-known/",
  "/mcp",
  "/api/_nuxt_icon",
]
const PUBLIC_PATHS = new Set([
  "/login",
  "/api/auth/login",
  "/api/health",
  "/favicon.ico",
  "/manifest.webmanifest",
  "/icon.svg",
  "/robots.txt",
])

export default defineEventHandler(async (event) => {
  if (!authEnabled()) return
  const path = event.path.split("?")[0]!
  if (PUBLIC_PATHS.has(path) || PUBLIC_PREFIXES.some((p) => path.startsWith(p))) return
  if (await isLoggedIn(event)) return

  if (path.startsWith("/api/")) {
    throw createError({ statusCode: 401, statusMessage: "Not signed in" })
  }
  return sendRedirect(event, `/login?next=${encodeURIComponent(event.path)}`, 302)
})
