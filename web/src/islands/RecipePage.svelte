<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import ClipboardCheck from "@lucide/svelte/icons/clipboard-check"
  import Columns2 from "@lucide/svelte/icons/columns-2"
  import Clock from "@lucide/svelte/icons/clock"
  import Dices from "@lucide/svelte/icons/dices"
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import Flame from "@lucide/svelte/icons/flame"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Plus from "@lucide/svelte/icons/plus"
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
  import ShareSheet from "../components/ShareSheet.svelte"
  import WeeChefCard from "../components/WeeChefCard.svelte"
  import { untrack } from "svelte"
  import { whenActive } from "../lib/active"
  import { api, errorMessage, pathId } from "../lib/api"
  import { bookPalette } from "../lib/books"
  import { plural } from "../lib/checks"
  import { cookedLine, logView, markCooked } from "../lib/history"
  import { scaleIngredient } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import {
    countItems,
    hostOf,
    kicker,
    nutritionLabels,
    type ConnectorInfo,
    type Cookbook,
    type CookStats,
    type Recipe,
    type RecipeChecks,
    type ShareLink,
  } from "../lib/recipe"
  import { randomHref, rememberRandom } from "../lib/random"
  import { forgetViewed, getScale, read, rememberViewed, setScale, write } from "../lib/storage"
  import { flash, toast } from "../lib/toast"

  interface Data {
    recipe: Recipe
    cookbooks: Cookbook[]
    inCookbooks: number[]
    cookStats?: CookStats
    /** Wee Chef's check of an imported recipe; null when it was never checked. */
    checks?: RecipeChecks | null
    /** Wee Chef's checks are set up: offers "Check with Wee Chef". */
    weeChefChecks?: boolean
    /** Its share link, if it has one. */
    share?: ShareLink | null
  }

  const id = pathId()
  const page = pageState<Data>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
    inCookbooks: await api<number[]>(`/api/recipes/${id}/cookbooks`),
    checks: await api<RecipeChecks | null>(`/api/recipes/${id}/checks`).catch(() => null),
    weeChefChecks: await api<ConnectorInfo>("/api/connector")
      .then((c) => !!c.weeChefChecks)
      .catch(() => false),
  }))
  const recipe = $derived(page.data?.recipe)

  // Just imported (or asked for from the menu): Wee Chef's check runs in the background for
  // a second or so. Pick up its result (and the tidied recipe) without a reload.
  let polls = 0
  // "Check with Wee Chef" was chosen (when, and the fixes already there): say how it went
  // when it's done. It may wait behind a "Check all", so keep looking for a while.
  let asked = false
  let askedAt = 0
  let fixedBefore = new Set<number>()
  const ASK_WAIT = 5 * 60_000
  $effect(() => {
    if (page.data?.checks?.status !== "pending") return
    const delay = !asked ? 1500 : polls < 10 ? 3000 : 10_000
    const timer = setTimeout(async () => {
      if (!page.data) return
      if (asked ? Date.now() - askedAt > ASK_WAIT : ++polls > 20) {
        asked = false
        return
      }
      if (asked) polls++
      const checks = await api<RecipeChecks | null>(`/api/recipes/${id}/checks`).catch(() => null)
      if (!checks || !page.data) return
      if (checks.status !== "pending" && checks.flags.some((f) => f.state === "fixed")) {
        const fresh = await api<Recipe>(`/api/recipes/${id}`).catch(() => null)
        if (fresh) page.data.recipe = fresh
      }
      page.data.checks = checks.status === "pending" ? { ...checks } : checks
      if (asked && checks.status !== "pending") {
        asked = false
        const review = checks.flags.filter((f) => f.state === "review").length
        // This check's fixes only; the tidy counts each small thing it cleaned up
        const fixed = checks.flags
          .filter((f) => f.state === "fixed" && !fixedBefore.has(f.id))
          .reduce((n, f) => n + (f.detail?.fix === "tidy" ? (f.detail.count ?? 1) : 1), 0)
        const tidied = `Wee Chef tidied ${plural(fixed, "thing")}`
        const look = `${plural(review, "line")} might need a look`
        if (checks.status === "failed")
          toast({ title: "Wee Chef couldn't check this recipe", tone: "error" })
        else if (fixed && review) toast({ title: tidied, description: look })
        else if (fixed) toast({ title: tidied })
        else if (review) toast({ title: look })
        else toast({ title: "Wee Chef found nothing to change" })
      }
    }, delay)
    return () => clearTimeout(timer)
  })

  // Once per recipe: a refreshed copy (after Wee Chef's check, or an undo) isn't a new view
  $effect(() => {
    if (recipe?.id === undefined) return
    untrack(() => {
      const r = recipe!
      whenActive(() => {
        rememberViewed(r)
        logView(r.id)
      })
    })
  })
  $effect(() => {
    if (recipe) document.title = `${recipe.title} · Crumb`
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
  // Ticks are keyed by position, so they don't carry over to a replaced recipe
  let checked = $state(new Set<string>())
  $effect.pre(() => {
    void recipe
    untrack(() => {
      if (checked.size) checked = new Set()
    })
  })
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

  // ─── Ingredients / Method split (desktop) ───
  // One panel is primary and gets most of the width. Clicking or tapping inside a panel, or
  // keyboard focus entering it, makes it primary. Not hover: a boundary that moves under the
  // cursor makes the panels flip back and forth, and the text reflows while you read.
  // Locked, both keep a fixed split and focus changes nothing.
  type Panel = "ingredients" | "method"
  const LOCK_KEY = "crumb:recipe-split-locked"
  let primary = $state<Panel>("method")
  let locked = $state(read<boolean>(LOCK_KEY, false) === true)
  function toggleLock() {
    locked = !locked
    write(LOCK_KEY, locked)
  }

  function focusPanel(panel: Panel, target: EventTarget | null) {
    if (locked || primary === panel) return
    primary = panel
    if (target instanceof Element) holdStill(target)
  }
  function onPanelClick(panel: Panel, e: MouseEvent) {
    // Selecting text to copy shouldn't reflow it
    if (!getSelection()?.isCollapsed) return
    focusPanel(panel, e.target)
  }
  function onPanelFocus(panel: Panel, e: FocusEvent) {
    // Mouse clicks focus buttons too, but the click handler covers those (after the
    // click lands, so the row can't move out from under the pointer mid-press)
    if (e.target instanceof Element && e.target.matches(":focus-visible")) focusPanel(panel, e.target)
  }

  // Keep what was clicked where it was on screen while the panels resize around it. Stops
  // early if scrolling doesn't move it (a stuck sticky panel).
  function holdStill(el: Element) {
    const top = el.getBoundingClientRect().top
    const until = performance.now() + 400
    const tick = () => {
      const before = el.getBoundingClientRect().top
      const drift = before - top
      if (Math.abs(drift) >= 1) {
        window.scrollBy(0, drift)
        if (el.getBoundingClientRect().top === before) return
      }
      if (performance.now() < until) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  }

  // ─── Actions ───
  // Share, send and download live in the share sheet; the ⋯ menu keeps the rest
  let showShare = $state(false)

  async function checkNow() {
    const before = page.data?.checks?.flags ?? []
    try {
      const checks = await api<RecipeChecks>(`/api/recipes/${id}/checks`, { method: "POST" })
      fixedBefore = new Set(before.filter((f) => f.state === "fixed").map((f) => f.id))
      polls = 0
      asked = true
      askedAt = Date.now()
      if (page.data) page.data.checks = checks
      toast({ title: "Wee Chef is checking…" })
    } catch (e) {
      toast({ title: "Couldn't start the check", description: errorMessage(e), tone: "error" })
    }
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

  const menu: MenuItem[][] = $derived([
    [
      { label: "Mark as cooked", icon: ChefHat, onselect: cookedNow },
      { label: "Edit", icon: Pencil, onselect: () => (location.href = `/recipes/${id}/edit`) },
      { label: "Surprise me", icon: Dices, onselect: () => (location.href = randomHref(id)) },
    ],
    ...(page.data?.weeChefChecks
      ? [[{ label: "Check with Wee Chef", icon: ClipboardCheck, onselect: checkNow }]]
      : []),
    [{ label: "Delete", icon: Trash2, danger: true, onselect: () => (showDelete = true) }],
  ])
</script>

{#if page.loading}
  <div class="recipe-wide space-y-5">
    <div class="skeleton aspect-[4/3] w-full lg:max-w-[640px]"></div>
    <div class="skeleton h-9 w-2/3"></div>
    <div class="skeleton h-4 w-1/2"></div>
  </div>
{:else if !recipe}
  <EmptyState icon={FileQuestion} title="Recipe not found" description="It may have been deleted.">
    <a href="/recipes" class="btn btn-soft">Back to recipes</a>
  </EmptyState>
{:else}
  <article class="recipe-wide space-y-12 lg:space-y-16">
    <!-- Hero: photo left, title and actions right on wide screens. Its box matches the
         skeleton in shell/recipe/index.astro so the card photo morphs onto it. -->
    <header
      class={[
        "grid gap-6",
        hasHero && "lg:grid-cols-[minmax(0,640px)_minmax(16rem,1fr)] lg:gap-12",
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

      <div class="min-w-0 space-y-5 lg:py-2">
        <div class="flex items-start gap-3">
          <div class="min-w-0 flex-1">
            {#if sub}<p class="kicker mb-2">{sub}</p>{/if}
            <h1 class="page-title text-balance">{recipe.title}</h1>
          </div>
          <div class="no-print -mt-1 flex flex-none gap-2">
            <button
              type="button"
              class="btn btn-outline btn-icon"
              aria-label="Share"
              title="Share"
              aria-haspopup="dialog"
              onclick={() => (showShare = true)}
            >
              <Share />
            </button>
            <Menu groups={menu} triggerClass="btn btn-outline btn-icon" />
          </div>
        </div>

        {#if recipe.author || sourceHost || cooked}
          <p class="text-ink-muted -mt-2 flex flex-wrap items-center gap-x-1.5 text-sm">
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
                class="link print-url inline-flex min-h-11 items-center gap-1 text-sm"
              >
                {sourceHost}<ExternalLink class="size-3.5" />
              </a>
            {/if}
          </p>
        {/if}

        {#if recipe.description}
          <p class="text-ink-muted max-w-[65ch] text-[17px] leading-relaxed text-pretty">
            {recipe.description}
          </p>
        {/if}

        {#if page.data?.checks}
          <WeeChefCard
            recipeId={id}
            checks={page.data.checks}
            onundo={(r, c) => {
              if (page.data) {
                page.data.recipe = r
                page.data.checks = c
              }
            }}
          />
        {/if}

        {#if meta.length || scaledYield}
          <dl class="card grid grid-cols-[repeat(auto-fit,minmax(5.5rem,1fr))] px-1 py-1.5">
            {#each meta as m (m.label)}
              <div class="px-3.5 py-3">
                <dt class="meta flex items-center gap-1.5">
                  <m.icon class="text-primary size-4" />{m.label}
                </dt>
                <dd class="mt-1 font-bold">{m.value}</dd>
              </div>
            {/each}
            {#if scaledYield}
              <div class="px-3.5 py-3">
                <dt class="meta flex items-center gap-1.5">
                  <Users class="text-primary size-4" />Serves
                </dt>
                <dd class="mt-1 font-bold">{scaledYield}</dd>
              </div>
            {/if}
          </dl>
        {/if}

        <!-- Cooking mode is the one main action here -->
        <div class="no-print grid grid-cols-2 gap-3 pt-1 lg:grid-cols-1 2xl:grid-cols-2">
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

    <div class={["split", locked && "locked"]} data-primary={primary}>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <section
        class="panel"
        data-secondary={!locked && primary !== "ingredients" ? "" : undefined}
        onclick={(e) => onPanelClick("ingredients", e)}
        onfocusin={(e) => onPanelFocus("ingredients", e)}
      >
        <div class="lg:sticky lg:top-6">
          <div class="mb-5 flex min-h-11 flex-wrap items-center justify-between gap-x-3 gap-y-2">
            <h2 class="section-title">Ingredients</h2>
            {#if ingredientCount}<ScaleControl bind:value={scale} class="no-print" />{/if}
          </div>
          {#if !ingredientCount}<p class="text-ink-muted text-sm">No ingredients listed.</p>{/if}
          <div class="space-y-6">
            {#each recipe.ingredients as section, si (si)}
              <div>
                {#if section.name}<h3 class="kicker mb-2.5">{section.name}</h3>{/if}
                <ul class="ingredients list-card">
                  {#each section.items as item, ii (ii)}
                    {@const key = `${si}-${ii}`}
                    {@const on = checked.has(key)}
                    <li class="relative">
                      <button
                        type="button"
                        class="hover:bg-tint/55 active:bg-tint flex min-h-12 w-full items-start gap-3 px-4 py-3 text-left transition-colors"
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
                          class={[
                            "min-w-0 leading-snug break-words transition",
                            on && "text-ink-muted line-through",
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
        </div>
      </section>

      <!-- Desktop only: between the panels -->
      <div class="gutter no-print">
        <button
          type="button"
          class={["btn btn-icon sticky top-6", locked ? "btn-tile" : "btn-outline text-ink-muted"]}
          aria-pressed={locked}
          aria-label="Keep the panels from resizing"
          title="Keep the panels from resizing"
          onclick={toggleLock}
        >
          <Columns2 class="size-5" />
        </button>
      </div>

      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <section
        class="panel"
        data-secondary={!locked && primary !== "method" ? "" : undefined}
        onclick={(e) => onPanelClick("method", e)}
        onfocusin={(e) => onPanelFocus("method", e)}
      >
        <h2 class="section-title mb-5 flex min-h-11 items-center">Method</h2>
        {#if !recipe.instructions.length}<p class="text-ink-muted text-sm">No steps listed.</p>{/if}
        <div class="space-y-8">
          {#each recipe.instructions as section, si (si)}
            <div>
              {#if section.name}<h3 class="kicker mb-4">{section.name}</h3>{/if}
              <ol class="space-y-6">
                {#each section.items as step, i (i)}
                  <li class="flex gap-4">
                    <span
                      class="text-primary w-9 shrink-0 text-right text-[28px] leading-none font-extrabold tabular-nums"
                      aria-hidden="true"
                    >
                      {stepOffset(si) + i + 1}
                    </span>
                    <p class="min-w-0 pt-0.5 text-[17px] leading-relaxed break-words">
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
          <div class="card mt-10 p-6">
            <h2 class="text-primary flex items-center gap-2 font-bold">
              <StickyNote class="size-[18px]" /> Notes
            </h2>
            <p class="hand mt-2 text-[26px] leading-tight whitespace-pre-line">{recipe.notes}</p>
          </div>
        {/if}

        {#if recipe.nutrition && Object.keys(recipe.nutrition).length}
          <div class="mt-10">
            <h2 class="section-title mb-5">Nutrition</h2>
            <dl class="grid grid-cols-[repeat(auto-fill,minmax(8.5rem,1fr))] gap-3">
              {#each Object.entries(recipe.nutrition) as [key, value] (key)}
                <div class="card px-4 py-3">
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
        <h2 class="section-title mb-5">On the shelf in</h2>
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

{#if recipe && page.data}
  <ShareSheet bind:open={showShare} {recipe} bind:share={page.data.share} />
{/if}

<Modal bind:open={showDelete} title="Delete this recipe?" description="This can't be undone.">
  {#snippet footer()}
    <button type="button" class="btn btn-ghost" onclick={() => (showDelete = false)}>Cancel</button>
    <button type="button" class="btn btn-danger" onclick={deleteRecipe}>Delete</button>
  {/snippet}
</Modal>

<style>
  /* About a quarter wider than the layout's column where the screen has room, never
     narrower, centred on it. The nav rail is 15rem; keep the page's 2rem side padding and
     1rem for a scrollbar. Same rule as the skeleton in shell/recipe/index.astro. */
  @media (min-width: 768px) {
    .recipe-wide {
      --w: min(88.75rem, max(100%, 100vw - 20rem));
      width: var(--w);
      margin-inline: calc((100% - var(--w)) / 2);
    }
  }

  /* Ingredients | toggle | Method. Stacked on phones and tablets. */
  .split {
    display: grid;
    gap: 3rem;
  }
  .gutter {
    display: none;
  }
  @media (min-width: 1024px) {
    .split {
      grid-template-columns: minmax(0, 38fr) 2.75rem minmax(0, 62fr);
      column-gap: 1.75rem;
      transition: grid-template-columns 250ms ease;
    }
    /* Short ingredient lines don't need as much room as steps do */
    .split:not(.locked)[data-primary="ingredients"] {
      grid-template-columns: minmax(0, 56fr) 2.75rem minmax(0, 44fr);
    }
    .split.locked {
      grid-template-columns: minmax(0, 42fr) 2.75rem minmax(0, 58fr);
    }
    /* The narrower panel invites a click */
    .panel[data-secondary] {
      cursor: pointer;
    }
    .gutter {
      display: block;
      position: relative;
    }
    /* A hairline down the gutter, below the toggle */
    .gutter::after {
      content: "";
      position: absolute;
      top: 4.25rem;
      bottom: 0;
      left: 50%;
      border-left: 1px solid var(--border);
    }
    .gutter > button {
      z-index: 1;
    }
    /* The narrower panel's headings step back; everything stays readable and usable */
    .panel[data-secondary] .section-title {
      color: var(--text-muted);
      transition: color 250ms ease;
    }
  }
  /* Small desktops: a gentler swing, so the narrow panel stays comfortable for long steps */
  @media (min-width: 1024px) and (max-width: 1279px) {
    .split {
      grid-template-columns: minmax(0, 42fr) 2.75rem minmax(0, 58fr);
    }
    .split:not(.locked)[data-primary="ingredients"] {
      grid-template-columns: minmax(0, 54fr) 2.75rem minmax(0, 46fr);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .split,
    .panel .section-title {
      transition: none;
    }
  }

  /* Paper: one column, no controls, nothing split across a page break */
  @media print {
    .split {
      display: block;
    }
    .panel + .panel {
      margin-top: 2rem;
    }
    .panel :global(.lg\:sticky) {
      position: static;
    }
    .ingredients > li,
    ol > li,
    .panel .space-y-6 > div {
      break-inside: avoid;
    }
    .print-url::after {
      content: " (" attr(href) ")";
      font-weight: 400;
      overflow-wrap: anywhere;
    }
  }

  /* Inset dividers between ingredient rows, as in .list-row */
  .ingredients > li + li::before {
    content: "";
    position: absolute;
    top: 0;
    left: 1rem;
    right: 1rem;
    border-top: 1px solid var(--border);
    pointer-events: none;
  }
</style>
