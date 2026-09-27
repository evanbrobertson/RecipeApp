/**
 * Accounts and households, whichever way this Crumb signs people in: its own accounts
 * (AUTH_MODE=accounts, `/api/auth/*` answered by the Rust server) or the hosted edition's
 * Better Auth (AUTH_MODE=hosted, the same paths proxied to the auth service). The pages
 * only use this, so they don't care which.
 */
import { api } from "./api"

export type Mode = "password" | "accounts" | "hosted"

export type Provider = "google" | "apple"

export const providerNames: Record<Provider, string> = { google: "Google", apple: "Apple" }

export type Status = {
  mode: Mode
  signedIn?: boolean
  setupNeeded?: boolean
  setupNeedsAppPassword?: boolean
  signupOpen?: boolean
  /** One password: whether it's set (so there's a Sign out). */
  passwordRequired?: boolean
  /** Google and Apple, when this Crumb has their keys. */
  providers?: Provider[]
  user?: { name: string; email: string } | null
  household?: { id: number; name: string; role: string } | null
}

export type Member = {
  id: string
  name: string
  email: string
  role: string
  you: boolean
}

export type PendingInvite = {
  id: string
  /** Hosted invites go to one address; self-hosted ones are links anyone can use once. */
  email?: string
  createdBy?: string | null
  /** Unix seconds. */
  expiresAt: number
}

export type Household = {
  id: string
  name: string
  role: string
  members: Member[]
  invites: PendingInvite[]
  /** Every household the person is in, this one included. */
  households: { id: string; name: string }[]
}

export type Device = {
  id: string
  userAgent: string | null
  /** Unix seconds. */
  lastSeenAt: number
  current: boolean
}

export type InvitePreview = {
  householdName?: string
  invitedBy?: string | null
  /** Hosted: the address the invite was sent to (only it can accept). */
  email?: string
}

export type SignInMethods = {
  password: boolean
  linked: { provider: Provider; email?: string | null; createdAt: number }[]
}

/** Where a Google or Apple sign-in is headed: sign in, add it to this account, or join. */
export type SocialIntent =
  | { intent: "login"; next?: string }
  | { intent: "link" }
  | { intent: "invite"; invite: string }

/** An app connected to Crumb (Claude, mostly): one per app and household. */
export type ConnectedApp = {
  id: string
  name: string
  household: { id: number; name: string | null } | null
  /** Unix seconds. */
  connectedAt: number
}

export type Credentials = { name?: string; email: string; password: string }

let status: Promise<Status> | null = null

/** How this Crumb signs people in, and who's signed in (fetched once per page). */
export function authStatus(): Promise<Status> {
  status ??= api<Status>("/api/auth/status").catch(() => ({ mode: "password" as const }))
  return status
}

const secs = (date: string | number | Date) => Math.floor(new Date(date).getTime() / 1000)

export interface Accounts {
  signIn(email: string, password: string): Promise<void>
  /** Resolves to whether the new account must confirm its email before signing in. */
  signUp(c: Required<Credentials>, callbackURL?: string): Promise<{ verify: boolean }>
  signOut(): Promise<void>
  devices(): Promise<Device[]>
  signOutDevice(d: Device): Promise<void>
  signOutOthers(): Promise<void>
  household(): Promise<Household>
  rename(name: string): Promise<void>
  /** Self-hosted: a link to hand over. Hosted: an email is sent to `email` (and the link returned too). */
  invite(email?: string): Promise<{ url: string }>
  cancelInvite(i: PendingInvite): Promise<void>
  removeMember(m: Member): Promise<void>
  leave(h: Household): Promise<void>
  switchTo(id: string): Promise<void>
  /** What an invite is for, when it can still be used (hosted: only once signed in). */
  previewInvite(token: string): Promise<InvitePreview | null>
  /** Joins; signed out, `creds` make an account (with a name) or sign in to one first. */
  acceptInvite(token: string, creds?: Credentials): Promise<{ verify: boolean }>
  /** Hosted, with email: sends a password reset link. */
  requestReset?(email: string): Promise<void>
  resetPassword?(token: string, password: string): Promise<void>
  /** The provider's page to send the browser to. */
  socialStart(provider: Provider, to: SocialIntent): Promise<string>
  signInMethods(): Promise<SignInMethods>
  unlink(provider: Provider): Promise<void>
}

