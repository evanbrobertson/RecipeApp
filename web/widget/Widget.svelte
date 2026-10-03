<script lang="ts">
  /**
   * Crumb's shelf inside ChatGPT: the cookbooks as books, one book's recipes, or the card of a
   * recipe that was just saved or looked up. Read-only; ChatGPT does the reorganising when the
   * cook asks it in the chat.
   */
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import CookingPot from "@lucide/svelte/icons/cooking-pot"
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import Maximize2 from "@lucide/svelte/icons/maximize-2"
  import { onMount } from "svelte"
  import Bookshelf from "../src/components/Bookshelf.svelte"
  import type { ShelfBook } from "../src/lib/books"
  import { callTool, openLink, say, showFullscreen, start, type ToolResult } from "./bridge"

  interface Card {
    id: number
    title: string
    link: string
    totalTime?: string | null
    category?: string | null
    description?: string | null
    ingredients?: number
    steps?: number
  }
  interface BookInfo extends ShelfBook {
    link?: string
  }
  type View =
    | { view: "shelf"; books: ShelfBook[] }
    | { view: "book"; book: BookInfo; recipes: Card[] }
    | { view: "recipe" | "saved"; isNew?: boolean; recipe: Card }

  let current = $state<View | null>(null)
  let shelf = $state<ShelfBook[] | null>(null)
  let photos = $state<Record<string, string>>({})
  let origin = $state("")
  let busy = $state(false)
  let failed = $state(false)

  function take(result: ToolResult) {
    const data = result.structuredContent as View | undefined
    if (!data || typeof data.view !== "string") return
    const meta = result._meta as { photos?: Record<string, string>; origin?: string } | undefined
    if (meta?.origin) origin = meta.origin
    if (meta?.photos) photos = { ...photos, ...meta.photos }
    if (data.view === "shelf") shelf = data.books
    current = data
    failed = false
  }

  onMount(() => {
    start({
      onResult: take,
      onContext: (c) => {
        if (c.theme) document.documentElement.classList.toggle("dark", c.theme === "dark")
      },
    }).catch(() => (failed = true))
  })

  async function openBook(book: ShelfBook) {
    busy = true
    try {
      const result = await callTool("get_cookbook", { cookbook: book.id })
      if (result.isError) failed = true
      else take(result)
    } catch {
      failed = true
    } finally {
      busy = false
    }
  }

  function back() {
    if (shelf) current = { view: "shelf", books: shelf }
  }

  const meta = (c: Card) => [c.totalTime, c.category].filter(Boolean).join(" · ")
</script>

{#snippet picture(c: Card, size: string)}
  {#if photos[c.id]}
    <img
      src={photos[c.id]}
      alt=""
      loading="lazy"
      class={["rounded-ctl bg-tint object-cover", size]}
    />
  {:else}
    <div class={["rounded-ctl bg-tint text-ink-dim grid place-items-center", size]}>
      <CookingPot class="size-6" />
    </div>
  {/if}
{/snippet}

<main class="mx-auto max-w-3xl p-3">
  {#if failed}
    <p class="hint p-4">Couldn't load this from Crumb. Ask ChatGPT to try again.</p>
  {:else if !current}
    <div class="h-40 animate-pulse rounded-ui bg-tint"></div>
  {:else if current.view === "shelf"}
    <div class="mb-2 flex items-center justify-between gap-3 px-1">
      <h1 class="font-serif text-2xl">Your shelf</h1>
      <button type="button" class="btn btn-sm btn-tile" onclick={showFullscreen}>
        <Maximize2 /> Bigger
      </button>
    </div>
    {#if current.books.length}
      <div class:opacity-60={busy} class="rounded-ui bg-tint p-3">
        <Bookshelf books={current.books} onopen={openBook} />
      </div>
    {:else}
      <p class="hint p-4">No cookbooks yet. Ask ChatGPT to sort your recipes into some.</p>
    {/if}
  {:else if current.view === "book"}
    <div class="mb-3 flex items-center gap-2 px-1">
      {#if shelf}
        <button type="button" class="btn btn-sm btn-icon btn-tile" aria-label="Back to the shelf" onclick={back}>
          <ArrowLeft />
        </button>
      {/if}
      <div class="min-w-0 flex-1">
        <h1 class="truncate font-serif text-2xl">{current.book.name}</h1>
        <p class="hint">{current.book.recipeCount} recipes</p>
      </div>
      {#if current.book.link}
        <button type="button" class="btn btn-sm btn-tile" onclick={() => openLink(current.book.link!)}>
          <ExternalLink /> Crumb
        </button>
      {/if}
    </div>
    {#if current.book.description}
      <p class="mb-3 px-1 text-ink-muted">{current.book.description}</p>
    {/if}
    <ul class="grid gap-2">
      {#each current.recipes as r (r.id)}
        <li>
          <button
            type="button"
            class="flex w-full items-center gap-3 rounded-ui border border-line bg-paper p-2 text-left"
            onclick={() => openLink(r.link)}
          >
            {@render picture(r, "size-14 flex-none")}
            <span class="min-w-0 flex-1">
              <span class="block truncate font-bold">{r.title}</span>
              {#if meta(r)}<span class="hint block truncate">{meta(r)}</span>{/if}
            </span>
          </button>
        </li>
      {:else}
        <li class="hint p-4">Nothing in this book yet.</li>
      {/each}
    </ul>
  {:else}
    {@const r = current.recipe}
    <article class="overflow-hidden rounded-ui border border-line bg-paper">
      {#if photos[r.id]}
        <img src={photos[r.id]} alt="" class="h-40 w-full object-cover" />
      {/if}
      <div class="grid gap-2 p-4">
        {#if current.view === "saved"}
          <p class="text-primary text-sm font-bold">
            {current.isNew === false ? "Already in Crumb" : "Saved to Crumb"}
          </p>
        {/if}
        <h1 class="font-serif text-2xl">{r.title}</h1>
        {#if meta(r)}<p class="hint">{meta(r)}</p>{/if}
        {#if r.description}<p class="text-ink-muted">{r.description}</p>{/if}
        {#if r.ingredients || r.steps}
          <p class="hint">{r.ingredients ?? 0} ingredients · {r.steps ?? 0} steps</p>
        {/if}
        <div class="mt-1 flex flex-wrap gap-2">
          <button type="button" class="btn btn-sm btn-primary" onclick={() => openLink(r.link)}>
            <ExternalLink /> Open in Crumb
          </button>
          <button
            type="button"
            class="btn btn-sm btn-soft"
            onclick={() => say(`Tell me about ${r.title}`)}
          >
            Talk about it
          </button>
        </div>
      </div>
    </article>
  {/if}
</main>
