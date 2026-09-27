<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { onMount } from "svelte"
  import { api, errorMessage } from "../lib/api"

  /**
   * Signing in, and with accounts (AUTH_MODE=accounts) the first-run setup and sign-up.
   * With one password it's the password box it always was; /api/auth/status says which.
   */
  let { kind = "login" }: { kind?: "login" | "setup" | "signup" } = $props()

  type Status = {
    mode: "password" | "accounts"
    setupNeeded?: boolean
    setupNeedsAppPassword?: boolean
    signupOpen?: boolean
  }

  let status = $state<Status | null>(null)
  let name = $state("")
  let email = $state("")
  let password = $state("")
  let appPassword = $state("")
  let loading = $state(false)
  let error = $state("")

  const accounts = $derived(status?.mode === "accounts")
  const query = typeof location === "undefined" ? "" : location.search

  onMount(async () => {
    status = await api<Status>("/api/auth/status").catch(() => ({ mode: "password" as const }))
    // Each form only where it applies
    if (status.mode === "password" && kind !== "login") location.replace(`/login${query}`)
    else if (status.setupNeeded && kind !== "setup") location.replace(`/setup${query}`)
    else if (!status.setupNeeded && kind === "setup") location.replace(`/login${query}`)
    else if (kind === "signup" && !status.signupOpen) location.replace(`/login${query}`)
  })

  function goNext() {
    const target = new URLSearchParams(location.search).get("next") ?? "/"
    // Only allow same-origin paths ("//host" would be an open redirect)
    location.href = target.startsWith("/") && !target.startsWith("//") ? target : "/"
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    loading = true
    error = ""
    const body =
      kind === "login"
        ? accounts
          ? { email, password }
          : { password }
        : { name, email, password, ...(appPassword ? { appPassword } : {}) }
    try {
      await api(`/api/auth/${kind}`, { method: "POST", body })
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

<p class="text-ink-muted mb-5">{intro}</p>
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
