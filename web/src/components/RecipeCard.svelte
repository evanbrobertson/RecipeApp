<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import Sparkles from "@lucide/svelte/icons/sparkles"
  import Photo from "./Photo.svelte"
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
    /** The reason was written by Wee Chef. */
    aiReason?: boolean
    /** Replaces the category/time line (Fresh in the box: "Today · from a link"). */
    meta?: string
    /** The `sizes` attribute for the photo; the default suits a 2–4 column grid. */
    sizes?: string
  }

  let {
    recipe,
    selectable = false,
    selected = false,
    ontoggle,
    reason,
    aiReason = false,
    meta,
    sizes = "(min-width: 1024px) 18rem, (min-width: 640px) 30vw, 50vw",
  }: Props = $props()

  const sub = $derived(meta ?? [kicker(recipe), recipe.totalTime].filter(Boolean).join(" · "))
</script>

{#snippet body()}
  <div
    class={[
      "rounded-ctl relative aspect-[4/3] w-full overflow-hidden",
      selected && "ring-tile ring-offset-canvas ring-3 ring-offset-2",
    ]}
  >
    <Photo
      {recipe}
      width={300}
      {sizes}
      alt={recipe.title}
      class="size-full transition duration-500 group-hover:scale-[1.03]"
    />
    {#if selectable}
      <span
        class={[
          "absolute top-2.5 left-2.5 grid size-7 place-items-center rounded-full border-2 border-white shadow",
          selected ? "bg-tile" : "bg-black/25",
        ]}
      >
        {#if selected}<Check class="text-on-tile size-4" />{/if}
      </span>
    {/if}
  </div>
  <div class="pt-2.5">
    <h3 class="line-clamp-2 text-base leading-tight font-bold">{recipe.title}</h3>
    {#if reason}
      <p class="text-ink-muted mt-1 line-clamp-2 text-sm">
        {#if aiReason}<Sparkles class="text-primary mr-0.5 inline size-3.5 align-[-2px]" />{/if}{reason}
      </p>
    {:else if sub}
      <p class="meta mt-0.5 truncate">{sub}</p>
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
