<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus"
  import BookSpine from "./BookSpine.svelte"
  import { bookPalette, type ShelfBook } from "../lib/books"
  import {
    layoutShelf,
    ROW_HEIGHT,
    SHELF,
    type BookOrigin,
    type Placed,
    type PlacedBook,
  } from "../lib/shelf"

  interface Props {
    books: ShelfBook[]
    /** The book that's off the shelf (open): its place stays empty until it's back */
    pulledId?: number | null
    addable?: boolean
    /** Compact: one shelf that scrolls sideways instead of more shelves */
    single?: boolean
    onopen: (book: ShelfBook, from?: BookOrigin) => void
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

  /** Room above the first shelf's tallest book */
  const TOP = 14

  let width = $state(0)
  const rows = $derived(width || single ? layoutShelf(books, { width, single, addable }) : [])
  const height = $derived(TOP + rows.length * ROW_HEIGHT)
  const scrollWidth = $derived(single ? Math.max(...rows.map((r) => r.width), 0) : 0)

  /** Top of an item's box, from the shelves' top */
  const top = (row: number, y: number, h: number) =>
    TOP + row * ROW_HEIGHT + SHELF.clearance + 4 - y - h

  // Books glide to new places on a resize or a new book, but not into their first ones
  let ready = $state(false)
  $effect(() => {
    if (!rows.length || ready) return
    const raf = requestAnimationFrame(() => requestAnimationFrame(() => (ready = true)))
    return () => cancelAnimationFrame(raf)
  })

  // A book back from being open settles into its place
  let landed = $state<number | null>(null)
  let last: number | null = null
  $effect(() => {
    const now = pulledId
    if (last !== null && now === null) {
      landed = last
      setTimeout(() => (landed = null), 600)
    }
    last = now
  })

  /**
   * The book whose place in a stack is empty: what lay on it settles onto the book below once it
   * has gone, and lifts again as it comes back (OpenBook calls `back`).
   */
  let gap = $state<number | null>(null)
  $effect(() => {
    const id = pulledId
    if (id === null) {
      gap = null
      return
    }
    const t = setTimeout(() => (gap = id), 180)
    return () => clearTimeout(t)
  })
  const drops = $derived.by(() => {
    const out = new Map<string, number>()
    for (const [ri, row] of rows.entries()) {
      const g = row.items.find((it) => it.kind === "book" && it.book.id === gap)
      if (g?.kind !== "book" || g.stack === null) continue
      for (const it of row.items) {
        if (it.stack === g.stack && it.y > g.y) out.set(itemKey(it, ri), g.h)
      }
    }
    return out
  })
  const itemKey = (it: Placed, ri: number) =>
    it.kind === "book" ? `${it.book.id}` : `${it.kind}-${ri}`

  /** The lying book under the pointer: a pot on it slides out with it */
  let hovered = $state<number | null>(null)
  const hover = (id: number | null) => () => (hovered = id)

  function open(e: MouseEvent, spine: PlacedBook) {
    const el = (e.currentTarget as HTMLElement).querySelector<HTMLElement>(".spine")
    onopen(spine.book, el ? { el, spine, back: () => (gap = null) } : undefined)
  }
</script>

{#snippet pot()}
  <!-- A pot of basil, because why not -->
  <svg viewBox="0 0 46 68" aria-hidden="true">
    <path d="M23 41 C 16 35, 9 34, 2 36 C 8 43, 16 43, 23 41Z" fill="#a9c4ae" />
    <path d="M23 41 C 20 29, 13 22, 5 20 C 7 31, 14 37, 23 41Z" fill="#6f9f7e" />
    <path d="M23 41 C 26 27, 33 19, 42 17 C 40 29, 32 37, 23 41Z" fill="#3b7a5a" />
    <path d="M23 41 C 19 25, 21 11, 27 2 C 31 15, 29 29, 23 41Z" fill="#2f6b4f" />
    <path d="M23 41 C 30 34, 37 33, 44 35 C 38 42, 30 43, 23 41Z" fill="#5e9473" />
    <path d="M27 4 C 25 16, 24 28, 23 41" stroke="#a9c4ae" stroke-width="0.8" fill="none" />
    <path d="M9 46 h28 l-3 22 h-22 z" fill="#b5654a" />
    <path d="M27 46 h10 l-3 22 h-8 z" fill="#8f4c35" opacity="0.55" />
    <rect x="6" y="39" width="34" height="8" rx="1.5" fill="#c4704f" />
    <rect x="6" y="45" width="34" height="2" fill="#8f4c35" opacity="0.6" />
    <rect x="8.5" y="40.5" width="7" height="4" rx="1" fill="#fff" opacity="0.16" />
  </svg>
{/snippet}

{#snippet crock()}
  <!-- A stoneware crock of spoons -->
  <svg viewBox="0 0 52 96" aria-hidden="true">
    <path d="M27 54 L 24 22" stroke="#8a5a3a" stroke-width="3.5" stroke-linecap="round" />
    <rect x="17" y="4" width="13" height="20" rx="4" fill="#a8744d" transform="rotate(-5 23 14)" />
    <path d="M18 54 L 11 16" stroke="#c08f5a" stroke-width="4" stroke-linecap="round" />
    <ellipse cx="9" cy="10" rx="6" ry="9" fill="#d4a26b" transform="rotate(-11 9 10)" />
    <ellipse cx="8" cy="9" rx="2.5" ry="5" fill="#fff" opacity="0.18" transform="rotate(-11 8 9)" />
    <path d="M33 54 L 37 28" stroke="#2f6b4f" stroke-width="3.5" stroke-linecap="round" />
    <ellipse cx="39" cy="15" rx="7.5" ry="14" stroke="#9aa59f" stroke-width="1.4" fill="none" />
    <ellipse cx="39" cy="15" rx="3.5" ry="13" stroke="#9aa59f" stroke-width="1.2" fill="none" />
    <path
      d="M7 42 h38 q3 0 3 3 v42 q0 9 -9 9 h-26 q-9 0 -9 -9 v-42 q0 -3 3 -3 z"
      fill="#f1e9d6"
    />
    <path d="M38 42 h7 q3 0 3 3 v42 q0 9 -9 9 h-3 z" fill="#c9bea4" opacity="0.5" />
    <rect x="4" y="54" width="44" height="5" fill="#2f6b4f" />
    <rect x="4" y="62" width="44" height="1.6" fill="#2f6b4f" />
    <rect x="3" y="38" width="46" height="7" rx="3.5" fill="#e6dcc4" />
    <rect x="7" y="47" width="5" height="40" rx="2.5" fill="#fff" opacity="0.35" />
  </svg>
{/snippet}

{#snippet corbel(side: "left" | "right")}
  <svg class={["corbel", side]} viewBox="0 0 16 24" aria-hidden="true">
    <path d="M0 0 h16 v7 h-2 v5 h-2 v5 a4 4 0 0 1 -8 0 v-5 h-2 v-5 h-2 z" />
    <rect x="0" y="0" width="16" height="2" fill="#fff" opacity="0.12" />
  </svg>
{/snippet}

<!-- Tiled wall; the planks run to its edges -->
<div
  bind:clientWidth={width}
  class={["shelves", single && "single", ready && "ready"]}
  style:height={`${height}px`}
  style:width={single ? `${scrollWidth}px` : undefined}
>
  {#each rows as _, ri (ri)}
    <div class="plank" style:top={`${TOP + ri * ROW_HEIGHT + SHELF.clearance}px`}>
      {@render corbel("left")}
      {@render corbel("right")}
    </div>
  {/each}
  {#each rows as row, ri (ri)}
    {#each row.items as item (itemKey(item, ri))}
      {#if item.kind === "book"}
        <button
          type="button"
          class={[
            "book",
            item.standing ? "up" : "flat",
            pulledId === item.book.id && "pulled",
            landed === item.book.id && "landed",
          ]}
          style:transform={`translate(${item.x}px, ${top(ri, item.y, item.h)}px)`}
          style:width={`${item.w}px`}
          style:height={`${item.h}px`}
          style:z-index={item.z + 1}
          style:--cloth={bookPalette(item.book.color).cloth}
          aria-label={`Open ${item.book.name}, ${item.book.recipeCount} recipes`}
          onclick={(e) => open(e, item)}
          onpointerenter={hover(item.book.id)}
          onpointerleave={hover(null)}
          onfocus={hover(item.book.id)}
          onblur={hover(null)}
        >
          <span
            class="lean"
            style:translate={drops.has(itemKey(item, ri))
              ? `0 ${drops.get(itemKey(item, ri))}px`
              : undefined}
            style:rotate={item.tilt ? `${item.tilt}deg` : undefined}
            style:transform-origin={item.pivot === "center" ? "50% 50%" : `${item.pivot} bottom`}
          >
            <span class="lift">
              {#if item.top}<span class="edge"></span>{/if}
              <BookSpine spine={item} />
            </span>
          </span>
        </button>
      {:else if item.kind === "add"}
        <button
          type="button"
          class="thing new-book"
          aria-label="New cookbook"
          style:transform={`translate(${item.x}px, ${top(ri, item.y, item.h)}px)`}
          style:width={`${item.w}px`}
          style:height={`${item.h}px`}
          onclick={onadd}
        >
          <Plus class="size-5" />
        </button>
      {:else}
        <div
          class="thing prop"
          style:transform={`translate(${item.x}px, ${top(ri, item.y, item.h)}px)`}
          style:width={`${item.w}px`}
          style:height={`${item.h}px`}
          style:z-index={item.z + 1}
        >
          {#if item.kind === "pot"}
            {@const ride = item.on === hovered && item.on !== pulledId}
            {@const drop = drops.get(itemKey(item, ri)) ?? 0}
            <!-- Sits on its book: slides out with it, and drops when it's taken -->
            <span class="ride" style:translate={`${ride ? 16 : 0}px ${drop - (ride ? 1 : 0)}px`}>
              {#key `${ride}-${drop}`}
                <span class={["wobble", (ride || drop) && "go"]}>{@render pot()}</span>
              {/key}
            </span>
          {:else}{@render crock()}{/if}
        </div>
      {/if}
    {/each}
  {/each}
</div>

<style>
  /* A glossy tiled backsplash, quieter than the brand tile */
  .shelves {
    --wall: var(--tint);
    --grout: color-mix(in srgb, var(--tint) 84%, var(--text));
    --plank: var(--shelf-wood);
    position: relative;
    min-width: 100%;
    background-color: var(--wall);
    background-image:
      linear-gradient(var(--grout) 0 2px, transparent 2px),
      linear-gradient(90deg, var(--grout) 0 2px, transparent 2px),
      radial-gradient(130% 130% at 0% 0%, rgb(255 255 255 / 0.45), transparent 42%),
      linear-gradient(135deg, transparent 70%, rgb(0 0 0 / 0.03));
    background-size: 58px 58px;
    background-position: -1px -1px;
  }
  :global(.dark) .shelves {
    --grout: color-mix(in srgb, var(--tint) 70%, #000);
    background-image:
      linear-gradient(var(--grout) 0 2px, transparent 2px),
      linear-gradient(90deg, var(--grout) 0 2px, transparent 2px),
      radial-gradient(130% 130% at 0% 0%, rgb(255 255 255 / 0.07), transparent 42%),
      linear-gradient(135deg, transparent 70%, rgb(0 0 0 / 0.08));
  }

  /* A painted plank: its top face catching the light, then the front edge */
  .plank {
    position: absolute;
    left: 0;
    right: 0;
    height: 22px;
    background-color: var(--plank);
    background-image:
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--plank) 70%, #000) 0,
        color-mix(in srgb, var(--plank) 78%, #fff) 7px,
        color-mix(in srgb, var(--plank) 90%, #fff) 8px,
        color-mix(in srgb, var(--plank) 70%, #fff) 8px,
        var(--plank) 9px,
        var(--plank) calc(100% - 3px),
        color-mix(in srgb, var(--plank) 75%, #000) 100%
      ),
      var(--cloth-grain);
    background-blend-mode: normal, soft-light;
    box-shadow: 0 12px 14px -8px rgb(20 32 25 / 0.4);
  }
  .corbel {
    position: absolute;
    top: 22px;
    width: 16px;
    height: 24px;
    fill: color-mix(in srgb, var(--plank) 82%, #000);
    filter: drop-shadow(3px 4px 3px rgb(20 32 25 / 0.25));
  }
  .corbel.left {
    left: 9%;
  }
  .corbel.right {
    right: 9%;
  }

  .book,
  .thing {
    position: absolute;
    top: 0;
    left: 0;
    display: block;
    padding: 0;
  }
  .ready .book,
  .ready .thing {
    transition: transform 0.6s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .book {
    cursor: pointer;
    outline: none;
    -webkit-tap-highlight-color: transparent;
  }
  .lean,
  .lift {
    position: absolute;
    inset: 0;
    display: block;
  }
  .lift {
    transition:
      transform 0.5s cubic-bezier(0.3, 1.5, 0.5, 1),
      filter 0.3s ease-out;
    filter: drop-shadow(3px 3px 3px rgb(20 32 25 / 0.28));
  }
  .lift :global(.spine) {
    position: absolute;
    inset: 0;
  }
  /* The book's top edge, seen from a little above: pages between boards, or a lying book's cover */
  .edge {
    position: absolute;
    left: 1px;
    right: 1px;
    bottom: 100%;
    height: 5px;
    border-radius: 2px 2px 0 0;
  }
  .up .edge {
    background:
      linear-gradient(90deg, rgb(0 0 0 / 0.3) 0 2px, transparent 2px calc(100% - 2px), rgb(0 0 0 / 0.3) 0),
      linear-gradient(180deg, #d9cfb4, #f4eedd);
  }
  .flat .edge {
    left: 2px;
    right: 2px;
    background:
      linear-gradient(180deg, rgb(0 0 0 / 0.25), transparent 70%),
      color-mix(in srgb, var(--cloth, #2f6b4f) 80%, #fff);
  }

  /* Standing: up and out of the row, as if a finger tipped it by its head */
  .up:hover .lift,
  .up:focus-visible .lift {
    transform: translateY(-14px);
    filter: drop-shadow(6px 10px 8px rgb(20 32 25 / 0.3));
  }
  /* Its neighbours give way a hair */
  .up:has(+ .up:hover) .lift {
    transform: rotate(-1.2deg);
    transform-origin: left bottom;
  }
  .up:hover + .up .lift {
    transform: rotate(1.2deg);
    transform-origin: right bottom;
  }
  /* Lying: slid out of the stack, towards the reader's hand */
  .flat:hover .lift,
  .flat:focus-visible .lift {
    transform: translateX(16px) translateY(-1px);
    filter: drop-shadow(6px 6px 6px rgb(20 32 25 / 0.3));
  }
  .book:active .lift {
    transition-duration: 0.12s;
    transform: translateY(-6px) scale(0.98);
  }
  .flat:active .lift {
    transform: translateX(10px) scale(0.98);
  }
  .book:focus-visible .lift :global(.spine) {
    outline: 2px solid var(--primary);
    outline-offset: 3px;
  }
  /* Off the shelf: the open book is drawn by OpenBook */
  .pulled {
    visibility: hidden;
  }
  .landed .lift {
    animation: settle 0.5s cubic-bezier(0.3, 1.5, 0.5, 1);
  }
  @keyframes settle {
    from {
      transform: translateY(-8px);
    }
  }

  /* A dashed outline of a book, waiting at the end of the last shelf */
  .new-book {
    display: grid;
    place-items: center;
    border: 2px dashed color-mix(in srgb, var(--text) 30%, transparent);
    border-radius: 3px 3px 2px 2px;
    color: var(--text-muted);
    transition:
      transform 0.6s cubic-bezier(0.2, 0.8, 0.2, 1),
      color 0.18s ease-out,
      border-color 0.18s ease-out,
      translate 0.4s cubic-bezier(0.3, 1.5, 0.5, 1);
  }
  .new-book:hover,
  .new-book:focus-visible {
    color: var(--primary);
    border-color: var(--primary);
    translate: 0 -8px;
    outline: none;
  }
  .prop {
    pointer-events: none;
    filter: drop-shadow(3px 3px 3px rgb(20 32 25 / 0.22));
  }
  .ride,
  .wobble {
    display: block;
    width: 100%;
    height: 100%;
  }
  .ride {
    transition: translate 0.5s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .wobble {
    transform-origin: 50% 100%;
  }
  /* A pot that's been moved rocks on its base before it settles */
  .wobble.go {
    animation: wobble 0.8s ease-out;
  }
  @keyframes wobble {
    25% {
      rotate: -5deg;
    }
    50% {
      rotate: 3deg;
    }
    75% {
      rotate: -1.2deg;
    }
  }
  /* Settles onto the book below when the one under it is taken */
  .lean {
    transition: translate 0.45s cubic-bezier(0.5, 0, 0.55, 1.35);
  }
  .prop svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }
</style>
