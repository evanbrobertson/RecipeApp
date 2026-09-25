<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus"
  import CookbookSpine from "./CookbookSpine.svelte"
  import { bookSize, type ShelfBook } from "../lib/books"

  interface Props {
    books: ShelfBook[]
    pulledId?: number | null
    addable?: boolean
    /** One row that scrolls sideways instead of wrapping onto more shelves */
    single?: boolean
    onopen: (book: ShelfBook) => void
    onadd?: () => void
  }
  let {
    books,
    pulledId = null,
    addable = false,
    single = false,
    onopen,
    onadd,
  }: Props = $props()

  // Pack books into shelf rows that fit the available width
  let width = $state(0)

  const GAP = 4
  const ADD_SLOT = 54
  const rows = $derived.by(() => {
    if (single) return [books]
    const available = Math.max(260, (width || 900) - 48)
    const out: ShelfBook[][] = [[]]
    let used = 0
    for (const book of books) {
      const w = bookSize(book).width + GAP
      if (used + w > available && out[out.length - 1]!.length) {
        out.push([])
        used = 0
      }
      out[out.length - 1]!.push(book)
      used += w
    }
    // Keep room for the "new cookbook" slot on the last row
    if (addable && used + ADD_SLOT > available) out.push([])
    return out
  })

  // Look down on the middle of each row's books rather than the middle of the shelf, so a
  // short row on a wide shelf isn't seen from far off to one side
  function eye(row: ShelfBook[], last: boolean) {
    let span = row.reduce((sum, b) => sum + bookSize(b).width + GAP, 0)
    if (addable && last) span += ADD_SLOT
    return `${20 + span / 2}px -30%`
  }
</script>

<!-- Sits on a tint backdrop; the planks run to its edges -->
<div bind:clientWidth={width} class={["space-y-5", single && "single"]}>
  {#each rows as row, ri (ri)}
    <div class="shelf-row" style:perspective-origin={eye(row, ri === rows.length - 1)}>
      <div class="books">
        {#each row as book (book.id)}
          <CookbookSpine {book} pulled={pulledId === book.id} onopen={() => onopen(book)} />
        {/each}
        {#if addable && ri === rows.length - 1}
          <button type="button" class="new-book" aria-label="New cookbook" onclick={onadd}>
            <Plus class="size-5" />
          </button>
        {/if}
        {#if ri === 0}
          <!-- A little herb pot on the first shelf, because why not -->
          <svg class="plant" viewBox="0 0 60 90" aria-hidden="true">
            <path d="M30 52 C 18 38, 8 36, 4 24 C 16 24, 26 32, 30 46" fill="#a9c4ae" />
            <path d="M30 52 C 42 36, 52 32, 57 18 C 44 20, 34 30, 30 46" fill="#2f6b4f" />
            <path d="M30 54 C 26 36, 28 20, 34 8 C 38 22, 36 38, 31 52" fill="#3b7a5a" />
            <path d="M14 58 h32 l-4 32 h-24 z" fill="#a55a40" />
            <rect x="11" y="52" width="38" height="8" fill="#8f4c35" />
          </svg>
        {/if}
      </div>
      <div class="plank"></div>
    </div>
  {/each}
</div>

<style>
  .shelf-row {
    position: relative;
    perspective: 1400px;
    perspective-origin: 50% -30%;
  }
  .single .shelf-row {
    width: max-content;
    min-width: 100%;
  }
  .books {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    min-height: 168px;
    padding: 0 16px;
    transform-style: preserve-3d;
  }
  @media (min-width: 640px) {
    .books {
      padding: 0 24px;
    }
  }
  /* A flat tile-green plank */
  .plank {
    height: 12px;
    background: var(--tile);
  }
  .new-book {
    display: grid;
    place-items: center;
    width: 44px;
    height: 124px;
    flex: none;
    margin-left: 8px;
    border: 2px dashed var(--border-accented);
    border-bottom: 0;
    border-radius: 3px 3px 0 0;
    color: var(--text-muted);
    transition:
      color 0.18s ease-out,
      border-color 0.18s ease-out,
      transform 0.18s ease-out;
  }
  .new-book:hover {
    color: var(--primary);
    border-color: var(--primary);
    transform: translateY(-4px);
  }
  .plant {
    width: 44px;
    height: 66px;
    margin-left: auto;
    padding-left: 12px;
    box-sizing: content-box;
    flex: none;
  }
</style>
