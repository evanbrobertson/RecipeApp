<script lang="ts">
  /**
   * A cookbook taken off the shelf: it leaves its place spine first, turns to show its cover as it
   * comes to the middle, and the cover swings open on a table of contents. Closing puts it back.
   *
   * The closed book is a solid (cover and spine) as deep as its spine on the shelf is wide, so at
   * the start of the flight its spine sits exactly over the one on the shelf.
   */
  import { tick, untrack } from "svelte"
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import X from "@lucide/svelte/icons/x"
  import BookSpine from "./BookSpine.svelte"
  import { api } from "../lib/api"
  import { bookColor, bookPalette, edgeShadow, type ShelfBook } from "../lib/books"
  import { spineFor, type BookOrigin } from "../lib/shelf"
  import type { CookbookDetail } from "../lib/recipe"

  let {
    book,
    from = null,
    onclose,
  }: { book: ShelfBook | null; from?: BookOrigin | null; onclose: () => void } = $props()

  let visible = $state(false)
  /** The scrim is up */
  let shown = $state(false)
  let open = $state(false)
  let closing = $state(false)
  let details = $state<CookbookDetail | null>(null)
  let loading = $state(false)
  let origin = $state<BookOrigin | null>(null)
  /** The closed book's depth, px */
  let depth = $state(0)

  let flight = $state<HTMLElement>()
  let cover = $state<HTMLElement>()
  let flying: Promise<void> | null = null

  const palette = $derived(bookPalette(book?.color))
  const pale = $derived(["sage", "butter", "cream"].includes(bookColor(book?.color)))
  /** The spine as the shelf drew it, or as it would have */
  const spine = $derived(origin?.spine ?? (book ? spineFor(book) : null))
  const style = $derived(spine?.style ?? "plain")

  const PERSPECTIVE = 2200
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))

  $effect(() => {
    const current = book
    if (!current) return
    untrack(() => {
      origin = from
      visible = true
      open = false
      closing = false
      details = null
      loading = true
    })
    let live = true
    api<CookbookDetail>(`/api/cookbooks/${current.id}`)
      .then((d) => live && (details = d))
      .catch(() => live && (details = null))
      .finally(() => live && (loading = false))
    tick().then(async () => {
      if (!live) return
      shown = true
      flying = fly("in")
      // The cover starts to swing as the book settles, not after
      const swing = setTimeout(() => live && !closing && (open = true), reduced() ? 0 : 640)
      await flying
      flying = null
      if (!live || closing) clearTimeout(swing)
    })
    return () => (live = false)
  })

  /**
   * The transforms that put the closed book's spine over `o` on the shelf, for a flight wrapper
   * whose untransformed self has the closed book facing us in the middle.
   */
  function onShelf(o: BookOrigin) {
    if (!cover) return null
    const shelf = o.el.getBoundingClientRect()
    if (!shelf.width) return null
    const c = cover.getBoundingClientRect()
    const { standing, w, h, tilt } = o.spine
    const along = standing ? h : w
    const across = standing ? w : h
    const H = c.height
    depth = (H * across) / along
    const cx = c.left + c.width / 2
    const cy = c.top + c.height / 2
    const ox = innerWidth / 2
    const oy = innerHeight / 2
    // Turned to face us, the spine is half the cover's width nearer than the book's middle, so
    // the perspective draws it larger and further out; scale and place it to land exactly
    let s = along / H
    let f = 1
    for (let i = 0; i < 3; i++) {
      f = PERSPECTIVE / (PERSPECTIVE - (-depth / 2 + (s * c.width) / 2))
      s = along / (H * f)
    }
    const tx = ox + (shelf.left + shelf.width / 2 - ox) / f - cx
    const ty = oy + (shelf.top + shelf.height / 2 - oy) / f - cy
    return {
      origin: `${cx}px ${cy}px ${-depth / 2}px`,
      tx,
      ty,
      scale: `${s}`,
      transform: `rotateZ(${tilt}deg) rotateZ(${standing ? 0 : -90}deg) rotateY(90deg)`,
    }
  }

  async function fly(way: "in" | "out") {
    const el = flight
    const at = origin && el && !reduced() ? onShelf(origin) : null
    if (!el) return
    if (!at) {
      // Nowhere to fly from (or to): fade
      const a = el.animate([{ opacity: 0, scale: "0.96" }, { opacity: 1, scale: "1" }], {
        duration: reduced() ? 1 : 260,
        easing: "ease-out",
        direction: way === "in" ? "normal" : "reverse",
        fill: "both",
      })
      await a.finished
      a.cancel()
      return
    }
    el.style.transformOrigin = at.origin
    // Off the shelf (or back onto it) on a slight arc towards you; the turn lags the move going
    // out, and leads it coming back, as a hand would
    const moves = [
      { offset: 0, translate: `${at.tx}px ${at.ty}px 0px`, scale: at.scale },
      { offset: 0.4, translate: `${at.tx * 0.55}px ${at.ty * 0.55}px 120px` },
      { offset: 1, translate: "0px 0px 0px", scale: "1" },
    ]
    const turn = [{ transform: at.transform }, { transform: "rotateZ(0deg) rotateZ(0deg) rotateY(0deg)" }]
    const opts = (duration: number, delay: number, easing: string): KeyframeAnimationOptions => ({
      duration,
      delay,
      easing,
      fill: "both",
      direction: way === "in" ? "normal" : "reverse",
    })
    const animations =
      way === "in"
        ? [
            el.animate(moves, opts(720, 0, "cubic-bezier(0.25, 0.8, 0.25, 1)")),
            el.animate(turn, opts(620, 110, "cubic-bezier(0.5, 0, 0.2, 1)")),
          ]
        : [
            el.animate(moves, opts(620, 60, "cubic-bezier(0.6, 0, 0.6, 1)")),
            el.animate(turn, opts(560, 0, "cubic-bezier(0.6, 0, 0.3, 1)")),
          ]
    await Promise.all(animations.map((a) => a.finished))
    if (way === "in") animations.forEach((a) => a.cancel())
  }

  async function close() {
    if (closing || !visible) return
    closing = true
    if (flying) await flying
    if (open) {
      open = false
      await wait(reduced() ? 0 : 500)
    }
    shown = false
    // What sat on the book lifts as it nears its place
    const back = origin?.back
    const lift = setTimeout(() => back?.(), reduced() ? 0 : 260)
    await fly("out")
    clearTimeout(lift)
    back?.()
    flight?.getAnimations().forEach((a) => a.cancel())
    visible = false
    closing = false
    origin = null
    onclose()
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && book) close()
  }
