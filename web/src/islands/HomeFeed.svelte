<script lang="ts">
  /** The kitchen's lower half: recently viewed, the shelf, and the newest recipes. */
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import CookingPot from "@lucide/svelte/icons/cooking-pot"
  import Bookshelf from "../components/Bookshelf.svelte"
  import GridSkeleton from "../components/GridSkeleton.svelte"
  import OpenBook from "../components/OpenBook.svelte"
  import RecipeCard from "../components/RecipeCard.svelte"
  import { api } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { ShelfBook } from "../lib/books"
  import type { Cookbook, RecipeSummary } from "../lib/recipe"
  import { recentlyViewed } from "../lib/storage"

  interface Data {
    recipes: RecipeSummary[]
    cookbooks: Cookbook[]
  }

  const page = pageState<Data>(async () => ({
    recipes: await api<RecipeSummary[]>("/api/recipes?limit=8"),
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
  }))
  const data = $derived(page.data)
  const loading = $derived(page.loading)
  let opened = $state<ShelfBook | null>(null)
  const recent = recentlyViewed()
</script>

<div class="space-y-12">
  {#if recent.length}
    <section>
      <h2 class="mb-3 font-serif text-xl font-semibold">Pick up where you left off</h2>
      <div class="no-scrollbar -mx-4 flex gap-3 overflow-x-auto px-4 pb-1">
        {#each recent as r (r.id)}
          <a
            href={`/recipes/${r.id}`}
            class="card hover:bg-raised rounded-ui flex w-64 flex-none items-center gap-3 p-2 pr-4 transition"
          >
            {#if r.image}
              <img
                src={r.image}
                alt=""
                referrerpolicy="no-referrer"
                loading="lazy"
                class="rounded-ui size-14 object-cover"
              />
            {:else}
              <span class="bg-primary/10 text-primary rounded-ui grid size-14 place-items-center">
                <CookingPot class="size-6" />
              </span>
            {/if}
            <span class="line-clamp-2 text-sm font-semibold">{r.title}</span>
          </a>
        {/each}
      </div>
    </section>
  {/if}

  {#if data?.cookbooks.length}
    <section>
      <div class="mb-2 flex items-center justify-between">
        <h2 class="font-serif text-xl font-semibold">Your shelf</h2>
        <a href="/cookbooks" class="btn btn-link">All cookbooks <ArrowRight class="size-4" /></a>
      </div>
      <div class="no-scrollbar -mx-4 overflow-x-auto px-4">
        <div class="min-w-max pt-6">
          <Bookshelf
            books={data.cookbooks.slice(0, 14)}
            pulledId={opened?.id}
            onopen={(b) => (opened = b)}
          />
        </div>
      </div>
      <OpenBook book={opened} onclose={() => (opened = null)} />
    </section>
  {/if}

  {#if loading || data?.recipes.length}
    <section>
      <div class="mb-4 flex items-center justify-between">
        <h2 class="font-serif text-xl font-semibold">Fresh in the box</h2>
        <a href="/recipes" class="btn btn-link">All recipes <ArrowRight class="size-4" /></a>
      </div>
      {#if loading}
        <GridSkeleton count={4} />
      {:else if data}
        <div class="grid grid-cols-2 gap-x-3 gap-y-6 sm:gap-x-5 md:grid-cols-3 lg:grid-cols-4">
          {#each data.recipes as recipe (recipe.id)}
            <RecipeCard {recipe} />
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>
