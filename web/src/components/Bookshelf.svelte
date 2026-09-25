<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus"
  import CookbookSpine from "./CookbookSpine.svelte"
  import { bookSize, stackBooks, type ShelfBook } from "../lib/books"

  interface Props {
    books: ShelfBook[]
    pulledId?: number | null
    addable?: boolean
    /** Compact: one row of short towers that scrolls sideways instead of more shelves */
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

  let width = $state(0)
  const w = $derived(width || 900)

  /** Phones get one tall tower per shelf, centred */
  const phone = $derived(!single && w < 560)
  /** The length of the longest book in a tower */
  const towerLen = $derived(single ? 172 : phone ? Math.min(300, w - 88) : 180)
  // Room for a tower: its books, a nudge and a slide-out either side, and the gap
  const SLOT_EXTRA = 36
  const PER_TOWER = { compact: 4, min: 4, max: 7, phone: 10 }

  // Books stack into towers; towers stand side by side on shelves (planks)
  const shelves = $derived.by(() => {
    const n = books.length
    if (!n) return addable ? [[[]]] : []
    let towers: ShelfBook[][]
    let perShelf: number
    if (single) {
      towers = stackBooks(books, Math.ceil(n / PER_TOWER.compact))
      perShelf = towers.length
    } else if (phone) {
      towers = stackBooks(books, Math.ceil(n / PER_TOWER.phone))
      perShelf = 1
    } else {
      const fit = Math.max(1, Math.floor((w - 48) / (towerLen + SLOT_EXTRA)))
      // 4 to 7 books a tower, spread over as many towers as fit
      const count = Math.max(
        Math.ceil(n / PER_TOWER.max),
        Math.min(fit, Math.ceil(n / PER_TOWER.min)),
      )
      towers = stackBooks(books, count)
      perShelf = fit
    }
    const out: ShelfBook[][][] = []
    for (let i = 0; i < towers.length; i += perShelf) out.push(towers.slice(i, i + perShelf))
    return out
  })

  const lastShelf = $derived(shelves.length - 1)
  const lastTower = $derived((shelves[lastShelf]?.length ?? 1) - 1)

  const height = (tower: ShelfBook[]) => tower.reduce((sum, b) => sum + bookSize(b).thickness, 0)

  /**
   * The herb pot sits on the shortest tower of the first shelf (not the one holding the new-book
   * slot, unless that is the only one).
   */
  const potTower = $derived.by(() => {
    const first = shelves[0] ?? []
    const skip = addable && lastShelf === 0 && first.length > 1 ? lastTower : -1
    let best = -1
    first.forEach((tower, i) => {
      if (i !== skip && (best < 0 || height(tower) < height(first[best]!))) best = i
    })
    return best
  })
</script>

{#snippet pot()}
  <!-- A little herb pot, because why not -->
  <svg class="plant" viewBox="0 0 60 90" aria-hidden="true">
    <path d="M30 52 C 18 38, 8 36, 4 24 C 16 24, 26 32, 30 46" fill="#a9c4ae" />
    <path d="M30 52 C 42 36, 52 32, 57 18 C 44 20, 34 30, 30 46" fill="#2f6b4f" />
    <path d="M30 54 C 26 36, 28 20, 34 8 C 38 22, 36 38, 31 52" fill="#3b7a5a" />
    <path d="M14 58 h32 l-4 32 h-24 z" fill="#a55a40" />
    <rect x="11" y="52" width="38" height="8" fill="#8f4c35" />
  </svg>
{/snippet}

<!-- Sits on a tint backdrop; the planks run to its edges -->
<div
  bind:clientWidth={width}
  class={["shelves", single && "single", phone && "phone"]}
  style:--tower-len={`${towerLen}px`}
>
  {#each shelves as shelf, si (si)}
    <div class="shelf">
      <div class="towers">
        {#each shelf as tower, ti (ti)}
          <div class="tower">
            {#if si === 0 && ti === potTower}
              {@render pot()}
            {/if}
            {#each tower as book, bi (book.id)}
              <CookbookSpine
                {book}
                atFoot={bi === tower.length - 1}
                pulled={pulledId === book.id}
                onopen={() => onopen(book)}
              />
            {/each}
            <!-- After the books, so it's the last stop in tab order; drawn on top of the stack -->
            {#if addable && si === lastShelf && ti === lastTower}
              <button type="button" class="new-book" aria-label="New cookbook" onclick={onadd}>
                <Plus class="size-5" />
              </button>
            {/if}
          </div>
        {/each}
      </div>
      <div class="plank"></div>
    </div>
  {/each}
</div>

<style>
  .shelves {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .single .shelf {
    width: max-content;
    min-width: 100%;
  }
  .towers {
    display: flex;
    align-items: flex-end;
    justify-content: space-evenly;
    gap: 24px;
    padding: 0 24px;
  }
  .single .towers {
    justify-content: flex-start;
    gap: 28px;
    padding: 0 20px;
  }
  @media (min-width: 640px) {
    .single .towers {
      justify-content: space-evenly;
    }
  }
  .phone .towers {
    justify-content: center;
    padding: 0 16px;
  }
  /* A stack of books lying flat, listed top to bottom */
  .tower {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: calc(var(--tower-len) + 16px);
  }
  /* A flat tile-green plank */
  .plank {
    height: 12px;
    background: var(--tile);
  }
  /* A dashed outline of a book, waiting on top of the last stack */
  .new-book {
    order: -1;
    display: grid;
    place-items: center;
    width: calc(var(--tower-len) * 0.82);
    height: 44px;
    flex: none;
    margin-bottom: 4px;
    border: 2px dashed var(--border-accented);
    border-radius: 3px;
    color: var(--text-muted);
    transform: translateX(-4px) rotate(-1.5deg);
    transition:
      color 0.18s ease-out,
      border-color 0.18s ease-out,
      transform 0.18s ease-out;
  }
  .new-book:hover,
  .new-book:focus-visible {
    color: var(--primary);
    border-color: var(--primary);
    transform: translateX(-4px) translateY(-3px) rotate(-1.5deg);
  }
  .plant {
    order: -2;
    width: 44px;
    height: 66px;
    flex: none;
    /* Off-centre, as if someone put it down in passing */
    margin: 0 0 -1px 28%;
  }
</style>
