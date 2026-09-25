<script lang="ts">
  import {
    bookLean,
    bookPalette,
    bookSize,
    edgeShadow,
    spineBands,
    type ShelfBook,
  } from "../lib/books"

  interface Props {
    book: ShelfBook
    /** At the foot of its tower, so it sits flatter on the plank */
    atFoot?: boolean
    pulled?: boolean
    onopen: () => void
  }
  let { book, atFoot = false, pulled = false, onopen }: Props = $props()

  const palette = $derived(bookPalette(book.color))
  const size = $derived(bookSize(book))
  const lean = $derived(bookLean(book, atFoot))
  // Only six cover colours, so some spines get bands to keep the stack from looking cloned
  const bands = $derived(spineBands(book))
</script>

<!-- A book lying flat with its spine to the viewer: the title reads left to right -->
<button
  type="button"
  class={["book", pulled && "pulled", bands && "banded"]}
  style:--len={size.length}
  style:--title={`${size.title}px`}
  style:--thick={`${size.thickness}px`}
  style:--tilt={`${lean.tilt}deg`}
  style:--nudge={`${lean.nudge}px`}
  style:--cloth={palette.cloth}
  style:--shade={palette.shade}
  style:--foil={palette.foil}
  style:--edge={edgeShadow(palette)}
  style:--band={bands ?? "transparent"}
  aria-label={`Open ${book.name}, ${book.recipeCount} recipes`}
  onclick={onopen}
>
  <span class="title font-serif">{book.name}</span>
</button>

<style>
  .book {
    position: relative;
    display: flex;
    align-items: center;
    /* Its own length, stretched to fit the title, but never much past the tower's longest */
    width: min(
      calc(var(--tower-len, 180px) * 1.06),
      max(calc(var(--tower-len, 180px) * var(--len)), var(--title))
    );
    height: var(--thick);
    flex: none;
    padding: 0 14px;
    /* Square board edges, the boards a shade darker than the spine cloth */
    border-radius: 3px;
    color: var(--foil);
    background: var(--cloth);
    /* A faint hairline in the text colour keeps dark covers visible on a dark shelf */
    box-shadow:
      var(--edge),
      inset 0 -3px 0 var(--shade),
      inset 0 0 0 1px color-mix(in srgb, var(--text) 9%, transparent);
    transform: translateX(var(--nudge)) rotate(var(--tilt));
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
    cursor: pointer;
    outline: none;
  }
  .book.banded {
    padding: 0 26px;
    /* Two 2px bands, 10px in from each end */
    background:
      linear-gradient(var(--band), var(--band)) 10px 0 / 2px 100% no-repeat,
      linear-gradient(var(--band), var(--band)) calc(100% - 10px) 0 / 2px 100% no-repeat,
      var(--cloth);
  }
  /* Slide out of the stack a little, towards the reader's hand */
  .book:hover,
  .book:focus-visible {
    z-index: 1;
    transform: translateX(calc(var(--nudge) + 14px)) rotate(calc(var(--tilt) * 0.4));
  }
  .book:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .book.pulled {
    z-index: 1;
    transform: translateX(calc(var(--nudge) + 40px)) rotate(0deg);
  }
  .title {
    min-width: 0;
    font-size: 14px;
    line-height: 1.15;
    /* Sit a hair above the darker board edge */
    padding-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
