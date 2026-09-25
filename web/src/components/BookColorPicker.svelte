<script lang="ts">
  import { BOOK_COLORS, bookPalette } from "../lib/books"
  let { value = $bindable("tomato") }: { value?: string } = $props()
</script>

<div class="flex flex-wrap gap-2" role="radiogroup" aria-label="Cover colour">
  {#each BOOK_COLORS as c (c)}
    <button
      type="button"
      role="radio"
      aria-checked={value === c}
      aria-label={c}
      class={[
        "rounded-ui size-9 shadow-sm transition hover:-translate-y-0.5",
        value === c && "ring-ink ring-offset-paper ring-3 ring-offset-2",
      ]}
      style:background={bookPalette(c).cloth}
      onclick={() => (value = c)}
    ></button>
  {/each}
</div>
