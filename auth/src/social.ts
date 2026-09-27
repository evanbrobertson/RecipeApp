/**
 * Sign in with Google and Apple, when their keys are set: the same env vars as Crumb's own
 * accounts (`src/social.rs`), so one set of keys works for either edition.
 *
 * - Google: `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`
 * - Apple: `APPLE_CLIENT_ID` (the Services ID), `APPLE_TEAM_ID`, `APPLE_KEY_ID` and
 *   `APPLE_PRIVATE_KEY` (the .p8 key's PEM). Apple's client secret is a JWT signed with
 *   that key and good for six months at most, so it's made here and remade daily.
 */
import { sign } from "node:crypto"

export type SocialEnv = Record<string, string | undefined>

type Provider = { clientId: string; clientSecret: string }

export type Social = {
  /** Better Auth's `socialProviders`. */
  providers: { google?: Provider; apple?: Provider & { appBundleIdentifier?: string } }
  /** Which are on, for Crumb's sign-in page. */
  ids: ("google" | "apple")[]
}

const DAY = 60 * 60 * 24

const b64url = (data: string | Buffer) => Buffer.from(data).toString("base64url")

/** Apple's client secret: an ES256 JWT, valid for `days`. */
export function appleClientSecret(
  opts: { clientId: string; teamId: string; keyId: string; privateKey: string },
  now = Math.floor(Date.now() / 1000),
  days = 150,
) {
  const header = b64url(JSON.stringify({ alg: "ES256", kid: opts.keyId }))
  const claims = b64url(
    JSON.stringify({
      iss: opts.teamId,
      iat: now,
      exp: now + days * DAY,
      aud: "https://appleid.apple.com",
      sub: opts.clientId,
    }),
  )
  const input = `${header}.${claims}`
  const signature = sign("sha256", Buffer.from(input), {
    key: opts.privateKey.replace(/\\n/g, "\n"),
    dsaEncoding: "ieee-p1363",
  })
  return `${input}.${b64url(signature)}`
}

export function socialFromEnv(env: SocialEnv = process.env): Social {
  const get = (k: string) => env[k]?.trim() || undefined
  const social: Social = { providers: {}, ids: [] }
  const googleId = get("GOOGLE_CLIENT_ID")
  const googleSecret = get("GOOGLE_CLIENT_SECRET")
  if (googleId && googleSecret) {
    social.providers.google = { clientId: googleId, clientSecret: googleSecret }
    social.ids.push("google")
  }
  const [clientId, teamId, keyId, privateKey] = [
    get("APPLE_CLIENT_ID"),
    get("APPLE_TEAM_ID"),
    get("APPLE_KEY_ID"),
    get("APPLE_PRIVATE_KEY"),
  ]
  if (clientId && teamId && keyId && privateKey) {
    const opts = { clientId, teamId, keyId, privateKey }
    let made = 0
    let secret = ""
    // Better Auth reads the secret when it swaps a code, so a getter keeps it fresh
    const current = () => {
      const now = Math.floor(Date.now() / 1000)
      if (now - made > DAY) {
        secret = appleClientSecret(opts, now)
        made = now
      }
      return secret
    }
    try {
      current()
      social.providers.apple = {
        clientId: opts.clientId,
        get clientSecret() {
          return current()
        },
      }
      social.ids.push("apple")
    } catch (err) {
      console.warn("[auth] Sign in with Apple is off: APPLE_PRIVATE_KEY isn't usable", err)
    }
  }
  return social
}
