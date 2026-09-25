<script lang="ts">
  /**
   * Reading mode for the stove: one big step at a time, screen kept awake,
   * swipe or tap to move, one-tap timers for any time mentioned in a step.
   */
  import AlarmClock from "@lucide/svelte/icons/alarm-clock"
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import Check from "@lucide/svelte/icons/check"
  import List from "@lucide/svelte/icons/list"
  import Moon from "@lucide/svelte/icons/moon"
  import PartyPopper from "@lucide/svelte/icons/party-popper"
  import Sun from "@lucide/svelte/icons/sun"
  import X from "@lucide/svelte/icons/x"
  import { fly } from "svelte/transition"
  import Modal from "../components/Modal.svelte"
  import ScaleControl from "../components/ScaleControl.svelte"
  import { api, pathId } from "../lib/api"
  import { markCooked } from "../lib/history"
  import { findTimers, parseIngredient, scaleIngredient } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import type { Recipe } from "../lib/recipe"
  import { getScale, read, setScale, write } from "../lib/storage"
  import { timers as kitchenTimers } from "../lib/timers.svelte"
  import { screenAwake } from "../lib/wakelock.svelte"

  const id = pathId()
  const page = pageState<{ recipe: Recipe }>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
  }))
  const recipe = $derived(page.data?.recipe)
  $effect(() => {
    if (recipe) document.title = `Cooking: ${recipe.title} · Crumb`
  })

  const awake = screenAwake()
  let scale = $state(getScale(id))
  $effect(() => setScale(id, scale))

  const steps = $derived(
    (recipe?.instructions ?? []).flatMap((s) => s.items.map((text) => ({ text, section: s.name }))),
  )
  const ingredients = $derived(
    (recipe?.ingredients ?? []).flatMap((s) => s.items.map((raw) => ({ raw, section: s.name }))),
  )

  const STEP_KEY = `crumb:cook:${id}`
  let index = $state(read(STEP_KEY, 0, "session"))
  $effect(() => write(STEP_KEY, index, "session"))
  // A remembered step past the end (the recipe was edited since) starts over
  $effect(() => {
    if (steps.length && index >= steps.length) index = 0
  })
  let finished = $state(false)
  let direction = $state(1)
  const step = $derived(steps[index])

  // Reaching the end logs a cook once per visit (the server also ignores repeats)
  let logged = false
  function go(to: number) {
    if (to < 0) return
    if (to >= steps.length) {
      finished = true
      if (!logged) {
        logged = true
        void markCooked(id)
      }
      return
    }
    direction = to > index ? 1 : -1
    finished = false
    index = to
  }
  const next = () => go(index + 1)
  function prev() {
    if (finished) finished = false
    else go(index - 1)
  }

  function onkeydown(e: KeyboardEvent) {
    if (showIngredients) return
    if (["ArrowRight", " ", "PageDown"].includes(e.key)) {
      e.preventDefault()
      next()
    } else if (["ArrowLeft", "PageUp"].includes(e.key)) {
      e.preventDefault()
      prev()
    }
  }

  // Swipe left/right on the step
  let startX = 0
  let startY = 0
  function onpointerdown(e: PointerEvent) {
    startX = e.clientX
    startY = e.clientY
  }
  function onpointerup(e: PointerEvent) {
    const dx = e.clientX - startX
    if (Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(e.clientY - startY)) {
      if (dx < 0) next()
      else prev()
    }
  }

  // Ingredients mentioned in this step ("Add the flour and sugar" → flour, sugar)
  const stepIngredients = $derived.by(() => {
    const text = step?.text.toLowerCase() ?? ""
    if (!text) return []
    return ingredients.filter(({ raw }) => {
      const name = parseIngredient(raw)
        .name.toLowerCase()
        .replace(/[^a-z\s-]/g, " ")
      const words = name.split(/\s+/).filter((w) => w.length > 3)
      const key = words[words.length - 1]
      return key ? text.includes(key.replace(/(es|s)$/, "")) : false
    })
  })

  const stepTimers = $derived(step ? findTimers(step.text) : [])

  let showIngredients = $state(false)
  let checked = $state(new Set<number>())
  function toggle(i: number) {
    const n = new Set(checked)
    if (n.has(i)) n.delete(i)
    else n.add(i)
    checked = n
  }

  // Scale text size to step length so long steps still fit
  const textSize = $derived.by(() => {
    const len = step?.text.length ?? 0
    if (len > 420) return "text-xl sm:text-2xl"
    if (len > 240) return "text-2xl sm:text-3xl"
    return "text-3xl sm:text-4xl"
  })
</script>

<svelte:window {onkeydown} />

