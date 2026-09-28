<script lang="ts">
  /**
   * A recipe photo sized for its slot (see lib/img.ts), falling back to the original URL
   * and then to a striped placeholder. `data-photo` lets lib/transitions.ts morph it into
   * the recipe page's hero.
   */
  import CakeSlice from "@lucide/svelte/icons/cake-slice"
  import CookingPot from "@lucide/svelte/icons/cooking-pot"
  import Croissant from "@lucide/svelte/icons/croissant"
  import Salad from "@lucide/svelte/icons/salad"
  import Sandwich from "@lucide/svelte/icons/sandwich"
  import Soup from "@lucide/svelte/icons/soup"
  import { fallbackSrc, sized } from "../lib/img"

  interface Props {
    recipe: { id: number; image?: string | null }
    /** About how wide it's shown, in CSS pixels (picks the 1x/2x files). */
    width: number
    /** The `sizes` attribute. Defaults to `{width}px`. */
    sizes?: string
    alt?: string
    class?: string
    /** Above the fold: load now, at high priority. */
    eager?: boolean
    /** The recipe page's hero (the other end of the photo morph). */
    hero?: boolean
    /** Icon size in the placeholder. */
    iconClass?: string
  }

  let {
    recipe,
    width,
    sizes,
    alt = "",
    class: klass = "",
    eager = false,
    hero = false,
    iconClass = "size-10",
  }: Props = $props()

  const ICONS = [CookingPot, Soup, Salad, Croissant, CakeSlice, Sandwich]
  const Placeholder = $derived(ICONS[recipe.id % ICONS.length]!)
  const img = $derived(sized(recipe, width))
  // Which image gave up, so a different one (edit, shuffle) gets a fresh try. Derived, not
  // reset in an effect: an effect that saw the <img> go would bring it back to fail again,
  // over and over, for as long as the page is open
  let failedImage = $state<string | null>(null)
  const failed = $derived(!!recipe.image && failedImage === recipe.image)
</script>

{#if img && !failed}
  <!-- Keyed so a new image gets a new element, with its fallback not yet used -->
  {#key recipe.image}
  <img
    src={img.src}
    srcset={img.srcset}
    sizes={sizes ?? `${width}px`}
    {alt}
    loading={eager ? "eager" : "lazy"}
    fetchpriority={eager ? "high" : "auto"}
    decoding={eager ? "sync" : "async"}
    referrerpolicy="no-referrer"
    data-photo={hero ? undefined : recipe.id}
    data-photo-hero={hero ? recipe.id : undefined}
    class={["object-cover", klass]}
    onerror={(e) => {
      if (!fallbackSrc(e.currentTarget as HTMLImageElement, img.fallback))
        failedImage = recipe.image ?? null
    }}
  />
  {/key}
{:else}
  <div
    class={["photo-empty grid place-items-center", klass]}
    data-photo={hero ? undefined : recipe.id}
    data-photo-hero={hero ? recipe.id : undefined}
    role={alt ? "img" : undefined}
    aria-label={alt || undefined}
  >
    <Placeholder class={[iconClass, "opacity-70"]} />
  </div>
{/if}
