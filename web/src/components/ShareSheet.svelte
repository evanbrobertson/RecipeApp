<script lang="ts">
  /**
   * Share, send and download one recipe or one cookbook: its share link (made on request,
   * with the notes on or off, stopped for good with "Stop sharing"), plain text, email, print
   * (recipes), and files. A cookbook's link is live: recipes added later show up on it.
   */
  import Copy from "@lucide/svelte/icons/copy"
  import FileBraces from "@lucide/svelte/icons/file-braces"
  import FileText from "@lucide/svelte/icons/file-text"
  import Link from "@lucide/svelte/icons/link"
  import Mail from "@lucide/svelte/icons/mail"
  import Printer from "@lucide/svelte/icons/printer"
  import Share from "@lucide/svelte/icons/share"
  import Modal from "./Modal.svelte"
  import { api, errorMessage } from "../lib/api"
  import { bookToText, recipeToText } from "../lib/format"
  import type { CookbookDetail, Recipe, ShareLink } from "../lib/recipe"
  import { toast } from "../lib/toast"

  interface Props {
    open: boolean
    /** What's shared: a recipe, or else a cookbook. */
    recipe?: Recipe
    cookbook?: CookbookDetail
    /** Null (or missing) until a link is made. */
    share?: ShareLink | null
  }
  let { open = $bindable(), recipe, cookbook, share = $bindable() }: Props = $props()

  const book = $derived(!recipe)
  const title = $derived(recipe?.title ?? cookbook?.name ?? "")
  const endpoint = $derived(
    recipe ? `/api/recipes/${recipe.id}/share` : `/api/cookbooks/${cookbook?.id}/share`,
  )
  const notesHint = $derived.by(() => {
    if (recipe) return recipe.notes ? "Shown on the shared page" : "This recipe has no notes yet"
    return "Shown on each recipe's page"
  })

  const canShare = "share" in navigator
  let busy = $state(false)
  let confirmStop = $state(false)
  $effect(() => {
    if (!open) confirmStop = false
  })

  async function create() {
    busy = true
    try {
      share = await api<ShareLink>(endpoint, { method: "POST" })
    } catch (e) {
      toast({ title: "Couldn't make a link", description: errorMessage(e), tone: "error" })
    } finally {
      busy = false
    }
  }

  async function copy(text: string, done: string) {
    try {
      await navigator.clipboard.writeText(text)
      toast({ title: done })
    } catch {
      toast({ title: "Couldn't copy", tone: "error" })
    }
  }

  function shareNative() {
    if (!share) return
    void navigator.share({ title, url: share.url }).catch(() => {})
  }

  async function setNotes(include: boolean) {
    if (!share) return
    const before = share
    share = { ...share, includeNotes: include }
    try {
      share = await api<ShareLink>(endpoint, {
        method: "PATCH",
        body: { includeNotes: include },
      })
    } catch (e) {
      share = before
      toast({ title: "Couldn't change that", description: errorMessage(e), tone: "error" })
    }
  }

  async function stop() {
    busy = true
    try {
      await api(endpoint, { method: "DELETE" })
      share = null
      confirmStop = false
      toast({ title: "Stopped sharing", description: "The link no longer works." })
    } catch (e) {
      toast({ title: "Couldn't stop sharing", description: errorMessage(e), tone: "error" })
    } finally {
      busy = false
    }
  }

  function print() {
    open = false
    // After the sheet has closed, so it isn't in the printout
    setTimeout(() => window.print(), 300)
  }

  const mailto = $derived(
    share
      ? `mailto:?subject=${encodeURIComponent(title)}&body=${encodeURIComponent(share.url)}`
      : "",
  )

  function copyText() {
    if (recipe) return copy(recipeToText(recipe, share?.url), "Recipe copied")
    const titles = cookbook?.recipes.map((r) => r.title) ?? []
    return copy(bookToText(title, titles, share?.url), "Cookbook copied")
  }
</script>

