/**
 * Accounts and households, whichever way this Crumb signs people in: its own accounts
 * (AUTH_MODE=accounts, `/api/auth/*` answered by the Rust server) or the hosted edition's
 * Better Auth (AUTH_MODE=hosted, the same paths proxied to the auth service). The pages
 * only use this, so they don't care which.
 */
import type {
  PublicKeyCredentialCreationOptionsJSON,
  PublicKeyCredentialRequestOptionsJSON,
} from "@simplewebauthn/browser"
import { ApiError, api } from "./api"

export type Mode = "password" | "accounts" | "hosted"

export type Status = {
  mode: Mode
  signedIn?: boolean
  setupNeeded?: boolean
  setupNeedsAppPassword?: boolean
  signupOpen?: boolean
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

export type Credentials = { name?: string; email: string; password: string }

export type Passkey = {
  id: string
  name: string
  /** Unix seconds. */
  createdAt: number
}

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
  /** Hosted: passkeys, when this browser can use them. */
  passkeys?: Passkeys
}

export interface Passkeys {
  supported(): boolean
  /** Whether the browser can offer passkeys in the email field's autofill. */
  autofill(): Promise<boolean>
  /**
   * Signs in with a passkey; `autofill` waits for one picked from the email field. Resolves
   * false when the person cancelled (or another ceremony took over).
   */
  signIn(opts?: { autofill?: boolean }): Promise<boolean>
  list(): Promise<Passkey[]>
  /** Resolves false when the person cancelled. */
  add(name: string): Promise<boolean>
  remove(p: Passkey): Promise<void>
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
}

/** "Firefox on Linux", roughly: enough to tell one's devices (and passkeys) apart. */
export function deviceName(agent: string | null) {
  if (!agent) return "Unknown device"
  const browser = /Edg\//.test(agent)
    ? "Edge"
    : /Firefox\//.test(agent)
      ? "Firefox"
      : /Chrome\//.test(agent)
        ? "Chrome"
        : /Safari\//.test(agent)
          ? "Safari"
          : /okhttp|Android/.test(agent)
            ? "Crumb app"
            : "Browser"
  const system = /iPhone|iPad/.test(agent)
    ? "iOS"
    : /Android/.test(agent)
      ? "Android"
      : /Mac OS X/.test(agent)
        ? "macOS"
        : /Windows/.test(agent)
          ? "Windows"
          : /Linux/.test(agent)
            ? "Linux"
            : ""
  return system ? `${browser} on ${system}` : browser
}

/** The person closed the prompt, or a newer ceremony (the button, over autofill) replaced it. */
const cancelled = (err: unknown) =>
  err instanceof Error && (err.name === "NotAllowedError" || err.name === "AbortError")

/**
 * Better Auth's passkey plugin. The WebAuthn helper only loads when a passkey is used, so it
 * stays off the sign-in page's critical path.
 */
const passkeys: Passkeys = {
  supported() {
    return typeof window !== "undefined" && "PublicKeyCredential" in window
  },
  async autofill() {
    if (!passkeys.supported()) return false
    const { browserSupportsWebAuthnAutofill } = await import("@simplewebauthn/browser")
    return browserSupportsWebAuthnAutofill()
  },
  async signIn({ autofill = false } = {}) {
    const { startAuthentication } = await import("@simplewebauthn/browser")
    const optionsJSON = await api<PublicKeyCredentialRequestOptionsJSON>(
      "/api/auth/passkey/generate-authenticate-options",
    )
    let response
    try {
      response = await startAuthentication({ optionsJSON, useBrowserAutofill: autofill })
    } catch (err) {
      if (cancelled(err)) return false
      throw err
    }
    const { clientExtensionResults: _, ...body } = response
    await api("/api/auth/passkey/verify-authentication", {
      method: "POST",
      body: { response: body },
    })
    return true
  },
  async list() {
    type Row = { id: string; name?: string | null; createdAt: string }
    const rows = await api<Row[]>("/api/auth/passkey/list-user-passkeys")
    return rows
      .map((r) => ({ id: r.id, name: r.name || "Passkey", createdAt: secs(r.createdAt) }))
      .sort((a, b) => b.createdAt - a.createdAt)
  },
  async add(name) {
    const { startRegistration } = await import("@simplewebauthn/browser")
    let optionsJSON: PublicKeyCredentialCreationOptionsJSON
    try {
      optionsJSON = await api("/api/auth/passkey/generate-register-options")
    } catch (err) {
      // Better Auth wants a sign-in from the last day before adding a way to sign in
      if (err instanceof ApiError && err.status === 403) {
        throw new ApiError("For safety, sign out and back in, then add the passkey.", 403)
      }
      throw err
    }
    let response
    try {
      response = await startRegistration({ optionsJSON })
    } catch (err) {
      if (cancelled(err)) return false
      if (err instanceof Error && err.name === "InvalidStateError") {
        throw new Error("This device already has a passkey for Crumb", { cause: err })
      }
      throw err
    }
    const { clientExtensionResults: _, ...body } = response
    await api("/api/auth/passkey/verify-registration", {
      method: "POST",
      body: { response: body, name },
    })
    return true
  },
  async remove(p) {
    await api("/api/auth/passkey/delete-passkey", { method: "POST", body: { id: p.id } })
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
  passkeys,
}

export function accounts(mode: Mode): Accounts {
  return mode === "hosted" ? hosted : own
}
