<script lang="ts">
  import Eye from "@lucide/svelte/icons/eye"
  import EyeOff from "@lucide/svelte/icons/eye-off"

  /** A password field with a show/hide button that stays in the tab order, right after it. */
  let {
    value = $bindable(""),
    id,
    name = "password",
    autocomplete,
    minlength,
    autofocus = false,
    required = true,
    describedby,
    large = true,
  }: {
    value?: string
    id: string
    name?: string
    autocomplete: "current-password" | "new-password" | "off"
    minlength?: number
    autofocus?: boolean
    required?: boolean
    describedby?: string
    /** The tall field the sign-in pages use; off for the account page's smaller ones. */
    large?: boolean
  } = $props()

  let shown = $state(false)
</script>

<div class="relative">
  <!-- svelte-ignore a11y_autofocus -->
  <input
    {id}
    {name}
    type={shown ? "text" : "password"}
    bind:value
    {autocomplete}
    {minlength}
    {autofocus}
    {required}
    aria-describedby={describedby}
    autocapitalize="none"
    autocorrect="off"
    spellcheck="false"
    class={["input pr-12", large && "input-lg"]}
  />
  <button
    type="button"
    class="text-ink-muted hover:text-primary focus-visible:outline-primary absolute inset-y-0 right-0 flex w-12 items-center justify-center rounded-r-ctl focus-visible:outline-2 focus-visible:-outline-offset-2"
    aria-label={shown ? "Hide password" : "Show password"}
    aria-pressed={shown}
    aria-controls={id}
    onclick={() => (shown = !shown)}
  >
    {#if shown}<EyeOff size={20} aria-hidden="true" />{:else}<Eye size={20} aria-hidden="true" />{/if}
  </button>
</div>
