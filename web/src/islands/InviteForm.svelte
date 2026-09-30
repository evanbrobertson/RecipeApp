<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { onMount } from "svelte"
  import PasswordInput from "../components/PasswordInput.svelte"
  import SocialButtons from "../components/SocialButtons.svelte"
  import {
    type Accounts,
    type InvitePreview,
    type Status,
    accounts,
    authStatus,
    socialError,
  } from "../lib/account"
  import { errorMessage } from "../lib/api"
  import { flash } from "../lib/toast"

  /**
   * An invite link's page (`/invite#<token>`): the token stays in the fragment, so it never
   * reaches a server log. Signed in, one button joins; signed out, making an account or
   * signing in joins too. Self-hosted invites are links anyone can use once; hosted ones
   * (Better Auth) only work for the address they were sent to.
   */
  const token = typeof location === "undefined" ? "" : decodeURIComponent(location.hash.slice(1))

  let status = $state<Status | null>(null)
  let client = $state<Accounts | null>(null)
  let preview = $state<InvitePreview | null>(null)
  let ready = $state(false)
  let tab = $state<"new" | "existing">("new")
  let name = $state("")
  let email = $state("")
  let password = $state("")
  let loading = $state(false)
  let error = $state(
    socialError(
      typeof location === "undefined" ? null : new URLSearchParams(location.search).get("error"),
    ) ?? "",
  )
  let checkEmail = $state(false)

  const hosted = $derived(status?.mode === "hosted")
  const signedIn = $derived(!!status?.user)
  /** Self-hosted, a link that's used up or expired shows nothing else. */
  const gone = $derived(ready && !preview && (!hosted || signedIn))

  onMount(async () => {
    status = await authStatus()
    if (token && status.mode !== "password") {
      client = accounts(status.mode)
      // Hosted invites can only be read by the person they're for, once signed in
      if (!hosted || signedIn) preview = await client.previewInvite(token)
      if (preview?.email) email = preview.email
    }
    ready = true
  })

  async function join(e?: SubmitEvent) {
    e?.preventDefault()
    loading = true
    error = ""
    try {
      const creds = signedIn
        ? undefined
        : { email, password, ...(tab === "new" ? { name } : {}) }
      const done = await client!.acceptInvite(token, creds)
      if (done.verify) {
        checkEmail = true
        loading = false
        return
      }
      flash({ title: preview?.householdName ? `Welcome to ${preview.householdName}` : "You're in" })
      location.href = "/"
    } catch (err) {
      error = errorMessage(err, "Couldn't join")
      loading = false
    }
  }

  async function signOut() {
    await client?.signOut().catch(() => {})
    location.reload()
  }
</script>

{#if !ready}
  <p class="text-ink-muted">Loading…</p>
{:else if !token || status?.mode === "password"}
  <p class="text-ink-muted">
    This link isn't a Crumb invite. Ask whoever sent it for a new one.
  </p>
{:else if checkEmail}
  <p class="mb-3 font-bold">Check your email</p>
  <p class="text-ink-muted">
    We sent a link to {email}. Open it to confirm your address; it brings you back here to join.
  </p>
{:else if gone}
  <p class="mb-3 font-bold">This invite can't be used</p>
  <p class="text-ink-muted">
    {hosted && signedIn
      ? `It has expired, was cancelled, or was sent to another address than ${status?.user?.email}.`
      : "It has expired, was cancelled or was already used. Ask for a new one."}
  </p>
  {#if hosted && signedIn}
    <button type="button" class="btn btn-ghost mt-4" onclick={signOut}>
      Use another account
    </button>
  {/if}
{:else}
  <p class="text-ink-muted mb-5">
    {#if preview?.householdName}
      {preview.invitedBy ?? "Someone"} invited you to <strong class="text-ink">{preview.householdName}</strong>
      on Crumb, to share their recipes.
    {:else}
      You've been invited to share a recipe box on Crumb. Use the email address the invite was
      sent to.
    {/if}
  </p>
  {#if signedIn}
    <p class="text-ink-muted mb-5 text-sm">
      Signed in as {status?.user?.email}. Your own recipes stay where they are; you can switch
      between households under More → Account.
    </p>
    {#if error}<p class="text-error mb-3 text-sm" role="alert">{error}</p>{/if}
    <button
      type="button"
      class="btn btn-primary btn-lg w-full"
      disabled={loading}
      onclick={() => join()}
    >
      {#if loading}<LoaderCircle class="animate-spin" />{/if}
      Join {preview?.householdName ?? "the household"}
    </button>
  {:else}
    <SocialButtons
      mode={status!.mode}
      providers={status?.providers}
      to={{ intent: "invite", invite: token }}
    />
    <div class="mb-4 flex gap-2" role="tablist" aria-label="Your account">
      <button
        type="button"
        role="tab"
        aria-selected={tab === "new"}
        class={["btn flex-1", tab === "new" ? "bg-tint text-primary" : "btn-ghost"]}
        onclick={() => (tab = "new")}
      >
        I'm new here
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={tab === "existing"}
        class={["btn flex-1", tab === "existing" ? "bg-tint text-primary" : "btn-ghost"]}
        onclick={() => (tab = "existing")}
      >
        I have an account
      </button>
    </div>
    <form class="space-y-4" onsubmit={join}>
      {#if tab === "new"}
        <div>
          <label class="label" for="name">Your name</label>
          <input
            id="name"
            name="name"
            bind:value={name}
            autocomplete="name"
            class="input input-lg"
            maxlength="80"
            required
          />
        </div>
      {/if}
      <div>
        <label class="label" for="email">Email</label>
        <input
          id="email"
          name="email"
          type="email"
          bind:value={email}
          autocomplete={tab === "new" ? "email" : "username"}
          autocapitalize="none"
          spellcheck="false"
          class="input input-lg"
          required
        />
      </div>
      <div>
        <label class="label" for="password">Password</label>
        <PasswordInput
          id="password"
          bind:value={password}
          autocomplete={tab === "new" ? "new-password" : "current-password"}
          minlength={tab === "new" ? 8 : undefined}
          describedby={tab === "new" ? "password-hint" : undefined}
        />
        {#if tab === "new"}
          <p id="password-hint" class="text-ink-muted mt-1.5 text-sm">At least 8 characters.</p>
        {/if}
      </div>
      {#if error}<p class="text-error text-sm" role="alert">{error}</p>{/if}
      <button type="submit" class="btn btn-primary btn-lg w-full" disabled={loading}>
        {#if loading}<LoaderCircle class="animate-spin" />{/if}
        {tab === "new" ? "Create account and join" : "Sign in and join"}
      </button>
    </form>
  {/if}
{/if}
