<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { api, errorMessage } from "../lib/api"

  let password = $state("")
  let loading = $state(false)
  let error = $state("")

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    loading = true
    error = ""
    try {
      await api("/api/auth/login", { method: "POST", body: { password } })
      const target = new URLSearchParams(location.search).get("next") ?? "/"
      // Only allow same-origin paths ("//host" would be an open redirect)
      location.href = target.startsWith("/") && !target.startsWith("//") ? target : "/"
    } catch (err) {
      error = errorMessage(err, "Couldn't sign in")
      loading = false
    }
  }
</script>

<form class="space-y-4" onsubmit={submit}>
  <div>
    <label class="label" for="password">Password</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      id="password"
      type="password"
      bind:value={password}
      autocomplete="current-password"
      class="input input-lg"
      autofocus
      required
    />
    {#if error}<p class="text-error mt-1.5 text-sm">{error}</p>{/if}
  </div>
  <button type="submit" class="btn btn-primary btn-lg w-full" disabled={loading}>
    {#if loading}<LoaderCircle class="animate-spin" />{/if} Sign in
  </button>
</form>
