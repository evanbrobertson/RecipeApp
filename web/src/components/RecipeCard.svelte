<script lang="ts">
  import Clock from "@lucide/svelte/icons/clock"
  import Check from "@lucide/svelte/icons/check"
  import CookingPot from "@lucide/svelte/icons/cooking-pot"
  import Soup from "@lucide/svelte/icons/soup"
  import Salad from "@lucide/svelte/icons/salad"
  import Croissant from "@lucide/svelte/icons/croissant"
  import CakeSlice from "@lucide/svelte/icons/cake-slice"
  import Sandwich from "@lucide/svelte/icons/sandwich"
  import Sparkles from "@lucide/svelte/icons/sparkles"
  import { kicker, type RecipeSummary } from "../lib/recipe"

  interface Props {
    recipe: Pick<
      RecipeSummary,
      "id" | "title" | "image" | "totalTime" | "recipeCategory" | "recipeCuisine"
    >
    selectable?: boolean
    selected?: boolean
    ontoggle?: (id: number) => void
    /** Why it's suggested, under the title (Try next). */
    reason?: string
    /** The reason was written by the AI layer. */
    aiReason?: boolean
  }

  let {
    recipe,
    selectable = false,
    selected = false,
    ontoggle,
    reason,
    aiReason = false,
  }: Props = $props()
  let imageFailed = $state(false)

  // A stable, friendly placeholder colour and icon per recipe when there's no photo
  const PLACEHOLDERS = [
    "bg-tomato-100 text-tomato-500 dark:bg-tomato-950/60",
    "bg-amber-100 text-amber-600 dark:bg-amber-950/60",
    "bg-lime-100 text-lime-700 dark:bg-lime-950/60",
    "bg-sky-100 text-sky-600 dark:bg-sky-950/60",
    "bg-rose-100 text-rose-500 dark:bg-rose-950/60",
    "bg-violet-100 text-violet-500 dark:bg-violet-950/60",
  ]
  const ICONS = [CookingPot, Soup, Salad, Croissant, CakeSlice, Sandwich]
  const Placeholder = $derived(ICONS[recipe.id % 6]!)
  const sub = $derived(kicker(recipe))
</script>

{#snippet body()}
  <div
    class={[
      "rounded-ui relative aspect-[4/3] w-full overflow-hidden shadow-sm transition duration-300 group-hover:-translate-y-0.5 group-hover:shadow-md",
      selected && "ring-primary ring-offset-canvas ring-3 ring-offset-2",
    ]}
  >
    {#if recipe.image && !imageFailed}
      <img
        src={recipe.image}
        alt={recipe.title}
        loading="lazy"
        decoding="async"
        referrerpolicy="no-referrer"
        class="size-full object-cover transition duration-500 group-hover:scale-[1.04]"
        onerror={() => (imageFailed = true)}
      />
    {:else}
      <div class={["flex size-full items-center justify-center", PLACEHOLDERS[recipe.id % 6]]}>
        <Placeholder class="size-12 opacity-80" />
      </div>
    {/if}
    {#if recipe.totalTime}
      <!-- Inset far enough to clear the card's rounded corner -->
      <span
        class="bg-canvas/90 absolute bottom-3 left-3 flex h-7 items-center gap-1 rounded-ui px-2.5 text-xs font-semibold shadow-sm backdrop-blur"
      >
        <Clock class="size-3.5" />{recipe.totalTime}
      </span>
    {/if}
    {#if selectable}
      <span
        class={[
          "absolute top-3 left-3 grid size-7 place-items-center rounded-full border-2 border-white shadow",
          selected ? "bg-primary" : "bg-black/20",
        ]}
      >
        {#if selected}<Check class="size-4 text-white" />{/if}
      </span>
    {/if}
  </div>
  <div class="px-1 pt-2.5">
    <h3 class="line-clamp-2 leading-snug font-semibold">{recipe.title}</h3>
    {#if sub}
      <p class="text-ink-muted mt-0.5 truncate text-sm">{sub}</p>
    {/if}
    {#if reason}
      <p class="text-ink-muted mt-1 line-clamp-2 text-sm">
        {#if aiReason}<Sparkles class="text-primary mr-0.5 inline size-3.5 align-[-2px]" />{/if}{reason}
      </p>
    {/if}
  </div>
{/snippet}

{#if selectable}
  <button
    type="button"
    class="group relative flex flex-col text-left"
    aria-pressed={selected}
    onclick={() => ontoggle?.(recipe.id)}
  >
    {@render body()}
  </button>
{:else}
  <a href={`/recipes/${recipe.id}`} class="group relative flex flex-col text-left">
    {@render body()}
  </a>
{/if}