</script>

<svelte:window {onkeydown} />

{#if visible && book && spine}
  <div class={["scrim", shown && "shown"]}></div>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    onclick={(e) => {
      const t = e.target as HTMLElement
      if (t === e.currentTarget || t === flight) close()
    }}
  >
    <div class="flight" bind:this={flight}>
      <div
        class={["stage", open && "open", closing && "closing", `s-${style}`, pale && "pale"]}
        style:--cloth={palette.cloth}
        style:--shade={palette.shade}
        style:--foil={palette.foil}
        style:--orn={palette.bands[0]}
        style:--edge={edgeShadow(palette)}
        style:--depth={`${depth}px`}
        role="dialog"
        aria-modal="true"
        aria-label={book.name}
      >
        <!-- Right page: contents -->
        <section class="page right">
          <header class="mb-4 flex items-baseline justify-between gap-2">
            <h2 class="section-title">Contents</h2>
            <span class="meta">{book.recipeCount} recipes</span>
          </header>
          {#if loading}
            <div class="space-y-3">
              {#each { length: 5 }, i (i)}<div class="skeleton h-4 w-full"></div>{/each}
            </div>
          {:else if !details?.recipes.length}
            <p class="text-ink-muted text-sm">
              Blank pages, for now. Add recipes from any recipe page or with “Select” on the recipes
              list.
            </p>
          {:else}
            <ol class="toc">
              {#each details.recipes as r, i (r.id)}
                <li style:--i={Math.min(i, 12)}>
                  <a href={`/recipes/${r.id}`} class="toc-link">
                    <span class="toc-title">{r.title}</span>
                    <span class="leader"></span>
                    <span class="toc-num">{i + 1}</span>
                  </a>
                </li>
              {/each}
            </ol>
          {/if}
          <footer class="mt-auto flex justify-end pt-4">
            <a href={`/cookbooks/${book.id}`} class="btn btn-soft">
              Open cookbook <ArrowRight />
            </a>
          </footer>
        </section>

        <!-- The spine, square to the cover: only seen while the book turns -->
        <div class="spine-face" aria-hidden="true">
          <div class="spine-scale" style:--k={depth && spine ? depth / (spine.standing ? spine.w : spine.h) : 1}>
            <BookSpine {spine} turned={!spine.standing} />
          </div>
        </div>

        <!-- The cover: front outside, endpaper inside -->
        <div class="cover" bind:this={cover}>
          <div class="cover-front">
            {#if style === "ends"}<span class="corner a"></span><span class="corner b"></span>{/if}
            <div class="frame">
              <ChefHat class="size-8" />
              <h2 class="cover-title font-serif text-[26px] leading-tight sm:text-[30px]">
                {book.name}
              </h2>
              <span class="rule"></span>
              <span class="text-[13px] font-bold tracking-[0.2em] uppercase">Recipes</span>
            </div>
            <span class="cover-shade"></span>
          </div>
          <div class="cover-inside">
            <p class="endpaper-muted text-[13px] font-bold tracking-[0.3em] uppercase">Ex libris</p>
            <h3 class="mt-3 font-serif text-[24px] leading-tight">{book.name}</h3>
            {#if book.description}<p class="mt-3 text-sm">{book.description}</p>{/if}
            <p class="endpaper-muted mt-auto text-[13px] font-semibold">
              A cookbook of {book.recipeCount} recipes
            </p>
            <span class="cover-shade"></span>
          </div>
        </div>

        <button type="button" class="close" aria-label="Close book" onclick={close}>
          <X class="size-[22px]" />
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgb(13 19 15 / 0.6);
    backdrop-filter: blur(6px);
    opacity: 0;
    transition: opacity 0.4s ease-out;
    pointer-events: none;
  }
  .scrim.shown {
    opacity: 1;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 61;
    perspective: 2200px;
  }
  .flight {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 16px;
    transform-style: preserve-3d;
  }
  .stage {
    --page-w: min(380px, 44vw);
    position: relative;
    width: calc(var(--page-w) * 2);
    height: min(560px, 80dvh);
    transform-style: preserve-3d;
    /* Closed: the cover (the right half) in the middle */
    transform: translateX(calc(var(--page-w) / -2));
    transition: transform 0.7s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .stage.open {
    transform: translateX(0);
  }
  .stage.closing {
    transition-duration: 0.5s;
  }
  .page {
    position: absolute;
    top: 0;
    width: var(--page-w);
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 28px 26px 20px;
    background: linear-gradient(90deg, rgb(0 0 0 / 0.08), transparent 8%), var(--paper);
    color: var(--text);
    border-radius: 0 var(--radius) var(--radius) 0;
    box-shadow: var(--menu-shadow);
    overflow: hidden;
    transform: translateZ(-1px);
  }
  /* The cover's shadow sweeping off the page as it opens */
  .page::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(270deg, rgb(20 30 24 / 0.28), transparent 70%);
    pointer-events: none;
    transition: opacity 0.6s 0.25s ease-out;
  }
  .open .page::after {
    opacity: 0;
  }
  .page.right {
    left: var(--page-w);
  }
  .toc {
    overflow-y: auto;
    margin-right: -8px;
    padding-right: 8px;
  }
  .open .toc li {
    animation: rise 0.4s calc(0.25s + var(--i) * 40ms) both cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes rise {
    from {
      opacity: 0;
      translate: 0 6px;
    }
  }
  .toc-link {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-height: 44px;
    padding: 10px 0;
    font-size: 1rem;
  }
  .toc-link:hover .toc-title {
    color: var(--primary);
  }
  .toc-title {
    font-family: var(--font-serif);
    transition: color 0.15s;
  }
  .leader {
    flex: 1;
    border-bottom: 2px dotted var(--border-accented);
    transform: translateY(-4px);
    min-width: 16px;
  }
  .toc-num {
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  /* Square to the cover at the hinge, running back into the page */
  .spine-face {
    position: absolute;
    top: -6px;
    left: calc(var(--page-w) - var(--depth));
    width: var(--depth);
    height: calc(100% + 12px);
    transform-origin: right center;
    transform: rotateY(-90deg);
    backface-visibility: hidden;
  }
  .spine-scale {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: 0 0;
    scale: var(--k);
  }

  .cover {
    position: absolute;
    top: -6px;
    left: var(--page-w);
    width: calc(var(--page-w) + 6px);
    height: calc(100% + 12px);
    transform-origin: left center;
    transform-style: preserve-3d;
    transition: transform 0.95s cubic-bezier(0.35, 0.65, 0.2, 1);
  }
  .stage.open .cover {
    transform: rotateY(-180deg);
  }
  .stage.closing .cover {
    transition-duration: 0.5s;
    transition-timing-function: cubic-bezier(0.5, 0, 0.3, 1);
  }
  .cover-front,
  .cover-inside {
    position: absolute;
    inset: 0;
    backface-visibility: hidden;
    border-radius: 0 var(--radius) var(--radius) 0;
    overflow: hidden;
  }
  .cover-front {
    display: grid;
    place-items: center;
    padding: 28px;
    color: var(--foil);
    /* Cloth over board: grain and weave, the hinge's groove, light from the upper left */
    background-color: var(--cloth);
    background-image:
      linear-gradient(90deg, rgb(0 0 0 / 0.3), rgb(255 255 255 / 0.08) 3%, rgb(0 0 0 / 0.14) 5%, transparent 9%),
      radial-gradient(120% 90% at 20% 0%, rgb(255 255 255 / 0.12), transparent 60%),
      linear-gradient(180deg, transparent 85%, rgb(0 0 0 / 0.12)),
      var(--cloth-grain),
      repeating-linear-gradient(0deg, rgb(255 255 255 / 0.05) 0 1px, transparent 1px 3px),
      repeating-linear-gradient(90deg, rgb(0 0 0 / 0.05) 0 1px, transparent 1px 3px);
    background-blend-mode: normal, normal, normal, soft-light, normal, normal;
    box-shadow:
      var(--edge),
      inset 0 0 0 1px rgb(0 0 0 / 0.12),
      var(--menu-shadow);
  }
  /* Two-tone books get contrasting corners */
  .corner {
    position: absolute;
    width: 92px;
    height: 92px;
    background-color: var(--orn);
    background-image: var(--cloth-grain);
    background-blend-mode: soft-light;
    rotate: 45deg;
  }
  .corner.a {
    top: -46px;
    right: -46px;
  }
  .corner.b {
    bottom: -46px;
    right: -46px;
  }
  .frame {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 100%;
    justify-content: center;
    text-align: center;
    border: 1.5px solid var(--foil);
    outline: 1px solid var(--foil);
    outline-offset: 5px;
    border-radius: 3px;
    padding: 20px;
    text-shadow: 0 -1px 0 rgb(0 0 0 / 0.3);
  }
  .pale .frame {
    text-shadow: 0 1px 0 rgb(255 255 255 / 0.4);
  }
  .s-rules .frame {
    border-color: var(--orn);
    outline-color: var(--orn);
  }
  /* Labelled books carry the title on a paper label on the cover too */
  .s-label .cover-title {
    padding: 14px 18px;
    color: #1c2b22;
    text-shadow: none;
    background: linear-gradient(160deg, rgb(255 255 255 / 0.35), transparent 40%, rgb(150 110 40 / 0.1)), #f3ead2;
    border-radius: 3px;
    box-shadow:
      inset 0 0 0 1px rgb(28 43 34 / 0.18),
      inset 0 0 0 4px #f3ead2,
      inset 0 0 0 5px rgb(28 43 34 / 0.28),
      0 1px 2px rgb(0 0 0 / 0.2);
    rotate: -0.8deg;
  }
  .rule {
    width: 40%;
    height: 1px;
    background: currentColor;
  }
  /* Darkens as a face turns away from the light */
  .cover-shade {
    position: absolute;
    inset: 0;
    background: rgb(10 16 12);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.45s ease-in;
  }
  .open .cover-front .cover-shade {
    opacity: 0.4;
  }
  .cover-inside .cover-shade {
    opacity: 0.35;
    transition: opacity 0.5s 0.45s ease-out;
  }
  .open .cover-inside .cover-shade {
    opacity: 0;
  }
  .closing .cover-inside .cover-shade {
    transition-delay: 0s;
  }
  .cover-inside {
    transform: rotateY(180deg);
    display: flex;
    flex-direction: column;
    padding: 34px 30px;
    /* Endpaper: paper faintly striped with the cover colour */
    color: var(--text);
    background: repeating-linear-gradient(
      135deg,
      color-mix(in srgb, var(--cloth) 12%, var(--paper)) 0 10px,
      color-mix(in srgb, var(--cloth) 7%, var(--paper)) 10px 20px
    );
    box-shadow: inset 0 0 0 1px var(--border);
    border-radius: var(--radius) 0 0 var(--radius);
  }
  .endpaper-muted {
    color: var(--text-muted);
  }
  .close {
    position: absolute;
    top: -52px;
    right: 0;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 999px;
    color: #fffdf8;
    background: rgb(255 253 248 / 0.16);
    opacity: 0;
    transition:
      background-color 0.15s ease-out,
      opacity 0.3s ease-out;
  }
  .open .close {
    opacity: 1;
  }
  .close:hover {
    background: rgb(255 253 248 / 0.26);
  }

  /* Phones: a single page; the cover swings away to show the contents */
  @media (max-width: 640px) {
    .stage {
      --page-w: min(86vw, 380px);
      width: var(--page-w);
      transform: none;
    }
    .stage.open {
      transform: none;
    }
    .page.right,
    .cover {
      left: 0;
    }
    .spine-face {
      left: calc(0px - var(--depth));
    }
    /* No gutter on a single page */
    .page {
      background: var(--paper);
      border-radius: var(--radius);
    }
    .stage.open .cover {
      transform: rotateY(-180deg) translateX(-8px);
      opacity: 0;
      transition:
        transform 0.9s cubic-bezier(0.3, 0.7, 0.2, 1),
        opacity 0.3s 0.6s;
    }
  }
</style>