/** Crumb's own accounts (AUTH_MODE=accounts). */
const own: Accounts = {
  async signIn(email, password) {
    await api("/api/auth/login", { method: "POST", body: { email, password } })
  },
  async signUp(c) {
    await api("/api/auth/signup", { method: "POST", body: c })
    return { verify: false }
  },
  async signOut() {
    await api("/api/auth/logout", { method: "POST" })
  },
  async devices() {
    type Row = { id: number; userAgent: string | null; lastSeenAt: number; current: boolean }
    const rows = await api<Row[]>("/api/auth/sessions")
    return rows.map((r) => ({ ...r, id: String(r.id) }))
  },
  async signOutDevice(d) {
    await api(`/api/auth/sessions/${d.id}`, { method: "DELETE" })
  },
  async signOutOthers() {
    await api("/api/auth/sessions/revoke-others", { method: "POST" })
  },
  async household() {
    type Row = {
      id: number
      name: string
      role: string
      you: number
      members: { userId: number; name: string; email: string; role: string }[]
      invites: { id: number; createdBy: string | null; expiresAt: number }[]
      households: { id: number; name: string }[]
    }
    const h = await api<Row>("/api/auth/household")
    return {
      id: String(h.id),
      name: h.name,
      role: h.role,
      members: h.members.map((m) => ({
        id: String(m.userId),
        name: m.name,
        email: m.email,
        role: m.role,
        you: m.userId === h.you,
      })),
      invites: h.invites.map((i) => ({ ...i, id: String(i.id) })),
      households: h.households.map((o) => ({ id: String(o.id), name: o.name })),
    }
  },
  async rename(name) {
    await api("/api/auth/household", { method: "PATCH", body: { name } })
  },
  async invite() {
    return api<{ url: string }>("/api/auth/invites", { method: "POST" })
  },
  async cancelInvite(i) {
    await api(`/api/auth/invites/${i.id}`, { method: "DELETE" })
  },
  async removeMember(m) {
    await api(`/api/auth/members/${m.id}`, { method: "DELETE" })
  },
  async leave() {
    await api("/api/auth/household/leave", { method: "POST" })
  },
  async switchTo(id) {
    await api("/api/auth/household/switch", { method: "POST", body: { id: Number(id) } })
  },
  async previewInvite(token) {
    return api<InvitePreview>("/api/auth/invite/preview", {
      method: "POST",
      body: { token },
    }).catch(() => null)
  },
  async acceptInvite(token, creds) {
    await api("/api/auth/invite/accept", { method: "POST", body: { token, ...creds } })
    return { verify: false }
  },
  async socialStart(provider, to) {
    const made = await api<{ url: string }>(`/api/auth/social/${provider}/start`, {
      method: "POST",
      body: to,
    })
    return made.url
  },
  async signInMethods() {
    return api<SignInMethods>("/api/auth/identities")
  },
  async unlink(provider) {
    await api(`/api/auth/identities/${provider}`, { method: "DELETE" })
  },
}

