<script lang="ts">
  /**
   * Mise en place: everything prepped, measured and in its place before the heat goes on.
   * Each ingredient gets a vessel sized to its quantity; tap it once it's ready.
   */
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import Check from "@lucide/svelte/icons/check"
  import Flame from "@lucide/svelte/icons/flame"
  import Hand from "@lucide/svelte/icons/hand"
  import Slice from "@lucide/svelte/icons/slice"
  import Soup from "@lucide/svelte/icons/soup"
  import { fly } from "svelte/transition"
  import EmptyState from "../components/EmptyState.svelte"
  import PrepBowl from "../components/PrepBowl.svelte"
  import ScaleControl from "../components/ScaleControl.svelte"
  import { api, pathId } from "../lib/api"
  import { ingredientColor } from "../lib/ingredientColor"
  import {
    formatQuantity,
    miseEnPlace,
    pluralUnit,
    type MiseItem,
    type Vessel,
  } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import type { Recipe } from "../lib/recipe"
  import { getScale, read, setScale, write } from "../lib/storage"
  import { screenAwake } from "../lib/wakelock.svelte"

  const id = pathId()
  const page = pageState<{ recipe: Recipe }>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
  }))
  const recipe = $derived(page.data?.recipe)
  $effect(() => {
    if (recipe) document.title = `Mise en place: ${recipe.title} · Crumb`
  })

  const awake = screenAwake()
  let scale = $state(getScale(id))
  $effect(() => setScale(id, scale))

  const items = $derived(
    miseEnPlace((recipe?.ingredients ?? []).flatMap((s) => s.items)).map((item, i) => ({
      ...item,
      key: i,
    })),
  )

  const SIZE_ORDER: Vessel[] = ["large", "medium", "small", "ramekin", "pinch"]
  const groups = $derived(
    [
      {
        title: "Chop & prep",
        hint: "Wash, peel and cut these first",
        icon: Slice,
        items: items.filter((i) => i.vessel === "board"),
      },
      {
        title: "Measure into bowls",
        hint: "Biggest bowls first, then the little ones",
        icon: Soup,
        items: items
          .filter((i) => SIZE_ORDER.includes(i.vessel))
          .toSorted((a, b) => SIZE_ORDER.indexOf(a.vessel) - SIZE_ORDER.indexOf(b.vessel)),
      },
      {
        title: "Keep within reach",
        hint: "Seasoning and extras to have out on the counter",
        icon: Hand,
        items: items.filter((i) => i.vessel === "jar"),
      },
    ].filter((g) => g.items.length),
  )

  const VESSEL_LABEL: Record<Vessel, string> = {
    large: "Large bowl",
    medium: "Medium bowl",
    small: "Small bowl",
    ramekin: "Ramekin",
    pinch: "Pinch bowl",
    board: "Board",
    jar: "On hand",
  }

  function amount(item: MiseItem) {
    if (item.quantity === null) return item.unit ? `a ${item.unit}` : ""
    const q = formatQuantity(item.quantity * scale)
    const max = item.quantityMax !== null ? `–${formatQuantity(item.quantityMax * scale)}` : ""
    const n = (item.quantityMax ?? item.quantity) * scale
    return `${q}${max}${item.unit ? ` ${pluralUnit(item.unit, n)}` : ""}`
  }

  const READY_KEY = `crumb:prep:${id}`
  let ready = $state<number[]>(read(READY_KEY, [], "session"))
  $effect(() => write(READY_KEY, ready, "session"))
  function toggle(key: number) {
    ready = ready.includes(key) ? ready.filter((k) => k !== key) : [...ready, key]
    if (ready.length === items.length) navigator.vibrate?.(60)
  }
  const progress = $derived(items.length ? ready.length / items.length : 0)
  const allReady = $derived(items.length > 0 && ready.length === items.length)

  const vesselCounts = $derived.by(() => {
    const counts = new Map<Vessel, number>()
    for (const i of items) counts.set(i.vessel, (counts.get(i.vessel) ?? 0) + 1)
    // One cutting board does for all the chopping
    if (counts.has("board")) counts.set("board", 1)
    return [...counts.entries()].filter(([v]) => v !== "jar")
  })
</script>

