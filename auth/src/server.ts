/**
 * crumb-auth: Better Auth for Crumb's hosted edition, as a small service beside the Rust
 * server. The server proxies `/api/auth/*` here unchanged and calls `/internal/*` with the
 * shared `AUTH_INTERNAL_SECRET`; nothing here should be reachable from the internet directly.
 */
import { Database } from "bun:sqlite"
import { timingSafeEqual } from "node:crypto"
import { getMigrations } from "better-auth/db/migration"
import { type Auth, authOptions, createAuth } from "./auth"
import { crumbSession, isMember } from "./kitchen"
import { type Mailer, mailerFromEnv } from "./mailer"

export type AppOptions = {
  dbPath: string
  baseURL: string
  secret: string
  internalSecret: string
  mailer: Mailer
}

export async function createApp(opts: AppOptions) {
  const db = new Database(opts.dbPath, { create: true })
  db.exec("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
  const options = { db, baseURL: opts.baseURL, secret: opts.secret, mailer: opts.mailer }
  // Better Auth owns this database's schema: create and add what's missing on every start
  const { runMigrations } = await getMigrations(authOptions(options))
  await runMigrations()
  const auth = createAuth(options)
  return { auth, fetch: (req: Request) => handle(auth, opts, req) }
}

function allowed(req: Request, secret: string) {
  const given = Buffer.from(req.headers.get("x-crumb-internal") ?? "")
  const expected = Buffer.from(secret)
  return given.length === expected.length && timingSafeEqual(given, expected)
}

async function handle(auth: Auth, opts: AppOptions, req: Request): Promise<Response> {
  const url = new URL(req.url)
  if (url.pathname === "/health") return Response.json({ ok: true })
  if (url.pathname.startsWith("/api/auth/")) return auth.handler(req)
  if (url.pathname.startsWith("/internal/")) {
    if (!allowed(req, opts.internalSecret)) return new Response("Forbidden", { status: 403 })
    try {
      return await internal(auth, url, req)
    } catch (err) {
      console.error("[internal]", err)
      return Response.json({ message: "Couldn't check the session" }, { status: 500 })
    }
  }
  return new Response("Not found", { status: 404 })
}

async function internal(auth: Auth, url: URL, req: Request): Promise<Response> {
  switch (url.pathname) {
    // The browser's cookies, forwarded: who they're signed in as, and where (`?create=0`:
    // without making a kitchen for someone in no household yet)
    case "/internal/session": {
      const create = url.searchParams.get("create") !== "0"
      return Response.json({ session: await crumbSession(auth, req.headers, { create }) })
    }
    case "/internal/member": {
      const user = url.searchParams.get("user") ?? ""
      const household = url.searchParams.get("household") ?? ""
      return Response.json({
        member: !!user && !!household && (await isMember(auth, user, household)),
      })
    }
    default:
      return new Response("Not found", { status: 404 })
  }
}

function required(name: string) {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(`${name} is required`)
  return value
}

if (import.meta.main) {
  const app = await createApp({
    // On Railway, the service's own volume (like Crumb's recipes.db)
    dbPath:
      process.env.AUTH_DB_PATH ??
      (process.env.RAILWAY_VOLUME_MOUNT_PATH
        ? `${process.env.RAILWAY_VOLUME_MOUNT_PATH}/auth.db`
        : "auth.db"),
    baseURL: required("SITE_URL").replace(/\/+$/, ""),
    secret: required("BETTER_AUTH_SECRET"),
    internalSecret: required("AUTH_INTERNAL_SECRET"),
    mailer: mailerFromEnv(),
  })
  const server = Bun.serve({
    hostname: process.env.HOST ?? "::",
    port: Number(process.env.PORT ?? 3100),
    fetch: app.fetch,
  })
  console.info(`crumb-auth listening on ${server.url}`)
}
