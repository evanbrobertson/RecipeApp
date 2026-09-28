<script lang="ts">
  /**
   * Shown instead of an error when a recipe site's bot check turned Crumb's server away
   * (`site_blocked`): the browser extension reads the page from the cook's own browser, which
   * is already past the check. The main action opens the page for the extension if it's
   * installed, else says where to get it; on a phone, which has no extension to use, pasting
   * the recipe leads.
   */
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste"
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import Puzzle from "@lucide/svelte/icons/puzzle"
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
  }
  let { url, onpaste, pasteHref = "/add", ondismiss }: Props = $props()

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
    <span class="well" aria-hidden="true"><ShieldCheck /></span>
    <div class="min-w-0 flex-1 pr-8">
      <h2 class="text-[17px] leading-snug font-bold">This site wants a human</h2>
      <p class="text-ink-muted mt-1 text-[15px]">
        {#if !possible}
          It asked for a human check, so Crumb couldn't read it from here. Paste the recipe text
          instead. On a computer, the Crumb extension can read pages like this from your browser.
        {:else if opened}
          Opened in a new tab. Click Crumb in your browser's toolbar there, and the recipe comes
          back here, ready to add.
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