<div class="dark bg-canvas text-ink fixed inset-0 flex flex-col">
  <!-- Top bar -->
  <header class="flex items-center gap-2 px-3 pt-[max(env(safe-area-inset-top),12px)] pb-2">
    <a href={`/recipes/${id}`} class="btn btn-ghost btn-lg btn-icon" aria-label="Exit cook mode">
      <X />
    </a>
    <div class="min-w-0 flex-1 text-center">
      <p class="truncate font-serif text-sm font-semibold">{recipe?.title ?? ""}</p>
      <p class="text-ink-muted text-xs">
        {#if finished}All done{:else if steps.length}Step {index + 1} of {steps.length}{/if}
      </p>
    </div>
    <button
      type="button"
      class="btn btn-ghost btn-lg btn-icon"
      aria-label="Ingredients"
      onclick={() => (showIngredients = true)}
    >
      <List />
    </button>
  </header>

  <!-- Progress: tap a segment to jump -->
  <div class="flex gap-1 px-4">
    {#each steps as _, i (i)}
      <button
        type="button"
        class={[
          "h-1.5 flex-1 rounded-full transition",
          i < index || finished ? "bg-primary" : i === index ? "bg-primary/60" : "bg-raised",
        ]}
        aria-label={`Go to step ${i + 1}`}
        onclick={() => go(i)}
      ></button>
    {/each}
  </div>

  {#if awake.supported}
    <div class="flex justify-center px-4 pt-3">
      <button
        type="button"
        class={[
          "rounded-ui flex h-7 items-center gap-1.5 px-3 text-xs",
          awake.active ? "text-ink-muted" : "bg-primary/15 text-primary",
        ]}
        onclick={awake.request}
      >
        {#if awake.active}<Sun class="size-3.5" /> Screen will stay on{:else}<Moon
            class="size-3.5"
          /> Tap to keep the screen on{/if}
      </button>
    </div>
  {/if}

  <!-- Step -->
  <main
    class="relative grid flex-1 touch-pan-y items-center overflow-hidden"
    {onpointerdown}
    {onpointerup}
  >
    {#if page.loading}
      <div class="mx-auto w-full max-w-3xl space-y-4 px-6">
        <div class="skeleton h-10 w-3/4"></div>
        <div class="skeleton h-10 w-2/3"></div>
      </div>
    {:else if finished}
      <section
        class="mx-auto w-full max-w-2xl px-6 text-center [grid-area:1/1]"
        in:fly={{ x: 40 * direction, duration: 280 }}
      >
        <div class="text-6xl">🍽️</div>
        <h2 class="mt-4 font-serif text-4xl font-semibold">Bon appétit!</h2>
        <p class="text-ink-muted mt-2">That's every step. Enjoy it.</p>
        <div class="mt-8 flex justify-center gap-3">
          <a href={`/recipes/${id}`} class="btn btn-primary btn-xl">Back to recipe</a>
          <button type="button" class="btn btn-soft btn-xl" onclick={() => go(0)}>
            Start over
          </button>
        </div>
      </section>
    {:else if step}
      {#key index}
        <section
          class="mx-auto w-full max-w-3xl px-6 py-4 [grid-area:1/1]"
          in:fly={{ x: 40 * direction, duration: 280, delay: 60 }}
          out:fly={{ x: -40 * direction, duration: 180 }}
        >
          {#if step.section}<p class="kicker mb-2 text-sm">{step.section}</p>{/if}
          <div class="flex items-start gap-4">
            <span class="text-primary font-serif text-5xl leading-none font-semibold sm:text-6xl">
              {index + 1}
            </span>
            <p class={["font-serif leading-snug text-pretty", textSize]}>{step.text}</p>
          </div>

          {#if stepTimers.length}
            <div class="mt-6 flex flex-wrap gap-2">
              {#each stepTimers as t (t.label)}
                <button
                  type="button"
                  class="btn btn-soft btn-lg"
                  onclick={() => kitchenTimers.start(`Step ${index + 1}: ${t.label}`, t.seconds)}
                >
                  <AlarmClock /> Start {t.label} timer
                </button>
              {/each}
            </div>
          {/if}

          {#if stepIngredients.length}
            <div class="mt-6">
              <p class="text-ink-muted mb-2 text-xs font-bold tracking-wider uppercase">
                You'll need
              </p>
              <div class="flex flex-wrap gap-2">
                {#each stepIngredients as ing (ing.raw)}
                  <span class="chip bg-raised font-normal">{scaleIngredient(ing.raw, scale)}</span>
                {/each}
              </div>
            </div>
          {/if}
        </section>
      {/key}
    {:else if recipe}
      <section class="mx-auto px-6 text-center">
        <p class="text-ink-muted">This recipe has no steps yet.</p>
      </section>
    {/if}
  </main>

  <!-- Big thumb-zone controls -->
  <footer
    class="grid grid-cols-[auto_1fr] gap-3 px-4 pt-2 pb-[max(env(safe-area-inset-bottom),16px)]"
  >
    <button
      type="button"
      class="btn btn-neutral h-16 w-16 p-0"
      aria-label="Previous step"
      disabled={index === 0 && !finished}
      onclick={prev}
    >
      <ArrowLeft class="size-6" />
    </button>
    {#if !finished}
      <button
        type="button"
        class="btn btn-primary h-16 text-lg"
        disabled={!steps.length}
        onclick={next}
      >
        {index + 1 >= steps.length ? "Finish" : "Next step"}
        {#if index + 1 >= steps.length}<PartyPopper />{:else}<ArrowRight />{/if}
      </button>
    {:else}
      <a href={`/recipes/${id}`} class="btn btn-soft h-16 text-lg">Close</a>
    {/if}
  </footer>

  <Modal bind:open={showIngredients} title="Ingredients" side>
    <div class="mb-3 flex justify-end"><ScaleControl bind:value={scale} /></div>
    <ul>
      {#each ingredients as ing, i (i)}
        <li>
          {#if ing.section && ing.section !== ingredients[i - 1]?.section}
            <p class="kicker mt-4 mb-1">{ing.section}</p>
          {/if}
          <button
            type="button"
            class="flex w-full items-start gap-3 py-2.5 text-left text-lg"
            aria-pressed={checked.has(i)}
            onclick={() => toggle(i)}
          >
            <span
              class={[
                "mt-1 grid size-5 shrink-0 place-items-center rounded-full border-2",
                checked.has(i) ? "bg-primary border-primary" : "border-line-strong",
              ]}
            >
              {#if checked.has(i)}<Check class="size-3.5 text-white" />{/if}
            </span>
            <span class={checked.has(i) ? "text-ink-dim line-through" : ""}>
              {scaleIngredient(ing.raw, scale)}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  </Modal>
</div>
