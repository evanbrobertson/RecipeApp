import { expect, test } from "bun:test"
import { createHash, generateKeyPairSync, randomBytes, sign } from "node:crypto"
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
  /** Each person's cookies, by name (a passkey challenge cookie joins the session's). */
  const jar = new Map<string, Map<string, string>>()
  async function call(
    who: string,
    path: string,
    body?: unknown,
    headers: Record<string, string> = {},
  ) {
    const h = new Headers(headers)
    h.set("origin", ORIGIN)
    const mine = jar.get(who) ?? new Map<string, string>()
    jar.set(who, mine)
    if (mine.size) h.set("cookie", [...mine].map(([k, v]) => `${k}=${v}`).join("; "))
    if (body !== undefined) h.set("content-type", "application/json")
    const res = await app.fetch(
      new Request(`${ORIGIN}${path}`, {
        method: body === undefined ? "GET" : "POST",
        headers: h,
        body: body === undefined ? undefined : JSON.stringify(body),
      }),
    )
    for (const c of res.headers.getSetCookie()) {
      const pair = c.split(";")[0]!
      const at = pair.indexOf("=")
      const [name, value] = [pair.slice(0, at), pair.slice(at + 1)]
      if (value) mine.set(name, value)
      else mine.delete(name)
    }
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

/** A passkey on a pretend device: P-256, "none" attestation, as a platform authenticator makes. */
function softAuthenticator() {
  const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" })
  const jwk = publicKey.export({ format: "jwk" })
  const id = randomBytes(16)
  let counter = 0
  const b64url = (b: Uint8Array) => Buffer.from(b).toString("base64url")
  const sha256 = (b: Uint8Array | string) => createHash("sha256").update(b).digest()
  const u32 = (n: number) => Buffer.from([n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255])
  const rpHash = sha256(new URL(ORIGIN).hostname)
  const clientData = (type: string, challenge: string) =>
    Buffer.from(JSON.stringify({ type, challenge, origin: ORIGIN, crossOrigin: false }))

  return {
    id: b64url(id),
    register(challenge: string) {
      // COSE EC2 key {1: 2, 3: -7, -1: 1, -2: x, -3: y} and {fmt, attStmt, authData}, in CBOR
      const bytes = (b: Buffer) => Buffer.concat([Buffer.from([0x58, b.length]), b])
      const text = (t: string) => Buffer.concat([Buffer.from([0x60 + t.length]), Buffer.from(t)])
      const cose = Buffer.concat([
        Buffer.from([0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21]),
        bytes(Buffer.from(jwk.x!, "base64url")),
        Buffer.from([0x22]),
        bytes(Buffer.from(jwk.y!, "base64url")),
      ])
      const authData = Buffer.concat([
        rpHash,
        Buffer.from([0x45]), // user present, user verified, attested credential data
        u32(counter),
        Buffer.alloc(16), // aaguid
        Buffer.from([0, id.length]),
        id,
        cose,
      ])
      const attestation = Buffer.concat([
        Buffer.from([0xa3]),
        text("fmt"),
        text("none"),
        text("attStmt"),
        Buffer.from([0xa0]),
        text("authData"),
        Buffer.from([0x59, authData.length >> 8, authData.length & 255]),
        authData,
      ])
      return {
        id: b64url(id),
        rawId: b64url(id),
        type: "public-key",
        authenticatorAttachment: "platform",
        response: {
          clientDataJSON: b64url(clientData("webauthn.create", challenge)),
          attestationObject: b64url(attestation),
          transports: ["internal"],
        },
      }
    },
    authenticate(challenge: string) {
      const authData = Buffer.concat([rpHash, Buffer.from([0x05]), u32(++counter)])
      const data = clientData("webauthn.get", challenge)
      return {
        id: b64url(id),
        rawId: b64url(id),
        type: "public-key",
        authenticatorAttachment: "platform",
        response: {
          clientDataJSON: b64url(data),
          authenticatorData: b64url(authData),
          signature: b64url(sign("sha256", Buffer.concat([authData, sha256(data)]), privateKey)),
        },
      }
    },
  }
}

test("a passkey added once signed in signs in without a password", async () => {
  const { call, internal, signUp } = await setup()
  const device = softAuthenticator()

  // Only someone signed in can add one
  expect((await call("stranger", "/api/auth/passkey/generate-register-options")).status).toBe(401)

  await signUp("ann", "Ann Cook")
  const ann = (await internal("ann", "/internal/session")).data.session
  const options = await call("ann", "/api/auth/passkey/generate-register-options")
  expect(options.status).toBe(200)
  expect(options.data.rp).toEqual({ name: "Crumb", id: "localhost" })
  const added = await call("ann", "/api/auth/passkey/verify-registration", {
    response: device.register(options.data.challenge),
    name: "Ann's phone",
  })
  expect(added.status).toBe(200)
  const list = await call("ann", "/api/auth/passkey/list-user-passkeys")
  expect(list.data.map((p: { name: string }) => p.name)).toEqual(["Ann's phone"])

  // On another device: no cookies, just the passkey
  const challenge = await call("phone", "/api/auth/passkey/generate-authenticate-options")
  expect(challenge.status).toBe(200)
  const signedIn = await call("phone", "/api/auth/passkey/verify-authentication", {
    response: device.authenticate(challenge.data.challenge),
  })
  expect(signedIn.status).toBe(200)
  const phone = (await internal("phone", "/internal/session")).data.session
  expect(phone.user.id).toBe(ann.user.id)
  expect(phone.household.id).toBe(ann.household.id)

  // A challenge is good once
  const replay = await call("phone", "/api/auth/passkey/verify-authentication", {
    response: device.authenticate(challenge.data.challenge),
  })
  expect(replay.status).toBe(400)

  // Removed, it no longer signs anyone in
  const removed = await call("ann", "/api/auth/passkey/delete-passkey", { id: list.data[0].id })
  expect(removed.status).toBe(200)
  const again = await call("thief", "/api/auth/passkey/generate-authenticate-options")
  const refused = await call("thief", "/api/auth/passkey/verify-authentication", {
    response: device.authenticate(again.data.challenge),
  })
  expect(refused.status).toBe(401)
  expect((await internal("thief", "/internal/session")).data.session).toBeNull()
})
