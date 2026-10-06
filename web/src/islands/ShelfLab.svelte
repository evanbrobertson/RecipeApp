<script lang="ts">
  /** Spike: the redesigned shelf with made-up books, at a few widths. Not linked from anywhere. */
  import Bookshelf from "../components/Bookshelf.svelte"
  import OpenBook from "../components/OpenBook.svelte"
  import type { ShelfBook } from "../lib/books"
  import type { BookOrigin } from "../lib/shelf"

  const names: [string, string, number][] = [
    ["Weeknight dinners", "tile", 24],
    ["Christmas baking", "clay", 12],
    ["Soups", "cream", 6],
    ["Grandma's Sunday roasts and other family favourites", "forest", 31],
    ["Bread", "butter", 4],
    ["Things to make when it's raining", "sage", 9],
    ["Curries", "forest", 17],
    ["Summer salads", "sage", 11],
    ["Ottolenghi-ish vegetable mains", "tile", 14],
    ["Breakfast", "butter", 8],
    ["Pasta", "clay", 20],
    ["Recipes from the trip to Lisbon", "cream", 5],
    ["Cakes", "tile", 13],
    ["Pickles & preserves", "forest", 7],
    ["Meal prep for busy weeks", "clay", 10],
    ["Desserts", "sage", 19],
    ["Dinner party showstoppers", "butter", 6],
    ["Mum's", "tile", 3],
    ["Slow cooker", "cream", 15],
    ["Noodles", "forest", 22],
  ]
  const all: ShelfBook[] = names.map(([name, color, recipeCount], i) => ({
    id: 101 + i * 7,
    name,
    color,
    recipeCount,
    description: null,
  }))

  let count = $state(all.length)
  let width = $state(1000)
  const books = $derived(all.slice(0, count))

  let opened = $state<ShelfBook | null>(null)
  let from = $state<BookOrigin | null>(null)
  const openBook = (b: ShelfBook, o?: BookOrigin) => {
    from = o ?? null
    opened = b
  }

  // Contents for the open book, without a server
  const real = window.fetch
  window.fetch = async (input, init) => {
    const url = String(input instanceof Request ? input.url : input)
    const m = url.match(/\/api\/cookbooks\/(\d+)$/)
    if (!m) return real(input, init)
    const b = all.find((x) => x.id === Number(m[1]))!
    const titles = ["Charred leeks with romesco", "Lemony white beans", "Brown butter gnocchi",
      "Smoky chickpea stew", "Crispy rice salad", "Miso aubergine", "Sticky toffee pudding",
      "Green goddess pasta", "Roast chicken thighs", "Pea & ricotta toast"]
    const recipes = titles.slice(0, Math.min(10, b.recipeCount)).map((title, i) => ({ id: i + 1, title }))
    await new Promise((r) => setTimeout(r, 250))
    return new Response(JSON.stringify({ ...b, recipes }), { headers: { "content-type": "application/json" } })
  }

  function theme(t: string) {
    document.documentElement.classList.toggle("dark", t === "dark")
  }
</script>

<div class="space-y-8">
  <div class="flex flex-wrap items-center gap-4 text-sm">
    <label class="flex items-center gap-2">Books <input type="range" min="0" max={all.length} bind:value={count} /> {count}</label>
    <label class="flex items-center gap-2">Width <input type="range" min="300" max="1200" bind:value={width} /> {width}px</label>
    <button type="button" class="btn btn-soft btn-sm" onclick={() => theme("light")}>Light</button>
    <button type="button" class="btn btn-soft btn-sm" onclick={() => theme("dark")}>Dark</button>
  </div>

  <section>
    <h2 class="section-title mb-3">Your shelf</h2>
    <div class="bg-tint rounded-ui overflow-hidden" style:max-width={`${width}px`}>
      <Bookshelf {books} addable pulledId={opened?.id} onopen={openBook} onadd={() => alert("New book")} />
    </div>
  </section>

  <section>
    <h2 class="section-title mb-3">Home</h2>
    <div class="bg-tint rounded-ui overflow-hidden" style:max-width={`${width}px`}>
      <div class="no-scrollbar overflow-x-auto">
        <Bookshelf books={books.slice(0, 14)} single pulledId={opened?.id} onopen={openBook} />
      </div>
    </div>
  </section>

  <section>
    <h2 class="section-title mb-3">Phone</h2>
    <div class="bg-tint rounded-ui overflow-hidden" style:max-width="343px">
      <Bookshelf {books} addable pulledId={opened?.id} onopen={openBook} />
    </div>
  </section>

  <OpenBook book={opened} {from} onclose={() => (opened = null)} />
</div>
