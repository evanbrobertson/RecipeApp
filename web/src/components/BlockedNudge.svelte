<script lang="ts">
  /**
   * Shown instead of an error when a recipe site's bot check turned Crumb's server away
   * (`site_blocked`): the browser extension reads the page from the cook's own browser, which
   * is already past the check. The main action opens the page so the cook can click the
   * extension's toolbar button there (it reads the recipe and hands it to Crumb), else says
   * where to get it; on a phone, which has no extension to use, pasting
   * the recipe leads.
   *
   * `why="terms"` is the same nudge for a site whose terms of service ask for no automated
   * copying (`site_terms`): Crumb never asked the site at all, so it says whose terms and
   * points at the same way out.
   */
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste"
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import Puzzle from "@lucide/svelte/icons/puzzle"
  import ScrollText from "@lucide/svelte/icons/scroll-text"
  import ShieldCheck from "@lucide/svelte/icons/shield-check"
  import X from "@lucide/svelte/icons/x"
  import { extensionInstalled, extensionPossible, extensionUrl } from "../lib/extension"

  interface Props {
    /** The recipe page that couldn't be read. */
    url: string
    /** Switches the caller to pasting the recipe; without it, `pasteHref` is linked. */
    onpaste?: () => void
    pasteHref?: string
    ondismiss?: () => void
    /** `bot`: the site's bot check turned Crumb away. `terms`: its terms ask us not to fetch. */
    why?: "bot" | "terms"
  }
  let { url, onpaste, pasteHref = "/add", ondismiss, why = "bot" }: Props = $props()
  const terms = $derived(why === "terms")
  const site = $derived.by(() => {
    try {
      return new URL(url).hostname.replace(/^www\./, "")
    } catch {
      return "This site"
    }
  })

  // The extension marks the page a moment after it loads, so this can flip while it's shown
  let installed = $state(extensionInstalled())
  const possible = $derived(installed || extensionPossible())
  let opened = $state(false)

  $effect(() => {
    installed = extensionInstalled()
    const watch = new MutationObserver(() => (installed = extensionInstalled()))
    watch.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-crumb-extension"],
    })
    return () => watch.disconnect()
  })
</script>

<div
  class="card text-ink relative flex flex-col gap-3 p-4 text-left"
  role="status"
  aria-live="polite"
>
  <div class="flex items-start gap-3">
    <span class="well" aria-hidden="true">
      {#if terms}<ScrollText />{:else}<ShieldCheck />{/if}
    </span>
    <div class="min-w-0 flex-1 pr-8">
      <h2 class="text-[17px] leading-snug font-bold">
        {terms ? "Their terms ask us not to" : "This site wants a human"}
      </h2>
      <p class="text-ink-muted mt-1 text-[15px]">
        {#if terms}
          {#if !possible}
            {site}'s terms don't allow automated copying, so Crumb didn't fetch it. Paste the
            recipe text instead. On a computer, the Crumb extension can read it from your own
            browser.
          {:else if opened}
            Opened in a new tab. Click Crumb in your browser's toolbar there, and the extension
            reads the recipe from that page and opens it in Crumb, ready to add.
          {:else if installed}
            {site}'s terms don't allow automated copying, so Crumb didn't fetch it. Open it on
            their site, then click Crumb in your toolbar: the extension reads the recipe from your
            own browser.
          {:else}
            {site}'s terms don't allow automated copying, so Crumb didn't fetch it. The Crumb
            extension can read it from your own browser instead.
          {/if}
        {:else if !possible}
          It asked for a human check, so Crumb couldn't read it from here. Paste the recipe text
          instead. On a computer, the Crumb extension can read pages like this from your browser.
        {:else if opened}
          Opened in a new tab. Click Crumb in your browser's toolbar there, and the extension
          reads the recipe from that page and opens it in Crumb, ready to add.
        {:else if installed}
          It asked for a human check, so Crumb couldn't read it from here. Open the recipe, then
          click Crumb in your toolbar: the extension reads it from your browser, which is already
          past the check.
        {:else}
          It asked for a human check, so Crumb couldn't read it from here. The Crumb extension can
          read it from your browser, which is already past the check.
        {/if}
      </p>
    </div>
    {#if ondismiss}
      <button
        type="button"
        class="btn btn-ghost btn-icon btn-sm absolute top-2 right-2"
        aria-label="Dismiss"
        onclick={ondismiss}
      >
        <X />
      </button>
    {/if}
  </div>

  <div class="flex flex-wrap gap-2">
    {#snippet paste(primary: boolean)}
      {#if onpaste}
        <button
          type="button"
          class={["btn", primary ? "btn-primary" : "btn-soft"]}
          onclick={onpaste}
        >
          <ClipboardPaste /> Paste the text instead
        </button>
      {:else}
        <a class={["btn", primary ? "btn-primary" : "btn-soft"]} href={pasteHref}>
          <ClipboardPaste /> Paste the text instead
        </a>
      {/if}
    {/snippet}

    {#if !possible}
      {@render paste(true)}
    {:else}
      {#if installed}
        <a
          class="btn btn-primary"
          href={url}
          target="_blank"
          rel="noopener noreferrer nofollow"
          onclick={() => (opened = true)}
        >
          <ExternalLink /> Read it with the extension
        </a>
      {:else}
        <a class="btn btn-primary" href={extensionUrl()} target="_blank" rel="noopener">
          <Puzzle /> Read it with the extension
        </a>
      {/if}
      {@render paste(false)}
    {/if}
  </div>
  {#if possible && !installed}
    <p class="text-ink-muted text-[13px]">
      Get the extension, then open the recipe page and click Crumb in your toolbar.
    </p>
  {/if}
</div>
