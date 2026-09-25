<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import Clock from "@lucide/svelte/icons/clock"
  import Copy from "@lucide/svelte/icons/copy"
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
  import ScaleControl from "../components/ScaleControl.svelte"
  import { whenActive } from "../lib/active"
  import { api, errorMessage, pathId } from "../lib/api"
  import { bookPalette } from "../lib/books"
  import { recipeToMarkdown } from "../lib/format"
  import { scaleIngredient } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import {
    countItems,
    hostOf,
    kicker,
    nutritionLabels,
    type Cookbook,
    type Recipe,
  } from "../lib/recipe"
  import { forgetViewed, getScale, rememberViewed, setScale } from "../lib/storage"
  import { flash, toast } from "../lib/toast"

  interface Data {
    recipe: Recipe
    cookbooks: Cookbook[]
    inCookbooks: number[]
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
      whenActive(() => rememberViewed(r))
      document.title = `${recipe.title} · Crumb`
    }
  })

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
  let imageFailed = $state(false)
  const hasHero = $derived(!!recipe?.image && !imageFailed)

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
      { label: "Edit", icon: Pencil, onselect: () => (location.href = `/recipes/${id}/edit`) },
      { label: "Copy as text", icon: Copy, onselect: copyRecipe },
      ...("share" in navigator ? [{ label: "Share", icon: Share, onselect: shareRecipe }] : []),
      { label: "Print", icon: Printer, onselect: () => window.print() },
    ],
    [{ label: "Delete", icon: Trash2, danger: true, onselect: () => (showDelete = true) }],
  ]
</script>

