import { expect, test } from "bun:test"
import { generateKeyPairSync, verify } from "node:crypto"
import { logMailer } from "../src/mailer"
import { createApp } from "../src/server"
import { type Social, appleClientSecret, socialFromEnv } from "../src/social"

const ORIGIN = "http://localhost:3000"
const SECRET = "internal-secret-for-tests"

async function setup(social?: Social) {
  const mailer = logMailer()
  const app = await createApp({
    dbPath: ":memory:",
    baseURL: ORIGIN,
    secret: "a-test-secret-that-is-long-enough-for-better-auth",
    internalSecret: SECRET,
    mailer,
    social,
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
  const internal = (who: string, path: string, body?: unknown) =>
    call(who, path, body, { "x-crumb-internal": SECRET })
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

test("Google and Apple are offered when their keys are set", async () => {
  const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" })
  const pem = privateKey.export({ type: "pkcs8", format: "pem" }).toString()
  expect(socialFromEnv({}).ids).toEqual([])
  const social = socialFromEnv({
    GOOGLE_CLIENT_ID: "google-client",
    GOOGLE_CLIENT_SECRET: "google-secret",
    APPLE_CLIENT_ID: "com.example.crumb",
    APPLE_TEAM_ID: "TEAM",
    APPLE_KEY_ID: "KEY1",
    // As an env var often carries it: one line, with \n
    APPLE_PRIVATE_KEY: pem.replace(/\n/g, "\\n"),
  })
  expect(social.ids).toEqual(["google", "apple"])

  // Apple's client secret is an ES256 JWT that its public key verifies
  const jwt = social.providers.apple!.clientSecret
  const [h, c, sig] = jwt.split(".")
  expect(JSON.parse(Buffer.from(h!, "base64url").toString())).toEqual({ alg: "ES256", kid: "KEY1" })
  const claims = JSON.parse(Buffer.from(c!, "base64url").toString())
  expect(claims).toMatchObject({ iss: "TEAM", sub: "com.example.crumb", aud: "https://appleid.apple.com" })
  expect(claims.exp - claims.iat).toBeLessThanOrEqual(180 * 24 * 3600)
  const ok = verify("sha256", Buffer.from(`${h}.${c}`), {
    key: publicKey,
    dsaEncoding: "ieee-p1363",
  }, Buffer.from(sig!, "base64url"))
  expect(ok).toBe(true)
  expect(appleClientSecret({ clientId: "a", teamId: "b", keyId: "c", privateKey: pem })).toContain(".")

  const { call, internal } = await setup(social)
  expect((await internal("x", "/internal/providers")).data).toEqual({ providers: ["google", "apple"] })
  const started = await call("x", "/api/auth/sign-in/social", {
    provider: "google",
    callbackURL: "/",
  })
  expect(started.status).toBe(200)
  const url = new URL(started.data.url)
  expect(url.host).toBe("accounts.google.com")
  expect(url.searchParams.get("client_id")).toBe("google-client")
  expect(url.searchParams.get("redirect_uri")).toBe(`${ORIGIN}/api/auth/callback/google`)
})

test("people can download their data and delete their account", async () => {
  const { call, internal, signUp } = await setup()
  for (const [who, name] of [["ann", "Ann"], ["bob", "Bob"], ["cat", "Cat"]] as const) {
    await signUp(who, name)
  }
  const ann = (await internal("ann", "/internal/session")).data.session
  const cat = (await internal("cat", "/internal/session")).data.session
  // Bob joins Ann's kitchen
  const invited = await call("ann", "/api/auth/organization/invite-member", {
    email: "bob@example.com",
    role: "member",
  })
  await call("bob", "/api/auth/organization/accept-invitation", { invitationId: invited.data.id })
  await internal("bob", "/internal/session")

  // Needs the cookies
  expect((await internal("nobody", "/internal/export", {})).status).toBe(401)
  const data = (await internal("ann", "/internal/export", {})).data
  expect(data.account).toMatchObject({ name: "Ann", email: "ann@example.com" })
  expect(data.signInMethods).toEqual({ password: true, linked: [] })
  expect(data.households).toEqual([
    expect.objectContaining({ externalId: ann.household.id, name: "Ann's kitchen", role: "owner" }),
  ])
  expect(data.devices.length).toBeGreaterThan(0)

  // The password is checked
  const wrong = await internal("ann", "/internal/delete-account", { password: "nope" })
  expect(wrong.status).toBe(401)
  expect(wrong.data.message).toContain("Incorrect password")

  // Ann goes: Bob keeps the kitchen, as its owner
  const gone = await internal("ann", "/internal/delete-account", { password: "long enough" })
  expect(gone.status).toBe(200)
  expect(gone.data.deletedHouseholds).toEqual([])
  expect((await internal("ann", "/internal/session")).data.session).toBeNull()
  const bob = (await internal("bob", "/internal/session")).data.session
  expect(bob.household).toMatchObject({ id: ann.household.id, role: "owner" })
  const signIn = await call("ann2", "/api/auth/sign-in/email", {
    email: "ann@example.com",
    password: "long enough",
  })
  expect(signIn.status).toBe(401)

  // Cat had hers to herself: it goes with her
  const catGone = await internal("cat", "/internal/delete-account", { password: "long enough" })
  expect(catGone.data.deletedHouseholds).toEqual([cat.household.id])
})
