<script lang="ts">
  /**
   * A share page's actions: save the recipe (or the whole shared cookbook) into your own
   * Crumb (its Add page takes `?url=`, and a Crumb reads another Crumb's share losslessly),
   * download the Crumb file, or print a recipe.
   */
  import Download from "@lucide/svelte/icons/download"
  import Plus from "@lucide/svelte/icons/plus"
  import Printer from "@lucide/svelte/icons/printer"
  import Modal from "../components/Modal.svelte"
  import { inlineData } from "../lib/api"

  interface Data {
    share: { title: string; url: string; exportUrl: string; kind?: "recipe" | "cookbook" }
  }

  const share = inlineData<Data>()?.share ?? {
    title: document.title,
    url: location.origin + location.pathname,
    exportUrl: `${location.pathname}/crumb.json`,
  }
  const book = share.kind === "cookbook"
  const KEY = "crumb:my-crumb"

  function remembered(): string {
    try {
      return localStorage.getItem(KEY) ?? ""
    } catch {
      return ""
    }
  }

  let open = $state(false)
  let address = $state(remembered())
  let problem = $state("")
  let field: HTMLInputElement | undefined = $state()

  /** "crumb.example.com" or a full URL, as the origin of that Crumb. */
  function crumbOrigin(raw: string): string | null {
    let value = raw.trim()
    if (!value) return null
    if (!/^https?:\/\//i.test(value)) value = `https://${value}`
    try {
      const url = new URL(value)
      if (!/^https?:$/.test(url.protocol)) return null
      const host = url.hostname
      // A domain, an IP address or localhost; not a bare word
      return host.includes(".") || host.includes(":") || host === "localhost" ? url.origin : null
    } catch {
      return null
    }
  }

  function start() {
    problem = ""
    open = true
    setTimeout(() => field?.focus(), 50)
  }

  function save(e: SubmitEvent) {
    e.preventDefault()
    const origin = crumbOrigin(address)
    if (!origin) {
      problem = "That doesn't look like a web address."
      return
    }
    try {
      localStorage.setItem(KEY, origin)
    } catch {}
    address = origin
    open = false
    window.open(`${origin}/add?url=${encodeURIComponent(share.url)}`, "_blank", "noopener")
  }
</script>

<div class="space-y-3">
  <button type="button" class="btn btn-primary btn-xl w-full sm:w-auto" onclick={start}>
    <Plus /> Save to my Crumb
  </button>
  <div class="flex flex-wrap gap-2">
    <a class="btn btn-soft" href={share.exportUrl} download data-no-prerender>
      <Download /> Download for Crumb (.json)
    </a>
    {#if !book}
      <button type="button" class="btn btn-soft" onclick={() => window.print()}>
        <Printer /> Print
      </button>
    {/if}
  </div>
</div>

<Modal
  bind:open
  title="Save to my Crumb"
  description={book
    ? "Your Crumb opens with this cookbook ready to add, every recipe in it."
    : "Your Crumb opens with this recipe ready to add."}
>
  <form id="save-to-crumb" onsubmit={save} novalidate>
    <label class="label" for="crumb-address">Your Crumb address</label>
    <input
      bind:this={field}
      bind:value={address}
      id="crumb-address"
      class="input"
      type="text"
      inputmode="url"
      autocomplete="url"
      autocapitalize="off"
      spellcheck="false"
      placeholder="crumb.example.com"
      aria-invalid={problem ? "true" : undefined}
      aria-describedby="crumb-address-hint"
    />
    <p id="crumb-address-hint" class={["hint", problem && "text-error"]} aria-live="polite">
      {problem || "Remembered on this device for next time."}
    </p>
  </form>
  {#snippet footer()}
    <button type="button" class="btn btn-ghost" onclick={() => (open = false)}>Cancel</button>
    <button type="submit" form="save-to-crumb" class="btn btn-primary">Open my Crumb</button>
  {/snippet}
</Modal>
