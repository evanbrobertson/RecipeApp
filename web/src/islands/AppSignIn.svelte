<script lang="ts">
  import { type Provider, type Status, authStatus, providerNames } from "../lib/account"
  import SocialButtons from "../components/SocialButtons.svelte"

  /**
   * The Crumb app's Google or Apple sign-in, in the browser: the app opened this page with its
   * sign-in's `id`, and once signed in the server hands the session back to the app
   * (src/app_sign_in.rs).
   */
  const params = new URLSearchParams(location.search)
  const id = params.get("id") ?? ""
  const asked = params.get("provider")
  const provider = asked === "google" || asked === "apple" ? (asked as Provider) : null

  let status = $state<Status | null>(null)
  authStatus().then((s) => (status = s))

  const offered = $derived(
    status && status.mode !== "password" && provider && status.providers?.includes(provider),
  )
</script>

{#if !status}
  <p class="text-ink-muted">Loading…</p>
{:else if !offered || !/^[\w-]{1,64}$/.test(id)}
  <div class="space-y-2">
    <h2 class="text-lg font-bold">This sign-in link doesn't work</h2>
    <p class="text-ink-muted">Go back to the Crumb app and try signing in again.</p>
  </div>
{:else}
  <div class="space-y-5">
    <div class="space-y-1">
      <h2 class="text-lg font-bold">Sign in to the Crumb app</h2>
      <p class="text-ink-muted">
        Continue with {providerNames[provider!]}, and you'll be back in the app.
      </p>
    </div>
    <SocialButtons
      mode={status.mode}
      providers={[provider!]}
      to={{ intent: "login", next: `/api/auth/app/done?id=${id}` }}
      divider={false}
    />
  </div>
{/if}
