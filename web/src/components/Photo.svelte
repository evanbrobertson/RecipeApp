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
  let failed = $state(false)
  let el: HTMLImageElement | undefined = $state()
  $effect(() => {
    // A different image (edit, shuffle) gets a fresh try, fallback included
    void recipe.image
    failed = false
    if (el) delete el.dataset.fellBack
  })
</script>

{#if img && !failed}
  <img
    bind:this={el}
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
      if (!fallbackSrc(e.currentTarget as HTMLImageElement, img.fallback)) failed = true
    }}
  />
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
