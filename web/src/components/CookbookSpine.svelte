<script lang="ts">
  import { bookPalette, bookSize, spineBands, type ShelfBook } from "../lib/books"

  let { book, pulled = false, onopen }: { book: ShelfBook; pulled?: boolean; onopen: () => void } =
    $props()

  const palette = $derived(bookPalette(book.color))
  const size = $derived(bookSize(book))
  // Only six cover colours, so some spines get bands to keep the shelf from looking cloned
  const bands = $derived(spineBands(book))
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
  style:--edge={palette.border}
  style:--band={bands ?? "transparent"}
  aria-label={`Open ${book.name}, ${book.recipeCount} recipes`}
  onclick={onopen}
>
  <span class={["face spine", bands && "banded"]}>
    <span class="title font-serif">{book.name}</span>
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
    transition: transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
    cursor: pointer;
    outline: none;
  }
  .book:hover,
  .book:focus-visible {
    transform: translateZ(30px) translateY(-8px) rotateY(-22deg);
  }
  .book:focus-visible .spine {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .book.pulled {
    transform: translateZ(120px) translateY(-32px) rotateY(-70deg);
  }

  .face {
    position: absolute;
    backface-visibility: hidden;
    /* Only the spine takes the pointer, so a turned book never blocks its neighbours */
    pointer-events: none;
  }

  /* The spine faces the viewer: flat cloth, square at the foot */
  .spine {
    pointer-events: auto;
    inset: 0;
    transform: translateZ(calc(var(--d) / 2));
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 0;
    border-radius: 3px 3px 0 0;
    color: var(--foil);
    background: var(--cloth);
    box-shadow: var(--edge);
  }
  /* Two 2px bands, 10px in from the top and bottom */
  .spine.banded {
    padding: 22px 0;
    background:
      linear-gradient(var(--band), var(--band)) 0 10px / 100% 2px no-repeat,
      linear-gradient(var(--band), var(--band)) 0 calc(100% - 10px) / 100% 2px no-repeat,
      var(--cloth);
  }
  .title {
    /* Vertical, reading bottom to top */
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-size: 14px;
    line-height: 1.1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-height: 100%;
  }

  /* Front cover: revealed as the book turns */
  .cover {
    top: 0;
    left: calc((var(--w) - var(--d)) / 2);
    width: var(--d);
    height: var(--h);
    transform: rotateY(90deg) translateZ(calc(var(--w) / 2));
    background: var(--cloth);
    box-shadow: var(--edge);
    display: grid;
    place-items: center;
    padding: 12px;
    border-radius: 0 3px 0 0;
  }
  .cover-title {
    color: var(--foil);
    font-size: 15px;
    line-height: 1.15;
    text-align: center;
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
    background: repeating-linear-gradient(90deg, #f8f3e6 0 2px, #e3dfd0 2px 3px);
    border: 2px solid var(--cloth);
  }
</style>
