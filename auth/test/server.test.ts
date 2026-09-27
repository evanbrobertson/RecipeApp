import { expect, test } from "bun:test"
import { logMailer } from "../src/mailer"
import { createApp } from "../src/server"

const ORIGIN = "http://localhost:3000"
const SECRET = "internal-secret-for-tests"

async function setup() {
  const mailer = logMailer()
  const app = await createApp({
    dbPath: ":memory:",
    baseURL: ORIGIN,
    secret: "a-test-secret-that-is-long-enough-for-better-auth",
    internalSecret: SECRET,
    mailer,
  })
  const jar = new Map<string, string>()
  async function call(
    who: string,
    path: string,
    body?: unknown,
    headers: Record<string, string> = {},
  ) {
    const h = new Headers(headers)
    h.set("origin", ORIGIN)
    if (jar.has(who)) h.set("cookie", jar.get(who)!)
    if (body !== undefined) h.set("content-type", "application/json")
    const res = await app.fetch(
      new Request(`${ORIGIN}${path}`, {
        method: body === undefined ? "GET" : "POST",
        headers: h,
        body: body === undefined ? undefined : JSON.stringify(body),
      }),
    )
    const cookies = res.headers
      .getSetCookie()
      .map((c) => c.split(";")[0]!)
      .filter((c) => !c.endsWith("="))
    if (cookies.length) jar.set(who, cookies.join("; "))
    const text = await res.text()
    let data: any = text
    try {
      data = JSON.parse(text)
    } catch {}
    return { status: res.status, data }
  }
  const internal = (who: string, path: string) =>
    call(who, path, undefined, { "x-crumb-internal": SECRET })
  const signUp = (who: string, name: string) =>
    call(who, "/api/auth/sign-up/email", {
      email: `${who}@example.com`,
      password: "long enough",
      name,
    })
  return { app, mailer, call, internal, signUp }
}

test("everyone gets a kitchen, and invites bring people into one", async () => {
  const { mailer, call, internal, signUp } = await setup()
  expect((await signUp("ann", "Ann Cook")).status).toBe(200)

  // Internal endpoints need the shared secret
  expect((await call("ann", "/internal/session")).status).toBe(403)
  const ann = (await internal("ann", "/internal/session")).data.session
  expect(ann.user.name).toBe("Ann Cook")
  expect(ann.household).toMatchObject({ name: "Ann's kitchen", role: "owner" })
  // Asking again doesn't make another
  expect((await internal("ann", "/internal/session")).data.session.household.id).toBe(
    ann.household.id,
  )
  expect((await internal("nobody", "/internal/session")).data.session).toBeNull()

  // Households are made by Crumb only, and never deleted from the browser
  expect(
    (await call("ann", "/api/auth/organization/create", { name: "Mine", slug: "mine" })).status,
  ).toBe(403)
  const del = await call("ann", "/api/auth/organization/delete", {
    organizationId: ann.household.id,
  })
  expect(del.status).toBeGreaterThanOrEqual(400)

  // Invites are for members, by email; the email carries the invite page's link
  const asAdmin = await call("ann", "/api/auth/organization/invite-member", {
    email: "bob@example.com",
    role: "admin",
  })
  expect(asAdmin.status).toBe(400)
  const invited = await call("ann", "/api/auth/organization/invite-member", {
    email: "bob@example.com",
    role: "member",
  })
  expect(invited.status).toBe(200)
  const link = mailer.sent.at(-1)!.text.match(/\/invite#(\S+)/)![1]
  expect(link).toBe(invited.data.id)

  // Bob signs up, accepts and lands in Ann's kitchen without a kitchen of his own
  await signUp("bob", "Bob")
  // Asked without making one (Crumb's status), Bob has no household until he accepts
  const before = (await internal("bob", "/internal/session?create=0")).data.session
  expect(before.user.name).toBe("Bob")
  expect(before.household).toBeNull()
  expect(
    (await call("bob", "/api/auth/organization/accept-invitation", { invitationId: link })).status,
  ).toBe(200)
  const bob = (await internal("bob", "/internal/session")).data.session
  expect(bob.household).toMatchObject({
    id: ann.household.id,
    name: "Ann's kitchen",
    role: "member",
  })
  const orgs = await call("bob", "/api/auth/organization/list")
  expect(orgs.data).toHaveLength(1)
  const q = `/internal/member?user=${bob.user.id}&household=${ann.household.id}`
  expect((await internal("bob", q)).data.member).toBe(true)

  // Roles stay owner and member
  const promote = await call("ann", "/api/auth/organization/update-member-role", {
    memberId: bob.user.id,
    role: "owner",
  })
  expect(promote.status).toBe(403)

  // Removed, Bob's connector stops and he gets his own (empty) kitchen
  expect(
    (
      await call("ann", "/api/auth/organization/remove-member", {
        memberIdOrEmail: "bob@example.com",
      })
    ).status,
  ).toBe(200)
  expect((await internal("bob", q)).data.member).toBe(false)
  const after = (await internal("bob", "/internal/session")).data.session
  expect(after.household).toMatchObject({ name: "Bob's kitchen", role: "owner" })
  expect(after.household.id).not.toBe(ann.household.id)
})
