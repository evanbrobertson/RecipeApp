<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import Clock from "@lucide/svelte/icons/clock"
  import Copy from "@lucide/svelte/icons/copy"
  import Dices from "@lucide/svelte/icons/dices"
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import Flame from "@lucide/svelte/icons/flame"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Plus from "@lucide/svelte/icons/plus"
  import Printer from "@lucide/svelte/icons/printer"
  import Share from "@lucide/svelte/icons/share"
  import Snowflake from "@lucide/svelte/icons/snowflake"
  import Soup from "@lucide/svelte/icons/soup"
  import StickyNote from "@lucide/svelte/icons/sticky-note"
  import Timer from "@lucide/svelte/icons/timer"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import Users from "@lucide/svelte/icons/users"
  import EmptyState from "../components/EmptyState.svelte"
  import Menu, { type MenuItem } from "../components/Menu.svelte"
  import Modal from "../components/Modal.svelte"
  import Photo from "../components/Photo.svelte"
  import ScaleControl from "../components/ScaleControl.svelte"
  import { whenActive } from "../lib/active"
  import { api, errorMessage, pathId } from "../lib/api"
  import { bookPalette } from "../lib/books"
  import { recipeToMarkdown } from "../lib/format"
  import { cookedLine, logView, markCooked } from "../lib/history"
  import { scaleIngredient } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import {
    countItems,
    hostOf,
    kicker,
    nutritionLabels,
    type Cookbook,
    type CookStats,
    type Recipe,
  } from "../lib/recipe"
  import { randomHref, rememberRandom } from "../lib/random"
  import { forgetViewed, getScale, rememberViewed, setScale } from "../lib/storage"
  import { flash, toast } from "../lib/toast"

  interface Data {
    recipe: Recipe
    cookbooks: Cookbook[]
    inCookbooks: number[]
    cookStats?: CookStats
  }

  const id = pathId()
  const page = pageState<Data>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
    inCookbooks: await api<number[]>(`/api/recipes/${id}/cookbooks`),
  }))
  const recipe = $derived(page.data?.recipe)

  $effect(() => {
    if (recipe) {
      const r = recipe
      whenActive(() => {
        rememberViewed(r)
        logView(r.id)
      })
      document.title = `${recipe.title} · Crumb`
    }
  })

  // Landed here from Surprise me: offer another, and don't serve this one again
  const fromRandom = new URLSearchParams(location.search).get("from") === "random"
  if (fromRandom) {
    rememberRandom(id)
    const url = new URL(location.href)
    url.searchParams.delete("from")
    history.replaceState(history.state, "", url.pathname + url.search + url.hash)
  }
  let shuffleHref = $state(randomHref(id))
  const refreshShuffle = () => (shuffleHref = randomHref(id))

  const cooked = $derived(cookedLine(page.data?.cookStats))
  function cookedNow() {
    void markCooked(id, (stats) => {
      if (page.data) page.data.cookStats = stats
    })
  }

  // ─── Scaling & checklists ───
  let scale = $state(getScale(id))
  $effect(() => setScale(id, scale))
  let checked = $state(new Set<string>())
  function toggle(key: string) {
    const next = new Set(checked)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    checked = next
  }

  const scaledYield = $derived.by(() => {
    const y = recipe?.recipeYield
    if (!y || scale === 1) return y
    return scaleIngredient(y, scale)
  })

  const meta = $derived(
    recipe
      ? [
          { icon: Timer, label: "Prep", value: recipe.prepTime },
          { icon: Flame, label: "Cook", value: recipe.cookTime },
          { icon: Snowflake, label: "Extra", value: recipe.freezeTime },
          { icon: Clock, label: "Total", value: recipe.totalTime },
        ].filter((m) => m.value)
      : [],
  )
  const sourceHost = $derived(hostOf(recipe?.url))
  const ingredientCount = $derived(recipe ? countItems(recipe.ingredients) : 0)
  const sub = $derived(recipe ? kicker(recipe) : "")
  const hasHero = $derived(!!recipe?.image)

  function stepOffset(sectionIndex: number) {
    return (recipe?.instructions ?? [])
      .slice(0, sectionIndex)
      .reduce((n, s) => n + s.items.length, 0)
  }

  // ─── Actions ───
  async function copyRecipe() {
    if (!recipe) return
    try {
      await navigator.clipboard.writeText(recipeToMarkdown(recipe))
      toast({ title: "Recipe copied" })
    } catch {
      toast({ title: "Couldn't copy", tone: "error" })
    }
  }

  function shareRecipe() {
    if (!recipe) return
    void navigator.share?.({ title: recipe.title, text: recipeToMarkdown(recipe) }).catch(() => {})
  }

  let showDelete = $state(false)
  async function deleteRecipe() {
    try {
      await api(`/api/recipes/${id}`, { method: "DELETE" })
      forgetViewed([id])
      flash({ title: "Recipe deleted" })
      location.href = "/recipes"
    } catch (e) {
      toast({ title: "Couldn't delete", description: errorMessage(e), tone: "error" })
    }
  }

  // ─── Cookbooks ───
  async function toggleCookbook(bookId: number) {
    if (!page.data) return
    const inBook = page.data.inCookbooks.includes(bookId)
    try {
      if (inBook) await api(`/api/cookbooks/${bookId}/recipes/${id}`, { method: "DELETE" })
      else
        await api(`/api/cookbooks/${bookId}/recipes`, {
          method: "POST",
          body: { recipeIds: [id] },
        })
      page.data.inCookbooks = inBook
        ? page.data.inCookbooks.filter((b) => b !== bookId)
        : [...page.data.inCookbooks, bookId]
    } catch (e) {
      toast({ title: "Couldn't update cookbook", description: errorMessage(e), tone: "error" })
    }
  }

  const menu: MenuItem[][] = [
    [
      { label: "Mark as cooked", icon: ChefHat, onselect: cookedNow },
      { label: "Edit", icon: Pencil, onselect: () => (location.href = `/recipes/${id}/edit`) },
      { label: "Copy as text", icon: Copy, onselect: copyRecipe },
      ...("share" in navigator ? [{ label: "Share", icon: Share, onselect: shareRecipe }] : []),
      { label: "Print", icon: Printer, onselect: () => window.print() },
      { label: "Surprise me", icon: Dices, onselect: () => (location.href = randomHref(id)) },
    ],
    [{ label: "Delete", icon: Trash2, danger: true, onselect: () => (showDelete = true) }],
  ]
