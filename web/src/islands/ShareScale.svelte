<script lang="ts">
  /**
   * Scaling on a share page. The server rendered the ingredient lines (and the yield) as
   * plain HTML with their text in `data-scale-raw`; this rewrites them in place, so the
   * page still reads without JavaScript.
   */
  import ScaleControl from "../components/ScaleControl.svelte"
  import { scaleIngredient } from "../lib/ingredients"

  const lines = Array.from(document.querySelectorAll<HTMLElement>("[data-scale-raw]"))
  const hasIngredients = !!document.querySelector(".share-ingredients [data-scale-raw]")
  let scale = $state(1)

  $effect(() => {
    for (const el of lines) {
      const raw = el.dataset.scaleRaw ?? ""
      el.textContent = scale === 1 ? raw : scaleIngredient(raw, scale)
    }
  })
</script>

{#if hasIngredients}<ScaleControl bind:value={scale} />{/if}
