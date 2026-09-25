<script lang="ts">
  /**
   * A cookbook pulled off the shelf: it flies to the centre, the cover swings open
   * and the right-hand page is a table of contents.
   */
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import X from "@lucide/svelte/icons/x"
  import { api } from "../lib/api"
  import { bookPalette, edgeShadow, type ShelfBook } from "../lib/books"
  import type { CookbookDetail } from "../lib/recipe"

  let { book, onclose }: { book: ShelfBook | null; onclose: () => void } = $props()

  let open = $state(false)
  let visible = $state(false)
  let details = $state<CookbookDetail | null>(null)
  let loading = $state(false)
  const palette = $derived(bookPalette(book?.color))

  $effect(() => {
    const current = book
    if (!current) return
    visible = true
    open = false
    details = null
    loading = true
    // Let the closed book land before the cover swings open
    const t = setTimeout(() => (open = true), 380)
    api<CookbookDetail>(`/api/cookbooks/${current.id}`)
      .then((d) => (details = d))
      .catch(() => (details = null))
      .finally(() => (loading = false))
    return () => clearTimeout(t)
  })

  function close() {
    open = false
    setTimeout(() => {
      visible = false
      onclose()
    }, 520)
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && book) close()
  }
</script>

<svelte:window {onkeydown} />

{#if visible && book}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    onclick={(e) => {
      if (e.target === e.currentTarget) close()
    }}
  >
    <div
      class={["stage", open && "open"]}
      style:--cloth={palette.cloth}
      style:--shade={palette.shade}
      style:--foil={palette.foil}
      style:--edge={edgeShadow(palette)}
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
              <li>
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

      <!-- The cover: front outside, endpaper inside -->
      <div class="cover">
        <div class="cover-front">
          <div class="frame">
            <ChefHat class="size-8" />
            <h2 class="font-serif text-[26px] leading-tight sm:text-[30px]">
              {book.name}
            </h2>
            <span class="rule"></span>
            <span class="text-[13px] font-bold tracking-[0.2em] uppercase">Recipes</span>
          </div>
        </div>
        <div class="cover-inside">
          <p class="endpaper-muted text-[13px] font-bold tracking-[0.3em] uppercase">Ex libris</p>
          <h3 class="mt-3 font-serif text-[24px] leading-tight">{book.name}</h3>
          {#if book.description}<p class="mt-3 text-sm">{book.description}</p>{/if}
          <p class="endpaper-muted mt-auto text-[13px] font-semibold">
            A cookbook of {book.recipeCount} recipes
          </p>
        </div>
      </div>

      <button type="button" class="close" aria-label="Close book" onclick={close}>
        <X class="size-[22px]" />
      </button>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgb(13 19 15 / 0.6);
    backdrop-filter: blur(6px);
    perspective: 2200px;
    animation: fade 0.3s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .stage {
    --page-w: min(380px, 44vw);
    position: relative;
    width: calc(var(--page-w) * 2);
    height: min(560px, 80dvh);
    transform-style: preserve-3d;
    /* Closed: centre the cover (the right half) */
    transform: translateX(calc(var(--page-w) / -2)) rotateX(8deg) scale(0.92);
    transition: transform 0.6s cubic-bezier(0.2, 0.8, 0.2, 1);
    animation: land 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .stage.open {
    transform: translateX(0) rotateX(0) scale(1);
  }
  @keyframes land {
    from {
      transform: translateX(calc(var(--page-w) / -2)) translateY(40vh) rotateX(40deg) scale(0.4);
      opacity: 0;
    }
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
  }
  .page.right {
    left: var(--page-w);
  }
  .toc {
    overflow-y: auto;
    margin-right: -8px;
    padding-right: 8px;
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

  .cover {
    position: absolute;
    top: -6px;
    left: var(--page-w);
    width: calc(var(--page-w) + 6px);
    height: calc(100% + 12px);
    transform-origin: left center;
    transform-style: preserve-3d;
    transition: transform 0.9s cubic-bezier(0.3, 0.7, 0.2, 1);
  }
  .stage.open .cover {
    transform: rotateY(-180deg);
  }
  .cover-front,
  .cover-inside {
    position: absolute;
    inset: 0;
    backface-visibility: hidden;
    border-radius: 0 var(--radius) var(--radius) 0;
  }
  .cover-front {
    display: grid;
    place-items: center;
    padding: 28px;
    color: var(--foil);
    /* Flat cloth, with a soft shadow along the hinge */
    background: linear-gradient(90deg, rgb(0 0 0 / 0.22), transparent 7%), var(--cloth);
    box-shadow: var(--edge), var(--menu-shadow);
  }
  .frame {
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
  }
  .rule {
    width: 40%;
    height: 1px;
    background: var(--foil);
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
    transition: background-color 0.15s ease-out;
  }
  .close:hover {
    background: rgb(255 253 248 / 0.26);
  }

  /* Phones: a single page; the cover swings away to reveal the contents */
  @media (max-width: 640px) {
    .stage {
      --page-w: min(86vw, 380px);
      width: var(--page-w);
      transform: rotateX(8deg) scale(0.92);
      animation-name: land-phone;
    }
    .stage.open {
      transform: none;
    }
    .page.right,
    .cover {
      left: 0;
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
  @keyframes land-phone {
    from {
      transform: translateY(40vh) rotateX(40deg) scale(0.4);
      opacity: 0;
    }
  }
</style>
