/**
 * What Crumb's server asks this service: who a request is signed in as, and in which
 * household (organization). Everyone has at least one: someone in none (just signed up,
 * or removed from their only one) gets a kitchen of their own on first use, the way a
 * self-hosted account does.
 */
import type { Auth } from "./auth"

type Member = { id: string; organizationId: string; userId: string; role: string; createdAt: Date }
type Org = { id: string; name: string }

export type CrumbSession = {
  user: { id: string; email: string; name: string; emailVerified: boolean }
  /** None only when asked not to make one (someone just signed up to accept an invite). */
  household: { id: string; name: string; role: string } | null
}

/** Kitchens being made, per user, so parallel first requests make only one. */
const making = new Map<string, Promise<Member>>()

export async function crumbSession(
  auth: Auth,
  headers: Headers,
  { create = true } = {},
): Promise<CrumbSession | null> {
  const found = await auth.api.getSession({ headers })
  if (!found) return null
  const { session, user } = found
  const ctx = await auth.$context
  const active = (session as { activeOrganizationId?: string | null }).activeOrganizationId
  let member = active ? await findMember(auth, active, user.id) : null
  const who = { id: user.id, email: user.email, name: user.name, emailVerified: user.emailVerified }
  if (!member) {
    member = await firstMembership(auth, user.id)
    if (!member && !create) return { user: who, household: null }
    member ??= await makeKitchen(auth, user)
    await ctx.internalAdapter.updateSession(session.token, {
      activeOrganizationId: member.organizationId,
    })
  }
  const org = await ctx.adapter.findOne<Org>({
    model: "organization",
    where: [{ field: "id", value: member.organizationId }],
  })
  if (!org) return null
  return { user: who, household: { id: org.id, name: org.name, role: member.role } }
}

/** Whether a person is (still) in a household: Claude's connector checks this. */
export async function isMember(auth: Auth, userId: string, organizationId: string) {
  return (await findMember(auth, organizationId, userId)) !== null
}

async function findMember(auth: Auth, organizationId: string, userId: string) {
  const ctx = await auth.$context
  return ctx.adapter.findOne<Member>({
    model: "member",
    where: [
      { field: "organizationId", value: organizationId },
      { field: "userId", value: userId },
    ],
  })
}

async function firstMembership(auth: Auth, userId: string) {
  const ctx = await auth.$context
  const [first] = await ctx.adapter.findMany<Member>({
    model: "member",
    where: [{ field: "userId", value: userId }],
    sortBy: { field: "createdAt", direction: "asc" },
    limit: 1,
  })
  return first ?? null
}

/** "Ann's kitchen", as a self-hosted Crumb names a new household. */
export function kitchenName(name: string) {
  const first = name.trim().split(/\s+/)[0] || name
  return `${first}'s kitchen`
}

function makeKitchen(auth: Auth, user: { id: string; name: string }): Promise<Member> {
  const pending = making.get(user.id)
  if (pending) return pending
  const made = (async () => {
    // A system action (no headers): the only way a household is made
    await auth.api.createOrganization({
      body: {
        name: kitchenName(user.name),
        slug: `kitchen-${crypto.randomUUID()}`,
        userId: user.id,
      },
    })
    const member = await firstMembership(auth, user.id)
    if (!member) throw new Error("the new kitchen has no owner")
    return member
  })().finally(() => making.delete(user.id))
  making.set(user.id, made)
  return made
}