{#if page.loading}
  <div class="mx-auto max-w-4xl space-y-4">
    <div class="skeleton aspect-[16/8] w-full"></div>
    <div class="skeleton h-10 w-2/3"></div>
    <div class="skeleton h-4 w-1/2"></div>
  </div>
{:else if !recipe}
  <EmptyState icon={FileQuestion} title="Recipe not found" description="It may have been deleted.">
    <a href="/recipes" class="btn btn-soft">Back to recipes</a>
  </EmptyState>
{:else}
  <article class="mx-auto max-w-4xl">
    <!-- Hero -->
    <div class="relative mb-6">
      {#if hasHero}
        <div class="rounded-ui relative aspect-[16/10] overflow-hidden shadow-sm sm:aspect-[16/8]">
          <img
            src={recipe.image}
            alt={recipe.title}
            referrerpolicy="no-referrer"
            decoding="async"
            fetchpriority="high"
            class="size-full object-cover"
            onerror={() => (imageFailed = true)}
          />
          <div
            class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/10 to-transparent"
          ></div>
          <div class="absolute inset-x-0 bottom-0 p-5 text-white sm:p-8">
            {#if sub}<p class="text-sm font-medium opacity-90">{sub}</p>{/if}
            <h1 class="font-serif text-3xl leading-tight font-semibold text-balance sm:text-5xl">
              {recipe.title}
            </h1>
          </div>
        </div>
      {:else}
        <div class="pt-2 pr-12">
          {#if sub}<p class="text-primary text-sm font-semibold">{sub}</p>{/if}
          <h1 class="font-serif text-4xl leading-tight font-semibold text-balance sm:text-5xl">
            {recipe.title}
          </h1>
        </div>
      {/if}
      <div class={["no-print absolute", hasHero ? "top-3 right-3" : "top-2 right-0"]}>
        <Menu
          groups={menu}
          triggerClass={hasHero
            ? "btn btn-icon bg-canvas/90 text-ink shadow-sm backdrop-blur"
            : "btn btn-outline btn-icon"}
        />
      </div>
    </div>

    {#if recipe.author || sourceHost}
      <p class="text-ink-muted -mt-2 mb-4 text-sm">
        {#if recipe.author}<span>By {recipe.author}</span>{/if}
        {#if recipe.author && sourceHost}<span> · </span>{/if}
        {#if sourceHost}
          <a
            href={recipe.url}
            target="_blank"
            rel="noopener noreferrer"
            class="hover:text-primary inline-flex items-center gap-1 underline-offset-2 hover:underline"
          >
            {sourceHost}<ExternalLink class="size-3" />
          </a>
        {/if}
      </p>
    {/if}

    {#if recipe.description}
      <p class="text-ink-muted mb-6 text-lg text-pretty">{recipe.description}</p>
    {/if}

    <!-- Primary actions -->
    <div class="no-print mb-6 grid grid-cols-2 gap-3 sm:flex">
      <a href={`/recipes/${id}/cook`} class="btn btn-primary btn-xl" data-no-prerender>
        <Flame /> Start cooking
      </a>
      <a href={`/recipes/${id}/prep`} class="btn btn-soft btn-xl"><Soup /> Mise en place</a>
    </div>

    {#if meta.length || scaledYield}
      <div class="border-line mb-8 flex flex-wrap items-center gap-x-6 gap-y-3 border-y py-4">
        {#each meta as m (m.label)}
          <div class="flex items-center gap-2 text-sm">
            <m.icon class="text-primary size-4" />
            <span class="text-ink-muted">{m.label}</span>
            <span class="font-semibold">{m.value}</span>
          </div>
        {/each}
        {#if scaledYield}
          <div class="flex items-center gap-2 text-sm">
            <Users class="text-primary size-4" />
            <span class="font-semibold">{scaledYield}</span>
          </div>
        {/if}
      </div>
    {/if}

    <div class="grid gap-10 md:grid-cols-5">
      <section class="md:col-span-2">
        <div class="md:sticky md:top-20">
          <div class="mb-3 flex items-center justify-between gap-2">
            <h2 class="font-serif text-2xl font-semibold">Ingredients</h2>
            {#if ingredientCount}<ScaleControl bind:value={scale} class="no-print" />{/if}
          </div>
          {#if !ingredientCount}<p class="text-ink-muted text-sm">No ingredients listed.</p>{/if}
          {#each recipe.ingredients as section, si (si)}
            <div class={si > 0 ? "mt-5" : ""}>
              {#if section.name}<h3 class="kicker mb-1">{section.name}</h3>{/if}
              <ul>
                {#each section.items as item, ii (ii)}
                  {@const key = `${si}-${ii}`}
                  <li>
                    <button
                      type="button"
                      class="hover:bg-raised/60 rounded-ui flex w-full items-start gap-3 px-2 py-2.5 text-left transition"
                      aria-pressed={checked.has(key)}
                      onclick={() => toggle(key)}
                    >
                      <span
                        class={[
                          "mt-0.5 grid size-5 shrink-0 place-items-center rounded-full border-2 transition",
                          checked.has(key) ? "bg-primary border-primary" : "border-line-strong",
                        ]}
                      >
                        {#if checked.has(key)}<Check class="size-3.5 text-white" />{/if}
                      </span>
                      <span
                        class={[
                          "leading-snug transition",
                          checked.has(key) && "text-ink-dim line-through",
                        ]}
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
      </section>

      <section class="md:col-span-3">
        <h2 class="mb-3 font-serif text-2xl font-semibold">Method</h2>
        {#if !recipe.instructions.length}<p class="text-ink-muted text-sm">No steps listed.</p>{/if}
        {#each recipe.instructions as section, si (si)}
          <div class={si > 0 ? "mt-6" : ""}>
            {#if section.name}<h3 class="kicker mb-2">{section.name}</h3>{/if}
            <ol class="space-y-4">
              {#each section.items as step, i (i)}
                <li class="flex gap-4">
                  <span
                    class="bg-primary/10 text-primary grid size-8 shrink-0 place-items-center rounded-full font-serif font-semibold"
                  >
                    {stepOffset(si) + i + 1}
                  </span>
                  <p class="pt-1 text-[1.05rem] leading-relaxed">{step}</p>
                </li>
              {/each}
            </ol>
          </div>
        {/each}

        {#if recipe.notes}
          <div class="rounded-ui mt-8 bg-amber-50 p-5 dark:bg-amber-950/30">
            <h2
              class="mb-1 flex items-center gap-2 font-semibold text-amber-800 dark:text-amber-300"
            >
              <StickyNote class="size-4" /> Notes
            </h2>
            <p class="text-sm whitespace-pre-line text-amber-900/90 dark:text-amber-100/80">
              {recipe.notes}
            </p>
          </div>
        {/if}

        {#if recipe.nutrition && Object.keys(recipe.nutrition).length}
          <div class="mt-8">
            <h2 class="mb-3 font-semibold">Nutrition</h2>
            <dl class="grid grid-cols-2 gap-2 sm:grid-cols-3">
              {#each Object.entries(recipe.nutrition) as [key, value] (key)}
                <div class="card px-3 py-2">
                  <dt class="text-ink-muted text-xs">
                    {nutritionLabels[key] ?? key.replace(/Content$/, "")}
                  </dt>
                  <dd class="text-sm font-semibold">{value}</dd>
                </div>
              {/each}
            </dl>
          </div>
        {/if}
      </section>
    </div>

    {#if page.data?.cookbooks.length}
      <section class="border-line no-print mt-12 border-t pt-6">
        <h2 class="text-ink-muted mb-3 text-sm font-semibold">On the shelf in</h2>
        <div class="flex flex-wrap gap-2">
          {#each page.data.cookbooks as book (book.id)}
            {@const inBook = page.data.inCookbooks.includes(book.id)}
            <button
              type="button"
              class={[
                "rounded-ui flex h-9 items-center gap-2 border pr-3 pl-1.5 text-sm font-medium transition",
                inBook
                  ? "bg-raised border-transparent"
                  : "border-line text-ink-muted hover:text-ink",
              ]}
              aria-pressed={inBook}
              onclick={() => toggleCookbook(book.id)}
            >
              <span class="h-6 w-2.5 rounded-sm" style:background={bookPalette(book.color).cloth}
              ></span>
              {book.name}
              {#if inBook}<Check class="size-3.5" />{:else}<Plus class="size-3.5" />{/if}
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
