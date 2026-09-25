<script lang="ts">
  /** Home below the tile header: pick up, can't decide, the shelf, and the newest recipes. */
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import LayoutGrid from "@lucide/svelte/icons/layout-grid"
  import Shuffle from "@lucide/svelte/icons/shuffle"
  import { onMount } from "svelte"
  import Bookshelf from "../components/Bookshelf.svelte"
  import GridSkeleton from "../components/GridSkeleton.svelte"
  import OpenBook from "../components/OpenBook.svelte"
  import Photo from "../components/Photo.svelte"
  import RecipeCard from "../components/RecipeCard.svelte"
  import TryNext from "../components/TryNext.svelte"
  import { api } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import { randomHref, wireRandomLinks } from "../lib/random"
  import type { ShelfBook } from "../lib/books"
  import type { Cookbook, RecipeSource, RecipeSummary, Suggestions } from "../lib/recipe"
  import { cookStep, read, recentlyViewed, write, type Viewed } from "../lib/storage"

  interface Data {
    recipes: RecipeSummary[]
    cookbooks: Cookbook[]
    suggestions: Suggestions
    recipeCount: number
  }

  const page = pageState<Data>(async () => ({
    recipes: await api<RecipeSummary[]>("/api/recipes?limit=8"),
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
    suggestions: await api<Suggestions>("/api/suggestions"),
    recipeCount: (await api<RecipeSummary[]>("/api/recipes")).length,
  }))
  const data = $derived(page.data)
  const loading = $derived(page.loading)
  let opened = $state<ShelfBook | null>(null)
  const recent = recentlyViewed().slice(0, 3)

  // "What should I cook next?" opens in place; remember that across visits
  const NEXT_OPEN = "crumb:home:next-open"
  let nextOpen = $state(read(NEXT_OPEN, false))
  $effect(() => write(NEXT_OPEN, nextOpen))

  let cantDecide: HTMLElement | undefined = $state()
  onMount(() => {
    if (cantDecide) wireRandomLinks(cantDecide)
  })

  const DAY = 86_400_000
  function dayLabel(ms: number, now = Date.now()): string {
    const start = new Date(now).setHours(0, 0, 0, 0)
    if (ms >= start) return "Today"
    if (ms >= start - DAY) return "Yesterday"
    if (ms >= start - 6 * DAY) return new Date(ms).toLocaleDateString(undefined, { weekday: "long" })
    return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" })
  }

  const SOURCE: Record<RecipeSource, string> = {
    url: "from a link",
    text: "from pasted text",
    claude: "via Claude",
    manual: "written by you",
    import: "imported",
  }

  function freshMeta(r: RecipeSummary) {
    const when = dayLabel(Date.parse(r.createdAt))
    const how = SOURCE[r.source]
    return how ? `${when} · ${how}` : when
  }

  function progress(v: Viewed): { step: number; of: number } | null {
    const step = cookStep(v.id)
    // Not started, or on the last step (cook mode stays there after Finish)
    if (step == null || !v.steps || step <= 0 || step >= v.steps - 1) return null
    return { step: step + 1, of: v.steps }
  }

  function viewedLine(v: Viewed) {
    return v.at ? `Viewed ${dayLabel(v.at).toLowerCase()}` : "Viewed recently"
  }
</script>

<div class="space-y-7 md:space-y-9">
  <div class={["grid gap-7 md:gap-8", recent.length && "lg:grid-cols-2"]}>
    {#if recent.length}
      <section>
        <h2 class="section-title mb-3.5">Pick up where you left off</h2>
        <div class="list-card">
          {#each recent as v (v.id)}
            {@const p = progress(v)}
            <a
              href={p ? `/recipes/${v.id}/cook` : `/recipes/${v.id}`}
              class="list-row"
              data-prerender-eager
            >
              <Photo
                recipe={v}
                width={64}
                class="rounded-ctl size-16 flex-none"
                iconClass="size-6"
              />
              <span class="min-w-0 flex-1">
                <span class="line-clamp-2 text-[17px] leading-tight font-bold">{v.title}</span>
                {#if p}
                  <span class="mt-2 flex items-center gap-3">
                    <span class="progress flex-1"><span style={`width:${(p.step / p.of) * 100}%`}></span></span>
                    <span class="meta flex-none">Step {p.step} of {p.of}</span>
                  </span>
                {:else}
                  <span class="text-ink-muted mt-0.5 block text-sm">{viewedLine(v)}</span>
                {/if}
              </span>
            </a>
          {/each}
        </div>
      </section>
    {/if}

    <section bind:this={cantDecide}>
      <h2 class="section-title mb-3.5">Can't decide?</h2>
      <div class="list-card">
        <a href={randomHref()} class="list-row" data-random data-no-prerender>
          <span class="well"><Shuffle /></span>
          <span class="min-w-0 flex-1">
            <span class="block text-[17px] leading-tight font-bold">Surprise me</span>
            <span class="text-ink-muted block text-sm">One recipe, picked at random</span>
          </span>
          <ChevronRight class="text-ink-muted size-5 flex-none" />
        </a>
        <button
          type="button"
          class="list-row w-full text-left"
          aria-expanded={nextOpen}
          aria-controls="try-next"
          onclick={() => (nextOpen = !nextOpen)}
        >
          <span class="well"><LayoutGrid /></span>
          <span class="min-w-0 flex-1">
            <span class="block text-[17px] leading-tight font-bold">What should I cook next?</span>
            <span class="text-ink-muted block text-sm">Four ideas to choose from</span>
          </span>
          <ChevronRight
            class={["text-ink-muted size-5 flex-none transition", nextOpen && "rotate-90"]}
          />
        </button>
      </div>
    </section>
  </div>

  {#if nextOpen}
    <section id="try-next" aria-label="Four ideas to choose from">
      {#if data?.suggestions}
        <TryNext initial={data.suggestions} total={data.recipeCount} />
      {:else}
        <GridSkeleton count={4} />
      {/if}
    </section>
  {/if}

  {#if data?.cookbooks.length}
    <section>
      <div class="mb-3.5 flex items-baseline justify-between gap-3">
        <h2 class="section-title">The shelf</h2>
        <a href="/cookbooks" class="link">See all</a>
      </div>
      <!-- The plank runs to the card's edges and sits on its bottom -->
      <div class="bg-tint rounded-ui overflow-hidden">
        <div class="no-scrollbar overflow-x-auto pt-5">
          <Bookshelf
            books={data.cookbooks.slice(0, 14)}
            single
            pulledId={opened?.id}
            onopen={(b) => (opened = b)}
          />
        </div>
      </div>
      <OpenBook book={opened} onclose={() => (opened = null)} />
    </section>
  {/if}

  {#if loading || data?.recipes.length}
    <section>
      <div class="mb-3.5 flex items-baseline justify-between gap-3">
        <h2 class="section-title">Fresh in the box</h2>
        <a href="/recipes" class="link">See all</a>
      </div>
      {#if loading}
        <GridSkeleton count={4} />
      {:else if data}
        <div
          class="grid grid-cols-2 gap-x-3 gap-y-5 sm:grid-cols-3 sm:gap-x-4 xl:grid-cols-6"
        >
          {#each data.recipes.slice(0, 6) as recipe, i (recipe.id)}
            <div class={[i >= 4 && "hidden sm:block"]}>
              <RecipeCard
                {recipe}
                meta={freshMeta(recipe)}
                sizes="(min-width: 1280px) 11rem, (min-width: 768px) 28vw, (min-width: 640px) 30vw, 50vw"
              />
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>
