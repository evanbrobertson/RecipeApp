/**
 * Changing the email on a hosted account. Crumb's server asks for it (`/internal/change-email`)
 * with the browser's cookies, and passes on the links' tokens (`/internal/confirm-email`).
 *
 * 1. It's really them: the password, or a sign-in from the last day (see `reauthenticate`).
 * 2. With email: the new address gets a link, and nothing changes until it's opened, so a
 *    mistyped address never takes over. Without email, the change is made at once (like
 *    sign-up, which doesn't wait on a verified email either).
 * 3. Once changed, the old address is told, with a link that puts it back for a week and signs
 *    out every device. That's what undoes a change made by someone who got in: they can't
 *    read the old inbox. The link only goes to an address that was verified, never to one
 *    that might have been a typo.
 *
 * The links carry a random token in their fragment (the page posts it once asked to, so a mail
 * scanner opening the link changes nothing), and only its SHA-256 is kept, in Better Auth's
 * verification table.
 */
import { createHash, randomBytes } from "node:crypto"
import { reauthenticate, refuse } from "./account"
import type { Auth } from "./auth"
import type { Mailer } from "./mailer"

const HOUR = 60 * 60 * 1000
/** How long the link to the new address works. */
const CONFIRM_MS = 24 * HOUR
/** How long the old address can put itself back. */
const REVERT_MS = 7 * 24 * HOUR

type User = { id: string; email: string; name: string; emailVerified: boolean }

/** What a link does, kept against its token's hash. */
type Pending =
  | { kind: "confirm"; userId: string; from: string; to: string }
  | { kind: "revert"; userId: string; to: string }

const identifier = (token: string) =>
  `crumb-email:${createHash("sha256").update(token).digest("hex")}`

function wellFormed(email: string) {
  return (
    email.length <= 254 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) && !email.includes("..")
  )
}

async function findUser(auth: Auth, where: { field: "id" | "email"; value: string }) {
  const ctx = await auth.$context
  return ctx.adapter.findOne<User>({ model: "user", where: [where] })
}

async function setEmail(auth: Auth, userId: string, email: string, verified: boolean) {
  const ctx = await auth.$context
  await ctx.adapter.update({
    model: "user",
    where: [{ field: "id", value: userId }],
    update: { email, emailVerified: verified, updatedAt: new Date() },
  })
}

/** A new link's token: `c.` to confirm or `r.` to revert, so its page can say which before it's used. */
async function remember(auth: Auth, pending: Pending, ms: number) {
  const token = `${pending.kind[0]}.${randomBytes(32).toString("base64url")}`
  const ctx = await auth.$context
  await ctx.internalAdapter.createVerificationValue({
    identifier: identifier(token),
    value: JSON.stringify(pending),
    expiresAt: new Date(Date.now() + ms),
  })
  return token
}

/** `{email, password}` from the signed-in person: `{pending, email}`, where `pending` means a link was sent. */
export async function changeEmail(
  auth: Auth,
  mailer: Mailer,
  baseURL: string,
  headers: Headers,
  body: { email: string; password: string },
): Promise<Response> {
  const found = await auth.api.getSession({ headers })
  if (!found) return refuse(401, "Not signed in")
  const { user, session } = found
  const email = body.email.trim().toLowerCase()
  if (!wellFormed(email)) return refuse(400, "email: Enter a valid email address")
  if (email === user.email.toLowerCase()) return refuse(400, "email: That's already your email")
  const refused = await reauthenticate(auth, user.id, session, body.password, "change your email")
  if (refused) return refused
  if (await findUser(auth, { field: "email", value: email })) {
    return refuse(409, "email: There's already an account with that email")
  }

  if (!mailer.enabled) {
    await setEmail(auth, user.id, email, false)
    return Response.json({ ok: true, pending: false, email })
  }
  const token = await remember(
    auth,
    { kind: "confirm", userId: user.id, from: user.email, to: email },
    CONFIRM_MS,
  )
  await mailer.send({
    to: email,
    subject: "Confirm your new email for Crumb",
    text: `${user.name}, you asked to use this address for your Crumb account.\n\nConfirm it here: ${baseURL}/email-change#${token}\n\nThe link works for a day. Until then you still sign in with ${user.email}. If it wasn't you, ignore this email and nothing changes.`,
  })
  return Response.json({ ok: true, pending: true, email })
}

/**
 * `{token}` from a link: confirms a new address (`{done: "changed"}`) or puts the old one back
 * (`{done: "reverted"}`). Works signed out, from whichever browser opened the email.
 */
export async function confirmEmail(
  auth: Auth,
  mailer: Mailer,
  baseURL: string,
  token: string,
): Promise<Response> {
  const gone = () => refuse(410, "This link has expired or was already used.")
  if (!token) return gone()
  const ctx = await auth.$context
  const id = identifier(token)
  const row = await ctx.internalAdapter.findVerificationValue(id)
  if (!row) return gone()
  // One use, whatever happens next
  await ctx.internalAdapter.deleteVerificationByIdentifier(id)
  if (new Date(row.expiresAt).getTime() < Date.now()) return gone()
  const pending = JSON.parse(row.value) as Pending
  const user = await findUser(auth, { field: "id", value: pending.userId })
  if (!user) return gone()
  const taken = await findUser(auth, { field: "email", value: pending.to })
  if (taken && taken.id !== user.id) {
    return refuse(409, "There's already an account with that email.")
  }

  if (pending.kind === "revert") {
    await setEmail(auth, user.id, pending.to, true)
    // Whoever changed it may still be signed in: nobody is, now
    await ctx.adapter.deleteMany({ model: "session", where: [{ field: "userId", value: user.id }] })
    return Response.json({ ok: true, done: "reverted", email: pending.to })
  }

  // Asked for from an address that has changed since: out of date
  if (user.email.toLowerCase() !== pending.from.toLowerCase()) return gone()
  await setEmail(auth, user.id, pending.to, true)
  if (user.emailVerified) {
    const notice = `The email for your Crumb account was changed from ${user.email} to ${pending.to}.`
    const revert = await remember(
      auth,
      { kind: "revert", userId: user.id, to: user.email },
      REVERT_MS,
    )
    await mailer.send({
      to: user.email,
      subject: "Your Crumb email was changed",
      text: `${notice}\n\nIf that was you, there's nothing to do.\n\nIf it wasn't, put your email back and sign out every device here: ${baseURL}/email-change#${revert}\n\nThen reset your password. The link works for a week.`,
    })
  }
  return Response.json({ ok: true, done: "changed", email: pending.to })
}
