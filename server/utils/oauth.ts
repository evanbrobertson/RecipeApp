import { createHash } from "node:crypto"
import type { H3Event } from "h3"
import { and, eq, gt, lt } from "drizzle-orm"
import { useDB } from "../database"
import { oauthClients, oauthTokens } from "../database/schema"

/**
 * Minimal OAuth 2.1 authorization server (dynamic client registration + PKCE)
 * so the app can be added to Claude as a custom connector.
 */

const MINUTE = 60_000
const DAY = 24 * 60 * MINUTE
const LIFETIME = { code: 5 * MINUTE, access: 30 * DAY, refresh: 365 * DAY } as const

export function authServerMetadata(origin: string) {
  return {
    issuer: origin,
    authorization_endpoint: `${origin}/oauth/authorize`,
    token_endpoint: `${origin}/oauth/token`,
    registration_endpoint: `${origin}/oauth/register`,
    revocation_endpoint: `${origin}/oauth/revoke`,
    response_types_supported: ["code"],
    grant_types_supported: ["authorization_code", "refresh_token"],
    code_challenge_methods_supported: ["S256"],
    token_endpoint_auth_methods_supported: ["none"],
    scopes_supported: ["recipes"],
  }
}

export function protectedResourceMetadata(origin: string) {
  return {
    resource: `${origin}/mcp`,
    authorization_servers: [origin],
    scopes_supported: ["recipes"],
    bearer_methods_supported: ["header"],
    resource_name: "Just the Recipe",
  }
}

export function isAllowedRedirectUri(uri: string): boolean {
  try {
    const url = new URL(uri)
    if (url.hash) return false
    if (url.protocol === "https:") return true
    return url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
  } catch {
    return false
  }
}

export function registerClient(name: string | null, redirectUris: string[]) {
  const id = randomToken(16)
  useDB().insert(oauthClients).values({ id, name, redirectUris }).run()
  return id
}

export function getClient(id: string) {
  return useDB().select().from(oauthClients).where(eq(oauthClients.id, id)).get()
}

function issue(
  kind: keyof typeof LIFETIME,
  clientId: string,
  extra: { codeChallenge?: string; redirectUri?: string } = {},
) {
  const token = randomToken(32)
  useDB()
    .insert(oauthTokens)
    .values({
      hash: sha256(token),
      kind,
      clientId,
      codeChallenge: extra.codeChallenge ?? null,
      redirectUri: extra.redirectUri ?? null,
      expiresAt: new Date(Date.now() + LIFETIME[kind]),
    })
    .run()
  return token
}

/** Looks up an unexpired token of the given kind and deletes it (single use). */
function consume(kind: "code" | "refresh", token: string) {
  const db = useDB()
  const row = db
    .select()
    .from(oauthTokens)
    .where(
      and(
        eq(oauthTokens.hash, sha256(token)),
        eq(oauthTokens.kind, kind),
        gt(oauthTokens.expiresAt, new Date()),
      ),
    )
    .get()
  if (row) db.delete(oauthTokens).where(eq(oauthTokens.hash, row.hash)).run()
  return row
}

export function createAuthorizationCode(
  clientId: string,
  codeChallenge: string,
  redirectUri: string,
) {
  return issue("code", clientId, { codeChallenge, redirectUri })
}

function tokenResponse(clientId: string) {
  // Opportunistically purge expired rows
  useDB().delete(oauthTokens).where(lt(oauthTokens.expiresAt, new Date())).run()
  return {
    access_token: issue("access", clientId),
    token_type: "Bearer",
    expires_in: Math.floor(LIFETIME.access / 1000),
    refresh_token: issue("refresh", clientId),
    scope: "recipes",
  }
}

function oauthError(statusCode: number, error: string, description: string) {
  return createError({
    statusCode,
    statusMessage: description,
    data: { error, error_description: description },
  })
}

export function exchangeAuthorizationCode(params: {
  code?: string
  clientId?: string
  redirectUri?: string
  codeVerifier?: string
}) {
  const { code, clientId, redirectUri, codeVerifier } = params
  if (!code || !codeVerifier)
    throw oauthError(400, "invalid_request", "Missing code or code_verifier")
  const row = consume("code", code)
  if (!row) throw oauthError(400, "invalid_grant", "Authorization code is invalid or expired")
  if (clientId && clientId !== row.clientId) {
    throw oauthError(400, "invalid_grant", "Code was issued to another client")
  }
  if (redirectUri && redirectUri !== row.redirectUri) {
    throw oauthError(400, "invalid_grant", "redirect_uri does not match")
  }
  const challenge = createHash("sha256").update(codeVerifier).digest("base64url")
  if (challenge !== row.codeChallenge)
    throw oauthError(400, "invalid_grant", "PKCE verification failed")
  return tokenResponse(row.clientId)
}

export function exchangeRefreshToken(refreshToken?: string) {
  if (!refreshToken) throw oauthError(400, "invalid_request", "Missing refresh_token")
  const row = consume("refresh", refreshToken)
  if (!row) throw oauthError(400, "invalid_grant", "Refresh token is invalid or expired")
  return tokenResponse(row.clientId)
}

export function revokeToken(token: string) {
  useDB()
    .delete(oauthTokens)
    .where(eq(oauthTokens.hash, sha256(token)))
    .run()
}

/** Validates the bearer token on an MCP request. Always true when auth is disabled. */
export function hasValidAccessToken(event: H3Event): boolean {
  if (!authEnabled()) return true
  const header = getHeader(event, "authorization") ?? ""
  const token = header.match(/^Bearer\s+(.+)$/i)?.[1]
  if (!token) return false
  const row = useDB()
    .select({ hash: oauthTokens.hash })
    .from(oauthTokens)
    .where(
      and(
        eq(oauthTokens.hash, sha256(token)),
        eq(oauthTokens.kind, "access"),
        gt(oauthTokens.expiresAt, new Date()),
      ),
    )
    .get()
  return Boolean(row)
}
