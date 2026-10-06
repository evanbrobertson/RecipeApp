<script lang="ts">
  /**
   * Reading mode for the stove: one big step at a time, screen kept awake, swipe or tap to
   * move, a timer on any step (suggested from the times it mentions) and the ingredients
   * pinned beside the steps on wider screens.
   */
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import Check from "@lucide/svelte/icons/check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import List from "@lucide/svelte/icons/list"
  import Moon from "@lucide/svelte/icons/moon"
  import PartyPopper from "@lucide/svelte/icons/party-popper"
  import Sun from "@lucide/svelte/icons/sun"
  import Volume2 from "@lucide/svelte/icons/volume-2"
  import VolumeOff from "@lucide/svelte/icons/volume-off"
  import X from "@lucide/svelte/icons/x"
  import { MediaQuery } from "svelte/reactivity"
  import { fly, slide } from "svelte/transition"
  import Modal from "../components/Modal.svelte"
  import ScaleControl from "../components/ScaleControl.svelte"
  import StepTimer from "../components/StepTimer.svelte"
  import { api, pathId } from "../lib/api"
  import { markCooked } from "../lib/history"
  import { findTimers, scaleIngredient } from "../lib/ingredients"
  import { pageState } from "../lib/page.svelte"
  import type { Recipe } from "../lib/recipe"
  import { stepReader } from "../lib/speech.svelte"
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
  // Read aloud: each new step is spoken as it comes up (and the current one when switched on)
  const reader = stepReader()
  $effect(() => {
    if (!reader.on) return
    if (finished) reader.speak("All done. Bon appétit!")
    else if (step) reader.speak(`Step ${index + 1}. ${step.text}`)
  })
  $effect(() => reader.stop)

  const next = () => go(index + 1)
  function prev() {
    if (finished) finished = false
    else go(index - 1)
  }

  function onkeydown(e: KeyboardEvent) {
    if (showIngredients || e.target instanceof HTMLInputElement) return
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

  // A step's timer is only a suggestion: the length the cook settles on is kept for the visit
  const stepTimers = $derived(step ? findTimers(step.text) : [])
  const TIMERS_KEY = `crumb:cook-timers:${id}`
  let timerLengths = $state(read<Record<number, number>>(TIMERS_KEY, {}, "session"))
  $effect(() => write(TIMERS_KEY, timerLengths, "session"))

  // Wide screens keep the ingredients beside the steps until unpinned; phones open them over
  const wide = new MediaQuery("min-width: 768px")
  const PINNED_KEY = "crumb:cook-pinned"
  let pinned = $state(read(PINNED_KEY, true))
  $effect(() => write(PINNED_KEY, pinned))
  const docked = $derived(pinned && wide.current)

  let showIngredients = $state(false)
  $effect(() => {
    if (wide.current) showIngredients = false
  })
  const CHECKED_KEY = `crumb:cook-checked:${id}`
  let checked = $state(new Set(read<number[]>(CHECKED_KEY, [], "session")))
  $effect(() => write(CHECKED_KEY, [...checked], "session"))
  function toggle(i: number) {
    const n = new Set(checked)
    if (n.has(i)) n.delete(i)
    else n.add(i)
    checked = n
  }

  // Scale text size to step length so long steps still fit
  const textSize = $derived.by(() => {
    const len = step?.text.length ?? 0
    if (len > 420) return "text-[1.3125rem] sm:text-2xl"
    if (len > 240) return "text-2xl sm:text-[1.75rem]"
    return "text-[1.75rem] sm:text-4xl"
  })
</script>

<svelte:window {onkeydown} />

<!-- Follows the app theme: cream by day, evening kitchen after dark. The tile is the chrome. -->
<div
  class="cook-mode bg-canvas text-ink fixed inset-0 flex flex-col"
  data-docked={docked || undefined}
>
  <header class="tile-surface pt-[max(env(safe-area-inset-top),8px)]">
    <div class="mx-auto flex max-w-3xl items-center gap-2 px-2 sm:px-4">
      <a href={`/recipes/${id}`} class="btn btn-lg btn-icon on-tile-btn" aria-label="Exit cook mode">
        <X class="size-[22px]" />
      </a>
      <div class="min-w-0 flex-1 text-center">
        <p class="truncate font-serif text-xl leading-tight">{recipe?.title ?? ""}</p>
        <p class="mt-0.5 flex items-center justify-center gap-1.5 text-[13px] font-semibold">
          {#if finished}All done{:else if steps.length}Step {index + 1} of {steps.length}{/if}
          {#if awake.supported && awake.active}
            <span aria-hidden="true">·</span>
            <Sun class="size-3.5" /> Screen stays on
          {/if}
        </p>
      </div>
      {#if reader.supported}
        <button
          type="button"
          class="btn btn-lg btn-icon on-tile-btn"
          aria-label="Read steps aloud"
          aria-pressed={reader.on}
          title={reader.on ? "Stop reading steps aloud" : "Read steps aloud"}
          onclick={reader.toggle}
        >
          {#if reader.on}<Volume2 class="size-[22px]" />{:else}<VolumeOff
              class="size-[22px]"
            />{/if}
        </button>
      {/if}
      {#if wide.current}
        <button
          type="button"
          class="btn btn-lg btn-icon on-tile-btn"
          aria-label="Pin ingredients"
          aria-pressed={pinned}
          title={pinned ? "Hide the ingredients" : "Pin the ingredients beside the steps"}
          onclick={() => (pinned = !pinned)}
        >
          <List class="size-[22px]" />
        </button>
      {:else}
        <button
          type="button"
          class="btn btn-lg btn-icon on-tile-btn"
          aria-label="Ingredients"
          aria-haspopup="dialog"
          onclick={() => (showIngredients = true)}
        >
          <List class="size-[22px]" />
        </button>
      {/if}
    </div>

    <!-- Progress: tap a segment to jump -->
    <div class="mx-auto flex max-w-3xl gap-1 px-4">
      {#each steps as _, i (i)}
        <button
          type="button"
          class="group flex h-11 flex-1 items-center"
          aria-label={`Go to step ${i + 1}`}
          aria-current={i === index && !finished ? "step" : undefined}
          onclick={() => go(i)}
        >
          <span
            class={[
              "block w-full rounded-full transition-all",
              i === index && !finished ? "h-2.5" : "h-1.5 group-hover:h-2",
              i <= index || finished ? "bg-on-tile" : "bg-on-tile/30",
            ]}
          ></span>
        </button>
      {/each}
    </div>

    {#if awake.supported && !awake.active}
      <div class="flex justify-center px-4 pb-3">
        <button type="button" class="chip bg-paper text-primary h-11" onclick={awake.request}>
          <Moon class="size-[18px]" /> Tap to keep the screen on
        </button>
      </div>
    {:else}
      <div class="h-1"></div>
    {/if}
  </header>

  <div class="flex min-h-0 flex-1">
    <div class="flex min-w-0 flex-1 flex-col">
      <!-- Step -->
      <main
        class={[
          "relative grid flex-1 touch-pan-y items-center overflow-x-hidden overflow-y-auto",
          kitchenTimers.list.length && "pb-14",
        ]}
        {onpointerdown}
        {onpointerup}
      >
        {#if page.loading}
          <div class="mx-auto w-full max-w-3xl space-y-4 px-5">
            <div class="skeleton h-10 w-3/4"></div>
            <div class="skeleton h-10 w-2/3"></div>
          </div>
        {:else if finished}
          <section
            class="mx-auto w-full max-w-2xl px-5 py-6 text-center [grid-area:1/1]"
            in:fly={{ x: 40 * direction, duration: 280 }}
          >
            <div class="tile-surface rounded-ui mx-auto grid size-24 place-items-center">
              <ChefHat class="size-11" strokeWidth={1.6} />
            </div>
            <h2 class="page-title mt-5 text-4xl">Bon appétit!</h2>
            <p class="text-ink-muted mt-2 text-lg">That's every step. Enjoy it.</p>
            <div class="mt-8 flex flex-wrap justify-center gap-3">
              <a href={`/recipes/${id}`} class="btn btn-primary btn-xl">Back to recipe</a>
              <button type="button" class="btn btn-soft btn-xl" onclick={() => go(0)}>
                Start over
              </button>
            </div>
          </section>
        {:else if step}
          {#key index}
            <section
              class="mx-auto w-full max-w-3xl px-5 py-6 [grid-area:1/1] sm:px-8"
              in:fly={{ x: 40 * direction, duration: 280, delay: 60 }}
              out:fly={{ x: -40 * direction, duration: 180 }}
            >
              {#if step.section}<p class="kicker mb-3">{step.section}</p>{/if}
              <div class="flex items-start gap-4 sm:gap-6">
                <span
                  class="text-primary font-serif text-5xl leading-[0.9] tabular-nums sm:text-6xl"
                  aria-hidden="true"
                >
                  {index + 1}
                </span>
                <p class={["leading-snug font-semibold text-pretty", textSize]}>{step.text}</p>
              </div>

              <div class="mt-7">
                <StepTimer
                  found={stepTimers}
                  bind:seconds={
                    () => timerLengths[index] ?? null,
                    (s) => {
                      if (s !== null) timerLengths[index] = s
                    }
                  }
                  onstart={(s) => kitchenTimers.start(`Step ${index + 1}`, s)}
                />
              </div>
            </section>
          {/key}
        {:else if recipe}
          <section class="mx-auto px-5 text-center">
            <p class="text-ink-muted text-lg">This recipe has no steps yet.</p>
          </section>
        {/if}
      </main>

      <!-- Big thumb-zone controls -->
      <footer class="border-line bg-paper border-t">
        <div
          class="mx-auto grid max-w-3xl grid-cols-[auto_1fr] gap-3 px-4 pt-3 pb-[max(env(safe-area-inset-bottom),16px)] sm:px-8"
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
            <a href={`/recipes/${id}`} class="btn btn-outline h-16 text-lg">Close</a>
          {/if}
        </div>
      </footer>
    </div>

    {#if docked}
      <aside
        class="border-line bg-paper shrink-0 border-l"
        aria-label="Ingredients"
        transition:slide={{ axis: "x", duration: 220 }}
      >
        <div
          class="ingredients-panel h-full overflow-y-auto px-5 pt-5 pb-[max(env(safe-area-inset-bottom),1.25rem)]"
        >
          <h2 class="section-title">Ingredients</h2>
          <ScaleControl class="mt-3 mb-2" bind:value={scale} />
          {@render ingredientList()}
        </div>
      </aside>
    {/if}
  </div>

  <Modal bind:open={showIngredients} title="Ingredients" side>
    <div class="mb-3 flex justify-end"><ScaleControl bind:value={scale} /></div>
    {@render ingredientList()}
  </Modal>
</div>

{#snippet ingredientList()}
  <ul>
    {#each ingredients as ing, i (i)}
      <li>
        {#if ing.section && ing.section !== ingredients[i - 1]?.section}
          <p class="kicker mt-4 mb-1">{ing.section}</p>
        {/if}
        <button
          type="button"
          class="rounded-ctl hover:bg-tint/60 active:bg-tint -mx-2 flex min-h-12 w-[calc(100%+1rem)] items-start gap-3 px-2 py-2.5 text-left text-lg transition"
          aria-pressed={checked.has(i)}
          onclick={() => toggle(i)}
        >
          <span
            class={[
              "mt-0.5 grid size-6 shrink-0 place-items-center rounded-full border-2 transition",
              checked.has(i) ? "bg-tile border-tile text-on-tile" : "border-line-strong",
            ]}
          >
            {#if checked.has(i)}<Check class="size-4" strokeWidth={2.5} />{/if}
          </span>
          <span class={checked.has(i) ? "text-ink-muted line-through" : ""}>
            {scaleIngredient(ing.raw, scale)}
          </span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  /* Running timers sit just above the thumb controls here, clear of the tile chrome */
  :global(body:has(.cook-mode) [data-timer-dock]) {
    top: auto;
    left: 0;
    bottom: calc(max(env(safe-area-inset-bottom), 16px) + 5.75rem);
  }
  /* The pinned ingredients keep one width as the panel slides, so the list doesn't reflow */
  .ingredients-panel {
    width: 18rem;
  }
  @media (min-width: 1024px) {
    .ingredients-panel {
      width: 20rem;
    }
  }
  @media (min-width: 1280px) {
    .ingredients-panel {
      width: 24rem;
    }
  }
  /* ...and the timers centre under the steps rather than behind the panel */
  :global(body:has(.cook-mode[data-docked]) [data-timer-dock]) {
    right: 18rem;
  }
  @media (min-width: 1024px) {
    :global(body:has(.cook-mode[data-docked]) [data-timer-dock]) {
      right: 20rem;
    }
  }
  @media (min-width: 1280px) {
    :global(body:has(.cook-mode[data-docked]) [data-timer-dock]) {
      right: 24rem;
    }
  }
  /* Icon buttons sitting on the tile: paper icons, a 6% darken when pressed */
  .on-tile-btn {
    color: var(--on-tile);
  }
  .on-tile-btn:hover,
  .on-tile-btn:active,
  .on-tile-btn[aria-pressed="true"] {
    background: rgb(0 0 0 / 0.12);
  }
  .on-tile-btn:focus-visible,
  :global(.tile-surface) button:focus-visible {
    outline-color: var(--on-tile);
  }
</style>
