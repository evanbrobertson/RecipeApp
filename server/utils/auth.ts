import { createHash, randomBytes, timingSafeEqual } from "node:crypto"
import type { H3Event } from "h3"

/** Whether a password is configured. Without one the app is open (local dev). */
export function authEnabled(): boolean {
  return Boolean(useRuntimeConfig().appPassword)
}

export function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex")
}

export function randomToken(bytes = 32): string {
  return randomBytes(bytes).toString("base64url")
}

export function checkPassword(candidate: string): boolean {
  const expected = Buffer.from(sha256(useRuntimeConfig().appPassword))
  const actual = Buffer.from(sha256(candidate))
  return timingSafeEqual(expected, actual)
}

/** Session cookie sealed with a key derived from the password, so changing it signs everyone out. */
function sessionConfig(event: H3Event) {
  const https = getRequestProtocol(event, { xForwardedProto: true }) === "https"
  return {
    name: "jtr_session",
    password: sha256(`jtr-session:${useRuntimeConfig().appPassword}`),
    maxAge: 60 * 60 * 24 * 90,
    // SameSite=Lax also stops cross-site form posts from riding on the session
    cookie: { sameSite: "lax" as const, httpOnly: true, secure: https },
  }
}

export async function isLoggedIn(event: H3Event): Promise<boolean> {
  if (!authEnabled()) return true
  const session = await getSession<{ ok?: boolean }>(event, sessionConfig(event))
  return session.data.ok === true
}

export async function logIn(event: H3Event) {
  const session = await useSession<{ ok?: boolean }>(event, sessionConfig(event))
  await session.update({ ok: true })
}

export async function logOut(event: H3Event) {
  const session = await useSession(event, sessionConfig(event))
  await session.clear()
}

/** Public origin of the app, used for OAuth metadata and the connector URL. */
export function publicOrigin(event: H3Event): string {
  const configured = useRuntimeConfig().public.siteUrl
  if (configured) return configured.replace(/\/+$/, "")
  if (process.env.RAILWAY_PUBLIC_DOMAIN) return `https://${process.env.RAILWAY_PUBLIC_DOMAIN}`
  return getRequestURL(event, { xForwardedHost: true, xForwardedProto: true }).origin
}
