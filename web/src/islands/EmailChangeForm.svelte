<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { followEmailLink } from "../lib/account"
  import { errorMessage } from "../lib/api"

  /**
   * Hosted only: where a change-email link lands, with its token in the fragment. `c.` links
   * confirm a new address; `r.` links (sent to the old one) put it back and sign every device
   * out. Nothing happens until the button is pressed, so a mail scanner opening the link
   * changes nothing.
   */
  const token = typeof location === "undefined" ? "" : location.hash.slice(1)
  const revert = token.startsWith("r.")

  let loading = $state(false)
  let error = $state("")
  let done = $state<{ done: "changed" | "reverted"; email: string } | null>(null)

  // The token stays out of history and anything copied from the address bar
  if (token) history.replaceState(null, "", location.pathname)

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    loading = true
    error = ""
    try {
      done = await followEmailLink(token)
    } catch (err) {
      error = errorMessage(err, "Something went wrong")
    }
    loading = false
  }
</script>

{#if !token}
  <p class="mb-3 font-bold">That link is incomplete</p>
  <p class="text-ink-muted">Open it straight from the email, or ask for a new one.</p>
{:else if done?.done === "changed"}
  <p class="mb-3 font-bold">Your email is now {done.email}</p>
  <p class="text-ink-muted">Sign in with it from now on.</p>
  <a class="btn btn-primary btn-lg mt-6 w-full" href="/">Open Crumb</a>
{:else if done}
  <p class="mb-3 font-bold">Your email is {done.email} again</p>
  <p class="text-ink-muted">
    Every device was signed out. Choose a new password now, in case someone else knows it.
  </p>
  <a class="btn btn-primary btn-lg mt-6 w-full" href="/reset-password">Reset your password</a>
{:else}
  <p class="mb-3 font-bold">{revert ? "Put your email back?" : "Confirm your new email"}</p>
  <p class="text-ink-muted mb-5">
    {revert
      ? "Your Crumb account goes back to this address, and every device is signed out, whoever changed it."
      : "Your Crumb account will use this address from now on."}
  </p>
  <form class="space-y-4" onsubmit={submit}>
    {#if error}<p class="text-error text-sm" role="alert">{error}</p>{/if}
    <button type="submit" class="btn btn-primary btn-lg w-full" disabled={loading}>
      {#if loading}<LoaderCircle class="animate-spin" />{/if}
      {revert ? "Put it back" : "Use this email"}
    </button>
  </form>
{/if}
<p class="text-ink-muted mt-6 text-center">
  <a class="text-primary font-bold hover:underline" href="/login">Back to sign in</a>
</p>
