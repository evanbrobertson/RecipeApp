<script lang="ts">
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import { bookPalette, bookSize, seeded, type ShelfBook } from "../lib/books"

  let { book, pulled = false, onopen }: { book: ShelfBook; pulled?: boolean; onopen: () => void } =
    $props()

  const palette = $derived(bookPalette(book.color))
  const size = $derived(bookSize(book))
  // Alternate spine designs so the shelf doesn't look cloned
  const variant = $derived(Math.floor(seeded(book.id, 3) * 3))
</script>

<button
  type="button"
  class={["book", pulled && "pulled"]}
  style:--w={`${size.width}px`}
  style:--h={`${size.height}px`}
  style:--d={`${size.depth}px`}
  style:--cloth={palette.cloth}
  style:--shade={palette.shade}
  style:--foil={palette.foil}
  aria-label={`Open ${book.name}, ${book.recipeCount} recipes`}
  onclick={onopen}
>
  <span class="face spine" data-style={variant}>
    <span class="band"></span>
    <span class="title font-serif">{book.name}</span>
    <span class="band"></span>
    <ChefHat class="emblem" />
  </span>
  <span class="face cover">
    <span class="cover-title font-serif">{book.name}</span>
  </span>
  <span class="face back"></span>
  <span class="face pages-top"></span>
</button>

<style>
  .book {
    position: relative;
    width: var(--w);
    height: var(--h);
    flex: none;
    transform-style: preserve-3d;
    transition: transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
    cursor: pointer;
    outline: none;
  }
  .book:hover,
  .book:focus-visible {
    transform: translateZ(36px) translateY(-10px) rotateY(-22deg);
  }
  .book.pulled {
    transform: translateZ(140px) translateY(-40px) rotateY(-70deg);
  }

  .face {
    position: absolute;
    backface-visibility: hidden;
    /* Only the spine takes the pointer, so a turned book never blocks its neighbours */
    pointer-events: none;
  }

  /* The spine faces the viewer */
  .spine {
    pointer-events: auto;
    inset: 0;
    transform: translateZ(calc(var(--d) / 2));
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 14px 0 10px;
    border-radius: 3px 3px 2px 2px;
    color: var(--foil);
    background:
      linear-gradient(
        90deg,
        rgb(0 0 0 / 0.28),
        transparent 18%,
        rgb(255 255 255 / 0.14) 45%,
        transparent 60%,
        rgb(0 0 0 / 0.3)
      ),
      var(--cloth);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  .spine[data-style="1"] {
    background:
      linear-gradient(
        90deg,
        rgb(0 0 0 / 0.28),
        transparent 18%,
        rgb(255 255 255 / 0.14) 45%,
        transparent 60%,
        rgb(0 0 0 / 0.3)
      ),
      linear-gradient(var(--shade) 0 18%, var(--cloth) 18% 82%, var(--shade) 82%);
  }
  .spine[data-style="2"] {
    background:
      linear-gradient(
        90deg,
        rgb(0 0 0 / 0.28),
        transparent 18%,
        rgb(255 255 255 / 0.14) 45%,
        transparent 60%,
        rgb(0 0 0 / 0.3)
      ),
      repeating-linear-gradient(0deg, transparent 0 6px, rgb(0 0 0 / 0.05) 6px 7px), var(--cloth);
  }
  .band {
    width: 70%;
    height: 5px;
    border-block: 1px solid var(--foil);
    opacity: 0.8;
  }
  .title {
    flex: 1;
    margin: 10px 0;
    writing-mode: vertical-rl;
    text-orientation: mixed;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.02em;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-height: calc(var(--h) - 80px);
    text-shadow: 0 1px 0 rgb(0 0 0 / 0.25);
  }
  .spine :global(.emblem) {
    width: 16px;
    height: 16px;
    margin-top: 6px;
    opacity: 0.85;
  }

  /* Front cover: revealed as the book turns */
  .cover {
    top: 0;
    left: calc((var(--w) - var(--d)) / 2);
    width: var(--d);
    height: var(--h);
    transform: rotateY(90deg) translateZ(calc(var(--w) / 2));
    background: linear-gradient(90deg, rgb(0 0 0 / 0.3), transparent 12%), var(--cloth);
    display: grid;
    place-items: center;
    padding: 14px;
    border-radius: 0 4px 4px 0;
  }
  .cover-title {
    color: var(--foil);
    font-size: 16px;
    font-weight: 600;
    text-align: center;
    border: 1px solid var(--foil);
    padding: 10px 8px;
    border-radius: 3px;
  }
  .back {
    top: 0;
    left: calc((var(--w) - var(--d)) / 2);
    width: var(--d);
    height: var(--h);
    transform: rotateY(-90deg) translateZ(calc(var(--w) / 2));
    background: var(--shade);
  }
  /* Page edges seen from above */
  .pages-top {
    left: 0;
    top: calc((var(--h) - var(--d)) / 2);
    width: var(--w);
    height: var(--d);
    transform: rotateX(90deg) translateZ(calc(var(--h) / 2));
    background: repeating-linear-gradient(90deg, #efe6d4 0 2px, #e2d6bf 2px 3px);
    border: 2px solid var(--cloth);
  }
</style>
