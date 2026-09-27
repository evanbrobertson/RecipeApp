/**
 * Better Auth for the hosted edition (`AUTH_MODE=hosted`): email + password accounts,
 * passkeys, sessions, and households as Better Auth organizations (members and email invites).
 *
 * Crumb's server (Rust) proxies `/api/auth/*` here and asks `/internal/*` (see
 * `server.ts`) who a request is signed in as. It keeps each household's recipes in a
 * SQLite file of its own, keyed by the organization's id, so this service only ever knows
 * people and memberships, never recipes.
 *
 * Membership rules are Crumb's, not the plugin's defaults: people can't make or delete
 * organizations themselves (everyone gets one kitchen, made on first use), roles are only
 * owner and member, and invites are always for members.
 */
import type { Database } from "bun:sqlite"
import { passkey } from "@better-auth/passkey"
import { type BetterAuthOptions, betterAuth } from "better-auth"
import { APIError, createAuthMiddleware } from "better-auth/api"
import { organization } from "better-auth/plugins"
import type { Mailer } from "./mailer"
import type { Social } from "./social"

const DAY = 60 * 60 * 24

export type AuthOptions = {
  db: Database
  /** The public origin people use (`SITE_URL`), e.g. https://crumb.example. */
  baseURL: string
  /** `BETTER_AUTH_SECRET`: signs cookies and tokens. */
  secret: string
  mailer: Mailer
  /** Google and Apple, when configured (see social.ts). */
  social?: Social
}

export function createAuth(opts: AuthOptions) {
  return betterAuth(authOptions(opts))
}

export function authOptions(opts: AuthOptions) {
  const { mailer, baseURL } = opts
  const social = opts.social ?? { providers: {}, ids: [] }
  return {
    appName: "Crumb",
    baseURL,
    basePath: "/api/auth",
    secret: opts.secret,
    database: opts.db,
    // Apple posts its answer back from its own origin
    trustedOrigins: social.ids.includes("apple")
      ? [baseURL, "https://appleid.apple.com"]
      : [baseURL],
    socialProviders: social.providers,
    account: {
      accountLinking: {
        enabled: true,
        // Linking from More → Account is done signed in, so the provider's address may
        // differ (Apple's "Hide My Email"). Signing in with Google or Apple only joins an
        // existing account when both sides have verified the email (Better Auth's
        // requireLocalEmailVerified, left on), so nobody takes over an account by its address.
        allowDifferentEmails: true,
      },
    },
    emailAndPassword: {
      enabled: true,
      minPasswordLength: 8,
      maxPasswordLength: 256,
      // Only when a verification email can actually be sent
      requireEmailVerification: mailer.enabled,
      revokeSessionsOnPasswordReset: true,
      sendResetPassword: async ({ user, url }) => {
        await mailer.send({
          to: user.email,
          subject: "Reset your Crumb password",
          text: `Someone (hopefully you) asked to reset the password for your Crumb account.\n\nChoose a new one here: ${url}\n\nIf it wasn't you, ignore this email and nothing changes.`,
        })
      },
    },
    emailVerification: {
      sendOnSignUp: mailer.enabled,
      autoSignInAfterVerification: true,
      sendVerificationEmail: async ({ user, url }) => {
        await mailer.send({
          to: user.email,
          subject: "Confirm your email for Crumb",
          text: `Welcome to Crumb, ${user.name}.\n\nConfirm your email to open your recipe box: ${url}`,
        })
      },
    },
    session: {
      // As long as a self-hosted session: 90 days from last use
      expiresIn: 90 * DAY,
      updateAge: DAY,
    },
    rateLimit: {
      enabled: true,
      window: 60,
      max: 100,
      customRules: {
        "/sign-in/email": { window: 60, max: 10 },
        "/sign-up/email": { window: 60, max: 5 },
        "/request-password-reset": { window: 60, max: 3 },
        "/sign-in/social": { window: 60, max: 10 },
        "/passkey/verify-authentication": { window: 60, max: 10 },
      },
    },
    advanced: {
      cookiePrefix: "crumb",
      // Crumb's server sets X-Real-IP to the client's address (and strips what the client sent)
      ipAddress: { ipAddressHeaders: ["x-real-ip"] },
    },
    hooks: {
      before: createAuthMiddleware(async (ctx) => {
        // Roles are owner and member only: no promotions, no admin invites
        if (ctx.path === "/organization/update-member-role") {
          throw new APIError("FORBIDDEN", { message: "Roles can't be changed in Crumb" })
        }
        if (ctx.path === "/organization/invite-member") {
          const body = ctx.body as { role?: unknown } | undefined
          if (body?.role !== undefined && body.role !== "member") {
            throw new APIError("BAD_REQUEST", { message: "Invite people as members" })
          }
        }
      }),
    },
    plugins: [
      // Passkeys are added once signed in (More → Account) and then sign in without a
      // password. Scoped to SITE_URL's host, and only ever accepted from that origin.
      passkey({
        rpName: "Crumb",
        rpID: new URL(baseURL).hostname,
        origin: baseURL,
      }),
      organization({
        // Kitchens are made by Crumb (see kitchen.ts), and never deleted from the browser:
        // a household's recipe file must not be orphaned
        allowUserToCreateOrganization: false,
        disableOrganizationDeletion: true,
        creatorRole: "owner",
        membershipLimit: 50,
        invitationLimit: 20,
        invitationExpiresIn: 7 * DAY,
        cancelPendingInvitationsOnReInvite: true,
        requireEmailVerificationOnInvitation: mailer.enabled,
        sendInvitationEmail: async ({ id, email, organization, inviter }) => {
          await mailer.send({
            to: email,
            subject: `${inviter.user.name} invited you to ${organization.name} on Crumb`,
            text: `${inviter.user.name} would like to share their recipe box, ${organization.name}, with you on Crumb.\n\nJoin here: ${baseURL}/invite#${id}\n\nThe invite works for a week, and only for ${email}.`,
          })
        },
      }),
    ],
  } satisfies BetterAuthOptions
}

export type Auth = ReturnType<typeof createAuth>