/** Better Auth (AUTH_MODE=hosted): households are its organizations. */
const hosted: Accounts = {
  async signIn(email, password) {
    await api("/api/auth/sign-in/email", { method: "POST", body: { email, password } })
  },
  async signUp(c, callbackURL = "/") {
    const res = await api<{ token: string | null }>("/api/auth/sign-up/email", {
      method: "POST",
      body: { ...c, callbackURL },
    })
    return { verify: !res.token }
  },
  async signOut() {
    await api("/api/auth/sign-out", { method: "POST", body: {} })
  },
  async devices() {
    type Row = { id: string; token: string; userAgent?: string | null; updatedAt: string }
    const [rows, current] = await Promise.all([
      api<Row[]>("/api/auth/list-sessions"),
      api<{ session: { id: string } } | null>("/api/auth/get-session"),
    ])
    return rows
      .map((r) => ({
        id: r.token,
        userAgent: r.userAgent ?? null,
        lastSeenAt: secs(r.updatedAt),
        current: r.id === current?.session.id,
      }))
      .sort((a, b) => Number(b.current) - Number(a.current) || b.lastSeenAt - a.lastSeenAt)
  },
  async signOutDevice(d) {
    await api("/api/auth/revoke-session", { method: "POST", body: { token: d.id } })
  },
  async signOutOthers() {
    await api("/api/auth/revoke-other-sessions", { method: "POST", body: {} })
  },
  async household() {
    type Org = {
      id: string
      name: string
      members: { id: string; userId: string; role: string; user: { name: string; email: string } }[]
      invitations: { id: string; email: string; status: string; expiresAt: string }[]
    }
    const [org, orgs, me] = await Promise.all([
      api<Org>("/api/auth/organization/get-full-organization"),
      api<{ id: string; name: string }[]>("/api/auth/organization/list"),
      api<{ user: { id: string } } | null>("/api/auth/get-session"),
    ])
    const mine = org.members.find((m) => m.userId === me?.user.id)
    const members = org.members
      .map((m) => ({
        id: m.id,
        name: m.user.name,
        email: m.user.email,
        role: m.role,
        you: m.userId === me?.user.id,
      }))
      .sort((a, b) => Number(b.role === "owner") - Number(a.role === "owner"))
    return {
      id: org.id,
      name: org.name,
      role: mine?.role ?? "member",
      members,
      invites: org.invitations
        .filter((i) => i.status === "pending" && secs(i.expiresAt) > Date.now() / 1000)
        .map((i) => ({ id: i.id, email: i.email, expiresAt: secs(i.expiresAt) })),
      households: orgs.map((o) => ({ id: o.id, name: o.name })),
    }
  },
  async rename(name) {
    await api("/api/auth/organization/update", { method: "POST", body: { data: { name } } })
  },
  async invite(email) {
    const made = await api<{ id: string }>("/api/auth/organization/invite-member", {
      method: "POST",
      body: { email, role: "member" },
    })
    return { url: `${location.origin}/invite#${made.id}` }
  },
  async cancelInvite(i) {
    await api("/api/auth/organization/cancel-invitation", {
      method: "POST",
      body: { invitationId: i.id },
    })
  },
  async removeMember(m) {
    await api("/api/auth/organization/remove-member", {
      method: "POST",
      body: { memberIdOrEmail: m.id },
    })
  },
  async leave(h) {
    await api("/api/auth/organization/leave", { method: "POST", body: { organizationId: h.id } })
    // Crumb's server picks their next household (or makes one) on the next request
  },
  async switchTo(id) {
    await api("/api/auth/organization/set-active", { method: "POST", body: { organizationId: id } })
  },
  async previewInvite(token) {
    type Invite = { organizationName: string; inviterEmail: string; email: string }
    const i = await api<Invite>(
      `/api/auth/organization/get-invitation?id=${encodeURIComponent(token)}`,
    ).catch(() => null)
    return i && { householdName: i.organizationName, invitedBy: i.inviterEmail, email: i.email }
  },
  async acceptInvite(token, creds) {
    if (creds?.name) {
      const made = await hosted.signUp(
        { name: creds.name, email: creds.email, password: creds.password },
        `/invite#${token}`,
      )
      if (made.verify) return made
    } else if (creds) {
      await hosted.signIn(creds.email, creds.password)
    }
    const joined = await api<{ member: { organizationId: string } }>(
      "/api/auth/organization/accept-invitation",
      { method: "POST", body: { invitationId: token } },
    )
    await hosted.switchTo(joined.member.organizationId)
    return { verify: false }
  },
  async requestReset(email) {
    await api("/api/auth/request-password-reset", {
      method: "POST",
      body: { email, redirectTo: "/reset-password" },
    })
  },
  async resetPassword(token, newPassword) {
    await api("/api/auth/reset-password", { method: "POST", body: { token, newPassword } })
  },
  async socialStart(provider, to) {
    type Started = { url: string }
    if (to.intent === "link") {
      const made = await api<Started>("/api/auth/link-social", {
        method: "POST",
        body: {
          provider,
          callbackURL: `/more/account?linked=${provider}`,
          errorCallbackURL: "/more/account",
        },
      })
      return made.url
    }
    // Back to the invite page to accept, signed in (it reads the invite from the fragment)
    const callbackURL = to.intent === "invite" ? `/invite#${to.invite}` : (to.next ?? "/")
    const made = await api<Started>("/api/auth/sign-in/social", {
      method: "POST",
      body: { provider, callbackURL, errorCallbackURL: "/login" },
    })
    return made.url
  },
  async signInMethods() {
    type Row = { providerId: string; createdAt: string }
    const rows = await api<Row[]>("/api/auth/list-accounts")
    return {
      password: rows.some((r) => r.providerId === "credential"),
      linked: rows
        .filter(
          (r): r is Row & { providerId: Provider } =>
            r.providerId === "google" || r.providerId === "apple",
        )
        .map((r) => ({ provider: r.providerId, createdAt: secs(r.createdAt) })),
    }
  },
  async unlink(provider) {
    type Row = { id: string; providerId: string }
    const rows = await api<Row[]>("/api/auth/list-accounts")
    const row = rows.find((r) => r.providerId === provider)
    if (!row) return
    await api("/api/auth/unlink-account", { method: "POST", body: { accountId: row.id } })
  },
}