<div class="mx-auto max-w-5xl">
  <div class="mb-6 flex items-start justify-between gap-3">
    <div class="min-w-0">
      <a
        href={`/recipes/${id}`}
        class="text-ink-muted hover:text-primary inline-flex items-center gap-1 text-sm"
      >
        <ArrowLeft class="size-4" />
        <span class="truncate">{recipe?.title ?? "Recipe"}</span>
      </a>
      <h1 class="mt-1 font-serif text-3xl font-semibold sm:text-4xl">Mise en place</h1>
      <p class="text-ink-muted mt-1">
        <em>“Everything in its place.”</em> Prep and measure it all before you start cooking. Tap each
        one when it's ready.
      </p>
    </div>
    <ScaleControl bind:value={scale} class="hidden shrink-0 sm:inline-flex" />
  </div>

  {#if vesselCounts.length}
    <!-- What to get out -->
    <div class="card mb-6 flex flex-wrap items-center gap-x-5 gap-y-2 px-4 py-3 text-sm">
      <span class="font-semibold">Get out:</span>
      {#each vesselCounts as [v, n] (v)}
        <span class="text-ink-muted">
          <strong class="text-ink">{n}</strong> × {VESSEL_LABEL[v].toLowerCase()}{n > 1 ? "s" : ""}
        </span>
      {/each}
      <ScaleControl bind:value={scale} class="ml-auto sm:hidden" />
    </div>
  {/if}

  <!-- Progress -->
  <div class="border-line bg-canvas/90 sticky top-14 z-20 -mx-4 mb-6 border-b px-4 py-3 backdrop-blur">
    <div class="flex items-center gap-3">
      <div class="bg-raised h-3 flex-1 overflow-hidden rounded-full">
        <div
          class="bg-primary h-full rounded-full transition-all duration-500"
          style:width={`${progress * 100}%`}
        ></div>
      </div>
      <span class="text-sm font-semibold tabular-nums">{ready.length}/{items.length} ready</span>
    </div>
    {#if awake.supported && !awake.active}
      <button type="button" class="text-primary mt-1 text-xs" onclick={awake.request}>
        Tap to keep the screen on while you prep
      </button>
    {/if}
  </div>

  {#if page.loading}
    <div class="skeleton h-64 w-full"></div>
  {:else if !items.length}
    <EmptyState icon={Soup} title="No ingredients to prep">
      <a href={`/recipes/${id}`} class="btn btn-soft">Back to recipe</a>
    </EmptyState>
  {:else}
    <div class="counter rounded-ui space-y-10 p-4 sm:p-8">
      {#each groups as group (group.title)}
        <section>
          <div class="mb-4 flex items-center gap-2">
            <group.icon class="text-primary size-5" />
            <h2 class="font-serif text-xl font-semibold">{group.title}</h2>
            <span class="text-ink-muted hidden text-sm sm:inline">· {group.hint}</span>
          </div>
          <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {#each group.items as item (item.key)}
              {@const isReady = ready.includes(item.key)}
              <button
                type="button"
                class={[
                  "group rounded-ui relative flex flex-col items-center px-2 pt-4 pb-3 text-center transition active:scale-[0.97]",
                  isReady ? "bg-primary/10" : "bg-canvas/70 hover:bg-canvas shadow-sm",
                ]}
                aria-pressed={isReady}
                onclick={() => toggle(item.key)}
              >
                <span
                  class={[
                    "absolute top-2 right-2 grid size-6 place-items-center rounded-full transition",
                    isReady
                      ? "bg-primary scale-100 text-white"
                      : "border-line-strong scale-75 border-2",
                  ]}
                >
                  {#if isReady}<Check class="size-4" />{/if}
                </span>
                <div class="flex h-28 items-end justify-center transition group-hover:-translate-y-0.5">
                  <PrepBowl vessel={item.vessel} filled={isReady} color={ingredientColor(item.name)} />
                </div>
                <p class="mt-2 text-lg leading-tight font-semibold tabular-nums">
                  {amount(item) || " "}
                </p>
                <p class={["line-clamp-2 leading-snug", isReady && "text-ink-muted line-through"]}>
                  {item.name}
                </p>
                {#if item.task}
                  <span
                    class="rounded-ui mt-1.5 bg-amber-100 px-2 py-0.5 text-xs font-semibold text-amber-800 capitalize dark:bg-amber-900/50 dark:text-amber-200"
                  >
                    {item.task}
                  </span>
                {:else if item.prep}
                  <span class="text-ink-muted mt-1 line-clamp-1 text-xs">{item.prep}</span>
                {/if}
                <span class="text-ink-dim mt-1 text-[11px] tracking-wide uppercase">
                  {VESSEL_LABEL[item.vessel]}
                </span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}

  {#if allReady}
    <div
      class="fixed inset-x-0 bottom-24 z-30 flex justify-center px-4 md:bottom-8"
      transition:fly={{ y: 24, duration: 300 }}
    >
      <a href={`/recipes/${id}/cook`} class="btn btn-primary btn-xl shadow-xl" data-no-prerender>
        <Flame /> Everything's in its place. Start cooking
      </a>
    </div>
  {/if}
</div>

<style>
  /* A warm speckled countertop */
  .counter {
    background:
      radial-gradient(circle at 20% 30%, rgb(0 0 0 / 0.025) 0 1px, transparent 1.5px) 0 0 / 14px
        14px,
      radial-gradient(circle at 70% 60%, rgb(0 0 0 / 0.03) 0 1px, transparent 1.5px) 0 0 / 19px 19px,
      var(--bg-muted);
    border: 1px solid var(--border-muted);
  }
</style>
