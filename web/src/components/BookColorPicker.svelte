<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import { BOOK_COLORS, bookColor, bookPalette } from "../lib/books"
  let { value = $bindable("tile") }: { value?: string } = $props()

  // Legacy names still pick out their nearest cover
  const current = $derived(bookColor(value))
</script>

<div class="flex flex-wrap gap-1" role="radiogroup" aria-label="Cover colour">
  {#each BOOK_COLORS as c (c)}
    {@const look = bookPalette(c)}
    <button
      type="button"
      role="radio"
      aria-checked={current === c}
      aria-label={c[0]!.toUpperCase() + c.slice(1)}
      class="swatch rounded-ctl"
      onclick={() => (value = c)}
    >
      <span class="cover" style:background={look.cloth} style:box-shadow={look.border === "none" ? null : look.border}></span>
      {#if current === c}
        <span class="check bg-tile text-on-tile"><Check class="size-3.5" strokeWidth={3} /></span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .swatch {
    position: relative;
    display: grid;
    place-items: end center;
    width: 48px;
    height: 56px;
    padding-bottom: 6px;
    transition: background-color 0.15s ease-out;
  }
  .swatch:hover {
    background: var(--tint);
  }
  /* A little book standing on its foot */
  .cover {
    width: 26px;
    height: 40px;
    border-radius: 3px 3px 0 0;
    /* A faint edge so the forest cover doesn't vanish into dark paper */
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 16%, transparent);
    transition: transform 0.18s ease-out;
  }
  .swatch:hover .cover,
  .swatch[aria-checked="true"] .cover {
    transform: translateY(-3px);
  }
  .check {
    position: absolute;
    top: 2px;
    right: 4px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 999px;
    box-shadow: 0 0 0 2px var(--paper);
  }
</style>
