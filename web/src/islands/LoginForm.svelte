<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { onMount } from "svelte"
  import SocialButtons from "../components/SocialButtons.svelte"
  import { type Status, accounts as client, authStatus, socialError } from "../lib/account"
  import { api, errorMessage } from "../lib/api"

  /**
   * Signing in, and with accounts (AUTH_MODE=accounts or hosted) the first-run setup and
   * sign-up. With one password it's the password box it always was; /api/auth/status says
   * which.
   */
  let { kind = "login" }: { kind?: "login" | "setup" | "signup" } = $props()

  let status = $state<Status | null>(null)
  let name = $state("")
  let email = $state("")
  let password = $state("")
  let appPassword = $state("")
  let loading = $state(false)
  /** A Google or Apple sign-in that came back with an error says why. */
  let error = $state(
    socialError(
      typeof location === "undefined" ? null : new URLSearchParams(location.search).get("error"),
    ) ?? "",
  )
  /** Hosted, with email: the new account must confirm its address first. */
  let checkEmail = $state(false)

  const accounts = $derived(status?.mode === "accounts" || status?.mode === "hosted")
  const hosted = $derived(status?.mode === "hosted")
  const query = typeof location === "undefined" ? "" : location.search

  onMount(async () => {
    status = await authStatus()
    // Each form only where it applies
    if (status.mode === "password" && kind !== "login") location.replace(`/login${query}`)
    else if (status.setupNeeded && kind !== "setup") location.replace(`/setup${query}`)
    else if (!status.setupNeeded && kind === "setup") location.replace(`/login${query}`)
    else if (kind === "signup" && !status.signupOpen) location.replace(`/login${query}`)
  })

  /** Where to go once signed in: only same-origin paths ("//host" would be an open redirect). */
  function nextPath() {
    const target = new URLSearchParams(location.search).get("next") ?? "/"
    return target.startsWith("/") && !target.startsWith("//") ? target : "/"
  }

  function goNext() {
    location.href = nextPath()
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    loading = true
    error = ""
    try {
      if (kind === "login" && accounts) await client(status!.mode).signIn(email, password)
      else if (kind === "login") await api("/api/auth/login", { method: "POST", body: { password } })
      else if (kind === "signup") {
        const made = await client(status!.mode).signUp({ name, email, password })
        if (made.verify) {
          checkEmail = true
          loading = false
          return
        }
      } else {
        const body = { name, email, password, ...(appPassword ? { appPassword } : {}) }
        await api("/api/auth/setup", { method: "POST", body })
      }
      goNext()
    } catch (err) {
      error = errorMessage(err, kind === "login" ? "Couldn't sign in" : "Couldn't make the account")
      loading = false
    }
  }

  const intro = $derived(
    kind === "setup"
      ? "Make the first account. It owns the recipes already here."
      : kind === "signup"
        ? "Make an account and a recipe box of your own."
        : accounts
          ? "Sign in to open your recipe box."
          : "Enter your password to open your recipe box.",
  )
  const action = $derived(kind === "login" ? "Sign in" : "Create account")
</script>

{#if checkEmail}
  <p class="mb-3 font-bold">Check your email</p>
  <p class="text-ink-muted">
    We sent a link to {email}. Open it to confirm your address and your recipe box is ready.
  </p>
{:else}
<p class="text-ink-muted mb-5">{intro}</p>
{#if status && accounts && kind !== "setup"}
  <SocialButtons
    mode={status.mode}
    providers={status.providers}
    to={{ intent: "login", next: nextPath() }}
  />
{/if}
<form class="space-y-4" onsubmit={submit}>
  {#if kind !== "login"}
    <div>
      <label class="label" for="name">Your name</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="name"
        bind:value={name}
        autocomplete="name"
        class="input input-lg"
        maxlength="80"
        autofocus
        required
      />
    </div>
  {/if}
  {#if accounts || kind !== "login"}
    <div>
      <label class="label" for="email">Email</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="email"
        type="email"
        bind:value={email}
        autocomplete="email"
        class="input input-lg"
        autofocus={kind === "login"}
        required
      />
    </div>
  {/if}
  <div>
    <label class="label" for="password">Password</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      id="password"
      type="password"
      bind:value={password}
      autocomplete={kind === "login" ? "current-password" : "new-password"}
      minlength={kind === "login" ? undefined : 8}
      class="input input-lg"
      autofocus={kind === "login" && !accounts}
      required
    />
    {#if kind !== "login"}
      <p class="text-ink-muted mt-1.5 text-sm">At least 8 characters.</p>
    {/if}
  </div>
  {#if kind === "setup" && status?.setupNeedsAppPassword}
    <div>
      <label class="label" for="app-password">Current app password</label>
      <input
        id="app-password"
        type="password"
        bind:value={appPassword}
        autocomplete="off"
        class="input input-lg"
        required
      />
      <p class="text-ink-muted mt-1.5 text-sm">The one this box used before accounts.</p>
    </div>
  {/if}
  {#if error}<p class="text-error text-sm" role="alert">{error}</p>{/if}
  <button type="submit" class="btn btn-primary btn-lg w-full" disabled={loading}>
    {#if loading}<LoaderCircle class="animate-spin" />{/if}
    {action}
  </button>
</form>
{#if kind === "login" && hosted}
  <p class="mt-4 text-center">
    <a class="text-primary font-bold hover:underline" href="/reset-password">Forgot your password?</a>
  </p>
{/if}
{/if}
{#if kind === "login" && status?.signupOpen}
  <p class="text-ink-muted mt-6 text-center">
    New here? <a class="text-primary font-bold hover:underline" href={`/signup${query}`}
      >Create an account</a
    >
  </p>
{:else if kind === "signup"}
  <p class="text-ink-muted mt-6 text-center">
    Have an account? <a class="text-primary font-bold hover:underline" href={`/login${query}`}
      >Sign in</a
    >
  </p>
{/if}
