<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { onMount } from "svelte"
  import PasswordInput from "../components/PasswordInput.svelte"
  import { type Accounts, accounts, authStatus } from "../lib/account"
  import { errorMessage } from "../lib/api"
  import { flash } from "../lib/toast"

  /**
   * Hosted only: asking for a password reset email, and choosing a new password from its
   * link (which lands here with `?token=`). Self-hosted, an owner can't reset a member's
   * password yet, so this page just points back to signing in.
   */
  const params = typeof location === "undefined" ? null : new URLSearchParams(location.search)
  const token = params?.get("token") ?? ""
  const invalid = params?.get("error") === "INVALID_TOKEN"

  let client = $state<Accounts | null>(null)
  let ready = $state(false)
  let email = $state("")
  let password = $state("")
  let sent = $state(false)
  let loading = $state(false)
  let error = $state("")

  onMount(async () => {
    const status = await authStatus()
    if (status.mode === "hosted") client = accounts("hosted")
    ready = true
  })

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    loading = true
    error = ""
    try {
      if (token) {
        await client!.resetPassword!(token, password)
        flash({ title: "Password changed. Sign in with the new one." })
        location.href = "/login"
        return
      }
      await client!.requestReset!(email)
      sent = true
    } catch (err) {
      error = errorMessage(err, "Something went wrong")
    }
    loading = false
  }
</script>

{#if !ready}
  <p class="text-ink-muted">Loading…</p>
{:else if !client}
  <p class="text-ink-muted">
    Ask whoever runs this Crumb to help you back in.
    <a class="text-primary font-bold hover:underline" href="/login">Back to sign in</a>
  </p>
{:else if invalid}
  <p class="mb-3 font-bold">That link has expired</p>
  <p class="text-ink-muted">
    <a class="text-primary font-bold hover:underline" href="/reset-password">Ask for a new one</a>
  </p>
{:else if sent}
  <p class="mb-3 font-bold">Check your email</p>
  <p class="text-ink-muted">
    If {email} has an account, a link to choose a new password is on its way.
  </p>
{:else}
  <p class="text-ink-muted mb-5">
    {token
      ? "Choose a new password. You'll be signed out everywhere else."
      : "Enter your email and we'll send you a link to choose a new password."}
  </p>
  <form class="space-y-4" onsubmit={submit}>
    {#if token}
      <div>
        <label class="label" for="password">New password</label>
        <PasswordInput
          id="password"
          bind:value={password}
          autocomplete="new-password"
          minlength={8}
          autofocus
          describedby="password-hint"
        />
        <p id="password-hint" class="text-ink-muted mt-1.5 text-sm">At least 8 characters.</p>
      </div>
    {:else}
      <div>
        <label class="label" for="email">Email</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="email"
          name="email"
          type="email"
          bind:value={email}
          autocomplete="email"
          autocapitalize="none"
          spellcheck="false"
          class="input input-lg"
          autofocus
          required
        />
      </div>
    {/if}
    {#if error}<p class="text-error text-sm" role="alert">{error}</p>{/if}
    <button type="submit" class="btn btn-primary btn-lg w-full" disabled={loading}>
      {#if loading}<LoaderCircle class="animate-spin" />{/if}
      {token ? "Change password" : "Send the link"}
    </button>
  </form>
  <p class="text-ink-muted mt-6 text-center">
    <a class="text-primary font-bold hover:underline" href="/login">Back to sign in</a>
  </p>
{/if}