export function accounts(mode: Mode): Accounts {
  return mode === "hosted" ? hosted : own
}

/** Connected apps, whichever way this Crumb signs people in. */
export function connectedApps(): Promise<ConnectedApp[]> {
  return api<ConnectedApp[]>("/api/connections")
}

export async function disconnectApp(app: ConnectedApp): Promise<void> {
  await api(`/api/connections/${encodeURIComponent(app.id)}`, { method: "DELETE" })
}

/** Deletes the signed-in account: its password, or with none, its email typed out. */
export async function deleteAccount(confirm: { password?: string; confirm?: string }) {
  await api("/api/account/delete", { method: "POST", body: confirm })
}

/**
 * Words for the `?error=` a Google or Apple sign-in came back with: Crumb's own codes
 * (src/social.rs) and Better Auth's. Only known codes: never text from the URL.
 */
export function socialError(code: string | null): string | null {
  if (!code) return null
  const words: Record<string, string> = {
    cancelled: "Sign-in was cancelled.",
    access_denied: "Sign-in was cancelled.",
    expired: "That sign-in took too long, or was started in another browser. Try again.",
    email_taken:
      "There's already an account with that email. Sign in with your password, then link it under More → Account.",
    account_not_linked:
      "There's already an account with that email. Sign in with your password, then link it under More → Account.",
    signup_closed:
      "There's no account for that sign-in, and sign-up isn't open here. Ask for an invite.",
    signup_disabled:
      "There's no account for that sign-in, and sign-up isn't open here. Ask for an invite.",
    not_set_up: "Crumb isn't set up yet. Make the first account with an email and password.",
    invite_gone: "This invite has expired or was already used. Ask for a new one.",
    already_linked: "That sign-in is already linked to another account.",
    account_already_linked_to_different_user: "That sign-in is already linked to another account.",
    unverified: "That account's email isn't verified yet.",
    email_not_verified: "That account's email isn't verified yet.",
    no_email: "That sign-in didn't share an email address.",
    email_not_found: "That sign-in didn't share an email address.",
  }
  return words[code] ?? "Couldn't sign in with that. Try again."
}
