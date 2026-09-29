<script lang="ts">
  /**
   * A preview's actions (/preview?url=…, src/preview.rs): a recipe read from a site and not
   * saved yet. "Add to my Crumb" saves it like the Add box does (the server reuses the
   * scrape it just showed) and opens it; the other steps are the page before that: reading,
   * asking first (arriving from another site), or why it couldn't be read.
   *
   * Opened by the extension (`via=extension`), the page first waits for the recipe the
   * extension read in the cook's browser (`crumb:page`), which gets past a site's bot check that
   * turns the server away. It posts that to /api/preview and reloads to show it; with nothing
   * handed over, or none in a few seconds, the reload reads the site as usual.
   */
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import RotateCw from "@lucide/svelte/icons/rotate-cw"
  import BookOpen from "@lucide/svelte/icons/book-open"
  import BlockedNudge from "../components/BlockedNudge.svelte"
  import { api, errorMessage, inlineData } from "../lib/api"
  import { importRecipe } from "../lib/importLink"
  import { flash, toast } from "../lib/toast"

  interface Data {
    preview: {
      state: "loading" | "handover" | "ask" | "ready" | "failed"
      url: string
      host?: string
      title?: string
      message?: string
      /** Why it failed, when the server says: `site_blocked` is a bot check, `site_terms` the site's terms. */
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

  /** How long the extension has to hand the recipe over before the site is read instead. */
  const HANDOVER_WAIT_MS = 8000

  $effect(() => {
    if (step !== "handover") return
    let done = false
    const finish = () => {
      if (done) return false
      done = true
      clearTimeout(giveUp)
      return true
    }
    const giveUp = setTimeout(() => finish() && read(), HANDOVER_WAIT_MS)
    const onMessage = async (e: MessageEvent) => {
      const data = e.data as { type?: string; page?: unknown } | null
      if (e.source !== window || e.origin !== location.origin || data?.type !== "crumb:page") return
      if (!finish()) return
      if (data.page && typeof data.page === "object") {
        // Whatever goes wrong, the reload reads the site itself
        await api("/api/preview", { method: "POST", body: { url, page: data.page } }).catch(
          () => {},
        )
      }
      read()
    }
    addEventListener("message", onMessage)
    postMessage({ type: "crumb:want-page" }, location.origin)
    return () => {
      clearTimeout(giveUp)
      removeEventListener("message", onMessage)
    }
  })

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
  {:else if step === "failed" && preview?.code === "site_terms"}
    <BlockedNudge {url} why="terms" />
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