</script>

{#if page.loading}
  <div class="space-y-4">
    <div class="skeleton aspect-[4/3] w-full lg:max-w-[640px]"></div>
    <div class="skeleton h-9 w-2/3"></div>
    <div class="skeleton h-4 w-1/2"></div>
  </div>
{:else if !recipe}
  <EmptyState icon={FileQuestion} title="Recipe not found" description="It may have been deleted.">
    <a href="/recipes" class="btn btn-soft">Back to recipes</a>
  </EmptyState>
{:else}
  <article class="space-y-7">
    <!-- Hero: photo left, title and actions right on wide screens. Its box matches the
         skeleton in shell/recipe/index.astro so the card photo morphs onto it. -->
    <header
      class={[
        "grid gap-5",
        hasHero && "lg:grid-cols-[minmax(0,640px)_minmax(16rem,1fr)] lg:gap-8",
      ]}
    >
      {#if hasHero}
        <Photo
          {recipe}
          width={768}
          sizes="(min-width: 1024px) 640px, 100vw"
          alt={recipe.title}
          eager
          hero
          iconClass="size-14"
          class="rounded-ctl aspect-[4/3] w-full"
        />
      {/if}

      <div class="min-w-0 space-y-4">
        <div class="flex items-start gap-3">
          <div class="min-w-0 flex-1">
            {#if sub}<p class="kicker mb-1.5">{sub}</p>{/if}
            <h1 class="page-title text-balance">{recipe.title}</h1>
          </div>
          <Menu groups={menu} triggerClass="btn btn-outline btn-icon no-print -mt-1 flex-none" />
        </div>

        {#if recipe.author || sourceHost || cooked}
          <p class="text-ink-muted flex flex-wrap items-center gap-x-1.5 text-sm">
            {#if cooked}
              <span class="text-ink inline-flex items-center gap-1.5 font-bold">
                <ChefHat class="text-primary size-4" />{cooked}
              </span>
              {#if recipe.author || sourceHost}<span aria-hidden="true">·</span>{/if}
            {/if}
            {#if recipe.author}<span>By {recipe.author}</span>{/if}
            {#if recipe.author && sourceHost}<span aria-hidden="true">·</span>{/if}
            {#if sourceHost}
              <a
                href={recipe.url}
                target="_blank"
                rel="noopener noreferrer"
                class="link inline-flex min-h-11 items-center gap-1 text-sm"
              >
                {sourceHost}<ExternalLink class="size-3.5" />
              </a>
            {/if}
          </p>
        {/if}

        {#if recipe.description}
          <p class="text-ink-muted text-[17px] leading-relaxed text-pretty">
            {recipe.description}
          </p>
        {/if}

        {#if meta.length || scaledYield}
          <dl class="card grid grid-cols-[repeat(auto-fit,minmax(4.5rem,1fr))]">
            {#each meta as m (m.label)}
              <div class="px-3 py-3">
                <dt class="meta flex items-center gap-1.5">
                  <m.icon class="text-primary size-4" />{m.label}
                </dt>
                <dd class="mt-0.5 font-bold">{m.value}</dd>
              </div>
            {/each}
            {#if scaledYield}
              <div class="px-3 py-3">
                <dt class="meta flex items-center gap-1.5">
                  <Users class="text-primary size-4" />Serves
                </dt>
                <dd class="mt-0.5 font-bold">{scaledYield}</dd>
              </div>
            {/if}
          </dl>
        {/if}

        <!-- Cooking mode is the one main action here -->
        <div class="no-print grid grid-cols-2 gap-3 lg:grid-cols-1 2xl:grid-cols-2">
          <a href={`/recipes/${id}/cook`} class="btn btn-primary btn-xl" data-no-prerender>
            <Flame /> Cooking mode
          </a>
          <a href={`/recipes/${id}/prep`} class="btn btn-tile btn-xl"><Soup /> Mise en place</a>
          {#if fromRandom}
            <a
              href={shuffleHref}
              class="btn btn-soft btn-lg col-span-2 lg:col-span-1 2xl:col-span-2"
              data-no-prerender
              onpointerdown={refreshShuffle}
              onfocus={refreshShuffle}
            >
              <Dices /> Shuffle again
            </a>
          {/if}
        </div>
      </div>
    </header>

    <div class="grid gap-7 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] lg:gap-10">
      <section>
        <div class="lg:sticky lg:top-6">
          <div class="mb-3.5 flex min-h-11 items-center justify-between gap-2">
            <h2 class="section-title">Ingredients</h2>
            {#if ingredientCount}<ScaleControl bind:value={scale} class="no-print" />{/if}
          </div>
          {#if !ingredientCount}<p class="text-ink-muted text-sm">No ingredients listed.</p>{/if}
          <div class="space-y-5">
            {#each recipe.ingredients as section, si (si)}
              <div>
                {#if section.name}<h3 class="kicker mb-2">{section.name}</h3>{/if}
                <ul class="ingredients list-card">
                  {#each section.items as item, ii (ii)}
                    {@const key = `${si}-${ii}`}
                    {@const on = checked.has(key)}
                    <li class="relative">
                      <button
                        type="button"
                        class="hover:bg-tint/55 active:bg-tint flex min-h-12 w-full items-start gap-3 px-3.5 py-3 text-left transition-colors"
                        aria-pressed={on}
                        onclick={() => toggle(key)}
                      >
                        <span
                          class={[
                            "mt-px grid size-[22px] shrink-0 place-items-center rounded-full border-2 transition",
                            on ? "bg-tile border-tile" : "border-line-strong",
                          ]}
                        >
                          {#if on}<Check class="text-on-tile size-3.5" strokeWidth={3} />{/if}
                        </span>
                        <span
                          class={["leading-snug transition", on && "text-ink-muted line-through"]}
                        >
                          {scaleIngredient(item, scale)}
                        </span>
                      </button>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        </div>
      </section>

      <section>
        <h2 class="section-title mb-3.5 flex min-h-11 items-center">Method</h2>
        {#if !recipe.instructions.length}<p class="text-ink-muted text-sm">No steps listed.</p>{/if}
        <div class="space-y-6">
          {#each recipe.instructions as section, si (si)}
            <div>
              {#if section.name}<h3 class="kicker mb-3">{section.name}</h3>{/if}
              <ol class="space-y-5">
                {#each section.items as step, i (i)}
                  <li class="flex gap-4">
                    <span
                      class="text-primary w-9 shrink-0 text-right text-[28px] leading-none font-extrabold tabular-nums"
                      aria-hidden="true"
                    >
                      {stepOffset(si) + i + 1}
                    </span>
                    <p class="pt-0.5 text-[17px] leading-relaxed">
                      <span class="sr-only">Step {stepOffset(si) + i + 1}: </span>{step}
                    </p>
                  </li>
                {/each}
              </ol>
            </div>
          {/each}
        </div>

        {#if recipe.notes}
          <!-- The cook's own notes are the one place for handwriting -->
          <div class="card mt-8 p-5">
            <h2 class="text-primary flex items-center gap-2 font-bold">
              <StickyNote class="size-[18px]" /> Notes
            </h2>
            <p class="hand mt-2 text-[26px] leading-tight whitespace-pre-line">{recipe.notes}</p>
          </div>
        {/if}

        {#if recipe.nutrition && Object.keys(recipe.nutrition).length}
          <div class="mt-8">
            <h2 class="section-title mb-3.5">Nutrition</h2>
            <dl class="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {#each Object.entries(recipe.nutrition) as [key, value] (key)}
                <div class="card px-3.5 py-3">
                  <dt class="meta">{nutritionLabels[key] ?? key.replace(/Content$/, "")}</dt>
                  <dd class="mt-0.5 font-bold">{value}</dd>
                </div>
              {/each}
            </dl>
          </div>
        {/if}
      </section>
    </div>

    {#if page.data?.cookbooks.length}
      <section class="no-print">
        <h2 class="section-title mb-3.5">On the shelf in</h2>
        <div class="flex flex-wrap gap-2">
          {#each page.data.cookbooks as book (book.id)}
            {@const inBook = page.data.inCookbooks.includes(book.id)}
            <button
              type="button"
              class={[
                "chip h-11 border pr-3.5 pl-2.5",
                inBook
                  ? "bg-tint text-primary border-transparent"
                  : "border-line bg-paper text-ink-muted hover:text-ink",
              ]}
              aria-pressed={inBook}
              onclick={() => toggleCookbook(book.id)}
            >
              <span
                class="h-6 w-2.5 rounded-[3px_3px_0_0] shadow-[inset_0_0_0_1px_rgb(28_43_34/0.22)] dark:shadow-[inset_0_0_0_1px_rgb(239_233_218/0.25)]"
                style:background={bookPalette(book.color).cloth}
              ></span>
              {book.name}
              {#if inBook}<Check class="size-4" />{:else}<Plus class="size-4" />{/if}
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </article>
{/if}

<Modal bind:open={showDelete} title="Delete this recipe?" description="This can't be undone.">
  {#snippet footer()}
    <button type="button" class="btn btn-ghost" onclick={() => (showDelete = false)}>Cancel</button>
    <button type="button" class="btn btn-danger" onclick={deleteRecipe}>Delete</button>
  {/snippet}
</Modal>

<style>
  /* Inset dividers between ingredient rows, as in .list-row */
  .ingredients > li + li::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0.875rem;
    right: 0.875rem;
    border-top: 1px solid var(--border);
    pointer-events: none;
  }
</style>
