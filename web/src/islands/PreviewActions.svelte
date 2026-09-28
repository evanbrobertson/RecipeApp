<script lang="ts">
  /**
   * A preview's actions (/preview?url=…, src/preview.rs): a recipe read from a site and not
   * saved yet. "Add to my Crumb" saves it like the Add box does (the server reuses the
   * scrape it just showed) and opens it; the other steps are the page before that: reading,
   * asking first (arriving from another site), or why it couldn't be read.
   */
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import RotateCw from "@lucide/svelte/icons/rotate-cw"
  import BookOpen from "@lucide/svelte/icons/book-open"
  import BlockedNudge from "../components/BlockedNudge.svelte"
  import { errorMessage, inlineData } from "../lib/api"
  import { importRecipe } from "../lib/importLink"
  import { flash, toast } from "../lib/toast"

  interface Data {
    preview: {
      state: "loading" | "ask" | "ready" | "failed"
      url: string
      host?: string
      title?: string
      message?: string
      /** Why it failed, when the server says: `site_blocked` is a bot check. */
      code?: string
    }
  }

  const preview = inlineData<Data>()?.preview
  const url = preview?.url ?? new URLSearchParams(location.search).get("url") ?? ""
  const host = preview?.host || "the site"
  const step = preview?.state ?? "loading"

  let saving = $state(false)
  let progress = $state("")

  /** This page again, scraping this time (a same-origin request, so the server will). */
  function read() {
    const next = new URL(location.href)
    next.searchParams.set("go", "1")
    location.replace(next)
  }

  async function add() {
    if (saving) return
    saving = true
    try {
      const saved = await importRecipe({ url }, (line) => (progress = line))
      flash(
        saved.isNew
          ? { title: "Added to your box", tone: "success" }
          : { title: "Already in your box" },
      )
      location.href = `/recipes/${saved.id}`
    } catch (e) {
      toast({ title: "Couldn't add it", description: errorMessage(e), tone: "error" })
      saving = false
      progress = ""
    }
  }
</script>

<div class="space-y-3">
  {#if step === "ready"}
    <button
      type="button"
      class="btn btn-primary btn-xl w-full sm:w-auto"
      onclick={add}
      disabled={saving}
    >
      {#if saving}<LoaderCircle class="animate-spin" /> Adding…{:else}<Plus /> Add to my Crumb{/if}
    </button>
    <p class="hint" aria-live="polite">{progress || "Not in your box yet."}</p>
  {:else if step === "ask"}
    <button type="button" class="btn btn-primary btn-xl w-full sm:w-auto" onclick={read}>
      <BookOpen /> Read it in Crumb
    </button>
  {:else if step === "failed" && preview?.code === "site_blocked"}
    <BlockedNudge {url} />
    <button type="button" class="btn btn-ghost" onclick={read}><RotateCw /> Try again</button>
  {:else if step === "failed"}
    <div class="flex flex-wrap gap-2">
      <button type="button" class="btn btn-primary" onclick={read}><RotateCw /> Try again</button>
      <a class="btn btn-soft" href="/add">Paste it into Add</a>
    </div>
  {:else}
    <button type="button" class="btn btn-primary btn-xl w-full sm:w-auto" disabled>
      <LoaderCircle class="animate-spin" /> Reading {host}…
    </button>
  {/if}
  {#if url && step !== "ready"}
    <a class="btn btn-ghost" href={url} rel="noopener noreferrer nofollow">
      <ExternalLink /> Back to {host}
    </a>
  {/if}
</div>
