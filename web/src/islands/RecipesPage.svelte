<script lang="ts">
  import BookOpen from "@lucide/svelte/icons/book-open"
  import BookPlus from "@lucide/svelte/icons/book-plus"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import Search from "@lucide/svelte/icons/search"
  import SearchX from "@lucide/svelte/icons/search-x"
  import SquareCheck from "@lucide/svelte/icons/square-check"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import X from "@lucide/svelte/icons/x"
  import { fly } from "svelte/transition"
  import EmptyState from "../components/EmptyState.svelte"
  import GridSkeleton from "../components/GridSkeleton.svelte"
  import Modal from "../components/Modal.svelte"
  import RecipeCard from "../components/RecipeCard.svelte"
  import { api, errorMessage } from "../lib/api"
  import { bookPalette } from "../lib/books"
  import { pageState } from "../lib/page.svelte"
  import type { Cookbook, RecipeSummary } from "../lib/recipe"
  import { forgetViewed } from "../lib/storage"
  import { toast } from "../lib/toast"

  interface Data {
    recipes: RecipeSummary[]
    cookbooks: Cookbook[]
    q: string
  }

  const initialQ = new URLSearchParams(location.search).get("q") ?? ""
  const page = pageState<Data>(async () => ({
    recipes: await api<RecipeSummary[]>(`/api/recipes?q=${encodeURIComponent(initialQ)}`),
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
    q: initialQ,
  }))

  // Search is server-side (title, ingredients, category, cuisine) and mirrored in the URL
  let search = $state(page.data?.q ?? initialQ)
  let q = $state(search)
  let searching = $state(false)
  const recipes = $derived(page.data?.recipes ?? [])
  const cookbooks = $derived(page.data?.cookbooks ?? [])

  let timer: ReturnType<typeof setTimeout> | undefined
  let requestId = 0
  function onsearch() {
    clearTimeout(timer)
    timer = setTimeout(async () => {
      const value = search.trim()
      if (value === q) return
      q = value
      const url = new URL(location.href)
      if (value) url.searchParams.set("q", value)
      else url.searchParams.delete("q")
      history.replaceState(null, "", url)
      const mine = ++requestId
      searching = true
      try {
        const results = await api<RecipeSummary[]>(`/api/recipes?q=${encodeURIComponent(value)}`)
        if (mine === requestId && page.data) page.data.recipes = results
      } catch (e) {
        toast({ title: "Search failed", description: errorMessage(e), tone: "error" })
      } finally {
        if (mine === requestId) searching = false
      }
    }, 250)
  }

  function clearSearch() {
    search = ""
    category = "all"
    onsearch()
  }

  let category = $state("all")
  const categories = $derived(
    [...new Set(recipes.map((r) => r.recipeCategory).filter((c): c is string => !!c))].sort(
      (a, b) => a.localeCompare(b),
    ),
  )
  const visible = $derived(
    category === "all" ? recipes : recipes.filter((r) => r.recipeCategory === category),
  )

  // ─── Selection ───
  let selecting = $state(false)
  let selected = $state(new Set<number>())
  let busy = $state(false)
  let showDelete = $state(false)
  let showCookbooks = $state(false)

  function toggleSelecting() {
    selecting = !selecting
    selected = new Set()
  }

  function toggle(id: number) {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selected = next
  }

  const allSelected = $derived(visible.length > 0 && visible.every((r) => selected.has(r.id)))
  function toggleAll() {
    selected = allSelected ? new Set() : new Set(visible.map((r) => r.id))
  }

  async function addToCookbook(book: Cookbook) {
    busy = true
    try {
      const ids = [...selected]
      await api(`/api/cookbooks/${book.id}/recipes`, { method: "POST", body: { recipeIds: ids } })
      toast({ title: `Added ${ids.length} to ${book.name}`, tone: "success" })
      showCookbooks = false
      toggleSelecting()
    } catch (e) {
      toast({ title: "Couldn't add to cookbook", description: errorMessage(e), tone: "error" })
    } finally {
      busy = false
    }
  }

  async function deleteSelected() {
    busy = true
    try {
      const ids = [...selected]
      await api("/api/recipes/bulk-delete", { method: "POST", body: { ids } })
      forgetViewed(ids)
      toast({ title: `Deleted ${ids.length} recipe${ids.length === 1 ? "" : "s"}`, tone: "success" })
      if (page.data) page.data.recipes = page.data.recipes.filter((r) => !selected.has(r.id))
      showDelete = false
      toggleSelecting()
    } catch (e) {
      toast({ title: "Couldn't delete", description: errorMessage(e), tone: "error" })
    } finally {
      busy = false
    }
  }
</script>