<Modal bind:open title="Share" sheet>
  <div class="space-y-6">
    <section aria-labelledby="share-link-title">
      <h3 id="share-link-title" class="group-title">Share a link</h3>
      {#if !share}
        <p class="text-ink-muted text-[15px]">
          {book
            ? "Anyone with the link sees this book and every recipe in it, including ones you add later."
            : "Anyone with the link can see this recipe, not the rest of your box."}
        </p>
        {#if book}
          <p class="meta mt-1">Links to single recipes in this book open the whole book.</p>
        {/if}
        <button type="button" class="btn btn-soft mt-3" disabled={busy} onclick={create}>
          <Link /> Create link
        </button>
      {:else}
        <label class="sr-only" for="share-url">Share link</label>
        <input
          id="share-url"
          class="input text-[15px]"
          readonly
          value={share.url}
          onfocus={(e) => e.currentTarget.select()}
        />
        {#if book}
          <p class="meta mt-2">Links to single recipes in this book open the whole book.</p>
        {/if}
        <div class="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            class="btn btn-soft"
            onclick={() => copy(share!.url, "Link copied")}
          >
            <Copy /> Copy link
          </button>
          {#if canShare}
            <button type="button" class="btn btn-soft" onclick={shareNative}>
              <Share /> Share…
            </button>
          {/if}
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={share.includeNotes}
          class="mt-4 flex min-h-11 w-full items-center justify-between gap-3 text-left"
          onclick={() => setNotes(!share!.includeNotes)}
        >
          <span>
            <span class="block font-bold">Include my notes</span>
            <span class="text-ink-muted block text-sm">{notesHint}</span>
          </span>
          <span class={["switch", share.includeNotes && "on"]} aria-hidden="true"></span>
        </button>
        {#if confirmStop}
          <div class="bg-tint rounded-ctl mt-4 p-4">
            <p class="font-bold">Stop sharing?</p>
            <p class="text-ink-muted mt-1 text-sm">
              The link stops working for everyone. A new link can be made later.
            </p>
            <div class="mt-3 flex flex-wrap justify-end gap-2">
              <button type="button" class="btn btn-ghost" onclick={() => (confirmStop = false)}>
                Keep sharing
              </button>
              <button type="button" class="btn btn-danger" disabled={busy} onclick={stop}>
                Stop sharing
              </button>
            </div>
          </div>
        {:else}
          <button
            type="button"
            class="btn btn-link text-error mt-3 min-h-11"
            onclick={() => (confirmStop = true)}
          >
            Stop sharing
          </button>
        {/if}
      {/if}
    </section>

    <section aria-labelledby="share-send-title">
      <h3 id="share-send-title" class="group-title">Send</h3>
      <div class="list-card">
        <button
          type="button"
          class="list-row settings-row w-full text-left"
          onclick={copyText}
        >
          <Copy class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Copy as text</span>
        </button>
        {#if share}
          <a class="list-row settings-row" href={mailto} data-no-prerender>
            <Mail class="settings-icon" />
            <span class="min-w-0 flex-1 font-bold">Email</span>
          </a>
        {/if}
        {#if recipe}
          <button type="button" class="list-row settings-row w-full text-left" onclick={print}>
            <Printer class="settings-icon" />
            <span class="min-w-0 flex-1 font-bold">Print or save as PDF</span>
          </button>
        {/if}
      </div>
    </section>

    <section aria-labelledby="share-download-title">
      <h3 id="share-download-title" class="group-title">Download</h3>
      <div class="list-card">
        <!-- Plain download links: the server names the file after the recipe or book -->
        <a
          class="list-row settings-row"
          href={recipe
            ? `/api/recipes/${recipe.id}/export?format=json`
            : `/api/cookbooks/${cookbook?.id}/export`}
          download
          data-no-prerender
        >
          <FileBraces class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">For another Crumb (.json)</span>
        </a>
        {#if recipe}
          <a
            class="list-row settings-row"
            href={`/api/recipes/${recipe.id}/export?format=md`}
            download
            data-no-prerender
          >
            <FileText class="settings-icon" />
            <span class="min-w-0 flex-1 font-bold">Markdown (.md)</span>
          </a>
        {/if}
      </div>
    </section>
  </div>
</Modal>

<style>
  .group-title {
    margin-bottom: 0.625rem;
    font-size: 0.8125rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  /* On/off track: tile when on */
  .switch {
    position: relative;
    flex: none;
    width: 2.75rem;
    height: 1.625rem;
    border-radius: 999px;
    background: var(--border-accented);
    transition: background-color 0.15s;
  }
  .switch::after {
    content: "";
    position: absolute;
    top: 0.1875rem;
    left: 0.1875rem;
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 999px;
    background: var(--paper);
    transition: transform 0.15s;
  }
  .switch.on {
    background: var(--tile);
  }
  .switch.on::after {
    transform: translateX(1.125rem);
  }
</style>
