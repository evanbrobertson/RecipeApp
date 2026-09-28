/**
 * "Download my data" and "Delete my account" for the hosted edition. Crumb's server asks
 * for them (`/internal/export`, `/internal/delete-account`) with the browser's cookies, adds
 * the recipes it keeps per household, and deletes the recipe files of the households that go.
 *
 * Deleting an account follows the self-hosted rules: a household it shares passes to whoever
 * joined first; one it had to itself is deleted. It needs the password when the account has
 * one, or else a sign-in from the last day (Better Auth's `freshAge`).
 */
import type { Auth } from "./auth"

type Member = { id: string; organizationId: string; userId: string; role: string; createdAt: Date }
type Org = { id: string; name: string }
type Account = { providerId: string; password?: string | null; createdAt: Date }
type Session = { userAgent?: string | null; createdAt: Date; updatedAt: Date }
type Passkey = { name?: string | null; createdAt: Date; deviceType: string }

const secs = (d: Date | string) => Math.floor(new Date(d).getTime() / 1000)

export function refuse(status: number, message: string) {
  return Response.json({ message }, { status })
}

/**
 * Proof that it's really them before a change they can't easily take back: the password when
 * the account has one, or else a sign-in (a passkey, Google, Apple) from the last day, Better
 * Auth's `freshAge`. The refusal to answer with, or null to go ahead.
 */
export async function reauthenticate(
  auth: Auth,
  userId: string,
  session: { createdAt: Date },
  password: string,
  action: string,
): Promise<Response | null> {
  const ctx = await auth.$context
  const credential = (await ctx.internalAdapter.findCredentialAccount(userId)) as Account | null
  if (credential?.password) {
    const ok =
      !!password && (await ctx.password.verify({ hash: credential.password, password }))
    return ok ? null : refuse(401, "password: Incorrect password")
  }
  const age = Date.now() - new Date(session.createdAt).getTime()
  if (age > ctx.sessionConfig.freshAge * 1000) {
    return refuse(403, `For your safety, sign out and in again, then ${action}.`)
  }
  return null
}

export async function exportAccount(auth: Auth, headers: Headers): Promise<Response> {
  const found = await auth.api.getSession({ headers })
  if (!found) return refuse(401, "Not signed in")
  const { user } = found
  const ctx = await auth.$context
  const accounts = (await ctx.internalAdapter.findAccounts(user.id)) as Account[]
  const sessions = (await ctx.internalAdapter.listSessions(user.id)) as Session[]
  const members = await ctx.adapter.findMany<Member>({
    model: "member",
    where: [{ field: "userId", value: user.id }],
    sortBy: { field: "createdAt", direction: "asc" },
  })
  const passkeys = await ctx.adapter.findMany<Passkey>({
    model: "passkey",
    where: [{ field: "userId", value: user.id }],
  })
  const households = []
  for (const m of members) {
    const org = await ctx.adapter.findOne<Org>({
      model: "organization",
      where: [{ field: "id", value: m.organizationId }],
    })
    if (org) {
      households.push({
        externalId: org.id,
        name: org.name,
        role: m.role,
        joinedAt: secs(m.createdAt),
      })
    }
  }
  return Response.json({
    account: {
      name: user.name,
      email: user.email,
      emailVerified: user.emailVerified,
      createdAt: secs(user.createdAt),
    },
    signInMethods: {
      password: accounts.some((a) => a.providerId === "credential"),
      linked: accounts
        .filter((a) => a.providerId !== "credential")
        .map((a) => ({ provider: a.providerId, createdAt: secs(a.createdAt) })),
      passkeys: passkeys.map((p) => ({ name: p.name ?? null, createdAt: secs(p.createdAt) })),
    },
    households,
    devices: sessions.map((s) => ({
      userAgent: s.userAgent ?? null,
      signedInAt: secs(s.createdAt),
      lastSeenAt: secs(s.updatedAt),
    })),
  })
}

export async function deleteAccount(
  auth: Auth,
  headers: Headers,
  password: string,
): Promise<Response> {
  const found = await auth.api.getSession({ headers })
  if (!found) return refuse(401, "Not signed in")
  const { user, session } = found
  const ctx = await auth.$context
  const refused = await reauthenticate(auth, user.id, session, password, "delete your account")
  if (refused) return refused

  const deletedHouseholds: string[] = []
  const owned = await ctx.adapter.findMany<Member>({
    model: "member",
    where: [
      { field: "userId", value: user.id },
      { field: "role", value: "owner" },
    ],
  })
  for (const m of owned) {
    const others = (
      await ctx.adapter.findMany<Member>({
        model: "member",
        where: [{ field: "organizationId", value: m.organizationId }],
        sortBy: { field: "createdAt", direction: "asc" },
      })
    ).filter((o) => o.userId !== user.id)
    const heir = others.find((o) => o.role === "owner") ?? others[0]
    if (heir) {
      if (heir.role !== "owner") {
        await ctx.adapter.update({
          model: "member",
          where: [{ field: "id", value: heir.id }],
          update: { role: "owner" },
        })
      }
      continue
    }
    for (const model of ["invitation", "member"]) {
      await ctx.adapter.deleteMany({
        model,
        where: [{ field: "organizationId", value: m.organizationId }],
      })
    }
    await ctx.adapter.delete({
      model: "organization",
      where: [{ field: "id", value: m.organizationId }],
    })
    deletedHouseholds.push(m.organizationId)
  }
  for (const model of ["member", "passkey"]) {
    await ctx.adapter.deleteMany({ model, where: [{ field: "userId", value: user.id }] })
  }
  await ctx.internalAdapter.deleteUser(user.id)
  return Response.json({ ok: true, deletedHouseholds })
}