<div>
  <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
    <h1 class="page-title">Recipes</h1>
    <div class="flex gap-2">
      {#if recipes.length}
        <button
          type="button"
          class={["btn", selecting ? "btn-tile" : "btn-outline"]}
          aria-pressed={selecting}
          onclick={toggleSelecting}
        >
          {#if selecting}<X /> Done{:else}<SquareCheck /> Select{/if}
        </button>
      {/if}
      <!-- The rail carries the same butter "Add recipe" on wider screens -->
      <a href="/add" class="btn btn-primary md:hidden"><Plus /> Add</a>
    </div>
  </div>

  <div class="mb-7 space-y-3">
    <div class="relative max-w-2xl">
      <span class="text-ink-muted pointer-events-none absolute top-1/2 left-4 -translate-y-1/2">
        {#if searching}<LoaderCircle class="size-5 animate-spin" />{:else}<Search
            class="size-5"
          />{/if}
      </span>
      <input
        type="search"
        bind:value={search}
        oninput={onsearch}
        class="input input-lg rounded-ui pr-14 pl-12 [&::-webkit-search-cancel-button]:appearance-none"
        placeholder={recipes.length && !q
          ? `Search ${recipes.length} recipes`
          : "Search by name or ingredient…"}
        aria-label="Search recipes"
      />
      {#if search}
        <button
          type="button"
          class="btn btn-ghost btn-icon absolute top-1/2 right-1.5 -translate-y-1/2"
          aria-label="Clear search"
          onclick={clearSearch}
        >
          <X />
        </button>
      {/if}
    </div>
    {#if categories.length > 1}
      <div
        class="no-scrollbar -mx-5 flex gap-2 overflow-x-auto px-5 md:mx-0 md:flex-wrap md:px-0"
        role="group"
        aria-label="Category"
      >
        {#each [{ label: "Everything", value: "all" }, ...categories.map( (c) => ({ label: c, value: c }), )] as c (c.value)}
          <button
            type="button"
            class={[
              "chip h-11 flex-none",
              category === c.value
                ? "bg-tile text-on-tile"
                : "bg-tint text-primary hover:bg-accented",
            ]}
            aria-pressed={category === c.value}
            onclick={() => (category = c.value)}
          >
            {c.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if page.loading}
    <GridSkeleton />
  {:else if !recipes.length && !q}
    <EmptyState
      icon={BookOpen}
      title="No recipes yet"
      description="Paste a link or some recipe text to save your first one."
    >
      <a href="/add" class="btn btn-primary"><Plus /> Add a recipe</a>
    </EmptyState>
  {:else if !visible.length}
    <EmptyState
      icon={SearchX}
      title="Nothing matches"
      description={q ? `No recipes found for “${q}”.` : "Try another category."}
    >
      <button type="button" class="btn btn-soft" onclick={clearSearch}>Clear search</button>
    </EmptyState>
  {:else}
    <div
      class="grid grid-cols-2 gap-x-3 gap-y-5 sm:grid-cols-3 sm:gap-x-4 lg:grid-cols-4 xl:grid-cols-5"
    >
      {#each visible as recipe (recipe.id)}
        <RecipeCard
          {recipe}
          sizes="(min-width: 1280px) 12rem, (min-width: 1024px) 18vw, (min-width: 768px) 21vw, (min-width: 640px) 31vw, 46vw"
          selectable={selecting}
          selected={selected.has(recipe.id)}
          ontoggle={toggle}
        />
      {/each}
    </div>
  {/if}

  {#if selecting}
    <!-- Room to scroll the last row out from under the bar -->
    <div class="h-20" aria-hidden="true"></div>
    <div
      transition:fly={{ y: 80, duration: 200 }}
      class="border-line bg-paper fixed inset-x-0 bottom-[calc(3.75rem+max(env(safe-area-inset-bottom),0.875rem))] z-30 border-t px-3 py-2.5 md:bottom-0 md:left-60 md:px-8"
      role="toolbar"
      aria-label="Selection"
    >
      <div class="mx-auto flex max-w-[71rem] items-center justify-between gap-2">
        <button type="button" class="btn btn-ghost px-3" onclick={toggleAll}>
          {allSelected ? "Clear" : "Select all"}
        </button>
        <span class="text-[15px] font-bold" aria-live="polite">{selected.size} selected</span>
        <div class="flex gap-2">
          <button
            type="button"
            class="btn btn-soft px-3.5"
            disabled={!selected.size}
            onclick={() => (showCookbooks = true)}
          >
            <BookPlus /> Cookbook
          </button>
          <button
            type="button"
            class="btn btn-outline btn-icon text-error"
            aria-label="Delete selected"
            disabled={!selected.size}
            onclick={() => (showDelete = true)}
          >
            <Trash2 />
          </button>
        </div>
      </div>
    </div>
  {/if}

  <Modal bind:open={showCookbooks} title="Add to cookbook">
    {#if !cookbooks.length}
      <div class="text-center">
        <p class="text-ink-muted">You don't have any cookbooks yet.</p>
        <a href="/cookbooks" class="btn btn-soft mt-3">Create a cookbook</a>
      </div>
    {:else}
      <div class="-mx-2 flex flex-col">
        {#each cookbooks as book (book.id)}
          {@const pal = bookPalette(book.color)}
          <button
            type="button"
            class="rounded-ctl hover:bg-tint active:bg-tint flex min-h-14 items-center gap-3 px-2 text-left transition-colors disabled:opacity-60"
            disabled={busy}
            onclick={() => addToCookbook(book)}
          >
            <!-- a tiny spine in the book's cloth -->
            <span
              class="h-9 w-4 flex-none rounded-[3px_3px_0_0] shadow-[inset_0_0_0_1px_rgb(28_43_34/0.22)] dark:shadow-[inset_0_0_0_1px_rgb(239_233_218/0.25)]"
              style:background={pal.cloth}
              aria-hidden="true"
            ></span>
            <span class="min-w-0 flex-1">
              <span class="block truncate text-base font-bold">{book.name}</span>
              <span class="meta block">
                {book.recipeCount} recipe{book.recipeCount === 1 ? "" : "s"}
              </span>
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </Modal>

  <Modal
    bind:open={showDelete}
    title="Delete recipes?"
    description={`${selected.size} recipe${selected.size === 1 ? "" : "s"} will be permanently deleted.`}
  >
    {#snippet footer()}
      <button type="button" class="btn btn-ghost" onclick={() => (showDelete = false)}>
        Cancel
      </button>
      <button type="button" class="btn btn-danger" disabled={busy} onclick={deleteSelected}>
        Delete
      </button>
    {/snippet}
  </Modal>
</div>
