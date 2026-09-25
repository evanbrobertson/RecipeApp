<script lang="ts">
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import BookOpen from "@lucide/svelte/icons/book-open"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import X from "@lucide/svelte/icons/x"
  import BookColorPicker from "../components/BookColorPicker.svelte"
  import EmptyState from "../components/EmptyState.svelte"
  import GridSkeleton from "../components/GridSkeleton.svelte"
  import Menu, { type MenuItem } from "../components/Menu.svelte"
  import Modal from "../components/Modal.svelte"
  import RecipeCard from "../components/RecipeCard.svelte"
  import { api, errorMessage, pathId } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { bookColor, bookPalette } from "../lib/books"
  import { pageState } from "../lib/page.svelte"
  import type { CookbookDetail } from "../lib/recipe"
  import { flash, toast } from "../lib/toast"

  const id = pathId()
  const page = pageState<{ cookbook: CookbookDetail }>(async () => ({
    cookbook: await api<CookbookDetail>(`/api/cookbooks/${id}`),
  }))
  const cookbook = $derived(page.data?.cookbook)
  $effect(() => {
    if (cookbook) document.title = `${cookbook.name} · Crumb`
  })

  let editing = $state(false)
  let form = $state({ name: "", description: "", color: "tile" })
  let showDelete = $state(false)

  function startEdit() {
    if (!cookbook) return
    form = {
      name: cookbook.name,
      description: cookbook.description ?? "",
      color: bookColor(cookbook.color),
    }
    editing = true
  }

  async function refresh() {
    if (page.data) page.data.cookbook = await api<CookbookDetail>(`/api/cookbooks/${id}`)
  }

  async function saveEdit(e: SubmitEvent) {
    e.preventDefault()
    try {
      await api(`/api/cookbooks/${id}`, { method: "PATCH", body: form })
      editing = false
      await refresh()
    } catch (err) {
      toast({ title: "Couldn't save", description: errorMessage(err), tone: "error" })
    }
  }

  async function removeRecipe(recipeId: number) {
    try {
      await api(`/api/cookbooks/${id}/recipes/${recipeId}`, { method: "DELETE" })
      await refresh()
    } catch (err) {
      toast({ title: "Couldn't remove recipe", description: errorMessage(err), tone: "error" })
    }
  }

  async function deleteCookbook() {
    try {
      await api(`/api/cookbooks/${id}`, { method: "DELETE" })
      flash({ title: "Cookbook deleted" })
      location.href = "/cookbooks"
    } catch (err) {
      toast({ title: "Couldn't delete", description: errorMessage(err), tone: "error" })
    }
  }

  const palette = $derived(bookPalette(cookbook?.color))

  const menu: MenuItem[][] = [
    [{ label: "Rename", icon: Pencil, onselect: startEdit }],
    [{ label: "Delete cookbook", icon: Trash2, danger: true, onselect: () => (showDelete = true) }],
  ]
</script>

{#snippet loading()}
  <div class="mb-7 flex items-end gap-4">
    <div class="skeleton h-32 w-12 rounded-[3px_3px_0_0]"></div>
    <div class="flex-1 space-y-2.5 pb-1">
      <div class="skeleton h-8 w-1/2"></div>
      <div class="skeleton h-4 w-1/4"></div>
    </div>
  </div>
  <GridSkeleton />
{/snippet}

{#if page.loading}
  {@render loading()}
{:else if !cookbook}
  <EmptyState icon={FileQuestion} title="Cookbook not found">
    <a href="/cookbooks" class="btn btn-soft">All cookbooks</a>
  </EmptyState>
{:else}
  <div class="mb-7">
    <a href="/cookbooks" class="btn btn-ghost mb-3 -ml-3 px-3"><ArrowLeft /> Shelf</a>
    {#if editing}
      <form class="card max-w-2xl space-y-4 p-4 sm:p-5" onsubmit={saveEdit}>
        <div>
          <label class="label" for="cookbook-name">Name</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="cookbook-name" bind:value={form.name} class="input input-lg" autofocus />
        </div>
        <div>
          <label class="label" for="cookbook-description">Description</label>
          <textarea
            id="cookbook-description"
            use:autosize
            bind:value={form.description}
            placeholder="Optional"
            rows="2"
            class="input"
          ></textarea>
        </div>
        <div>
          <span class="label">Cover colour</span>
          <BookColorPicker bind:value={form.color} />
        </div>
        <div class="flex gap-2 pt-1">
          <button type="submit" class="btn btn-primary">Save</button>
          <button type="button" class="btn btn-ghost" onclick={() => (editing = false)}>
            Cancel
          </button>
        </div>
      </form>
    {:else}
      <div class="flex items-end justify-between gap-3">
        <div class="flex min-w-0 items-end gap-4">
          <!-- the book's spine, standing on a strip of the shelf plank -->
          <div class="flex flex-none flex-col items-stretch" aria-hidden="true">
            <div
              class="spine relative flex h-32 w-12 items-center justify-center overflow-hidden rounded-[3px_3px_0_0]"
              style:background={palette.cloth}
              style:color={palette.foil}
              style:--shade={palette.shade}
            >
              <span class="spine-title truncate font-serif text-sm">{cookbook.name}</span>
            </div>
            <div class="bg-tile -mx-1.5 h-2 rounded-full"></div>
          </div>
          <div class="min-w-0 pb-2">
            <h1 class="page-title break-words">{cookbook.name}</h1>
            <p class="meta mt-1.5">
              {cookbook.recipes.length} recipe{cookbook.recipes.length === 1 ? "" : "s"}
            </p>
          </div>
        </div>
        <div class="flex-none pb-1.5"><Menu groups={menu} /></div>
      </div>
      {#if cookbook.description}
        <p class="text-ink-muted mt-4 max-w-2xl">{cookbook.description}</p>
      {/if}
    {/if}
  </div>

  {#if !cookbook.recipes.length}
    <EmptyState
      icon={BookOpen}
      title="This cookbook is empty"
      description="Open a recipe and tap the cookbook name, or use Select on the recipes page."
    >
      <a href="/recipes" class="btn btn-soft">Browse recipes</a>
    </EmptyState>
  {:else}
    <div
      class="grid grid-cols-2 gap-x-3 gap-y-5 sm:grid-cols-3 sm:gap-x-4 lg:grid-cols-4 xl:grid-cols-5"
    >
      {#each cookbook.recipes as recipe (recipe.id)}
        <div class="relative">
          <RecipeCard
            {recipe}
            sizes="(min-width: 1280px) 12rem, (min-width: 1024px) 18vw, (min-width: 768px) 21vw, (min-width: 640px) 31vw, 46vw"
          />
          <button
            type="button"
            class="btn btn-icon border-line bg-paper text-ink hover:bg-tint absolute top-2 right-2 border"
            aria-label={`Remove ${recipe.title} from cookbook`}
            title="Remove from cookbook"
            onclick={() => removeRecipe(recipe.id)}
          >
            <X />
          </button>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<Modal bind:open={showDelete} title="Delete this cookbook?" description="Recipes in it are kept.">
  {#snippet footer()}
    <button type="button" class="btn btn-ghost" onclick={() => (showDelete = false)}>Cancel</button>
    <button type="button" class="btn btn-danger" onclick={deleteCookbook}>Delete</button>
  {/snippet}
</Modal>

<style>
  /* Cloth with a darker fore-edge, foil bands near the top and bottom, and a faint inset
     line so pale cloths (cream) still read against the page */
  .spine {
    background-image: linear-gradient(
      90deg,
      transparent 78%,
      color-mix(in srgb, var(--shade) 55%, transparent)
    );
    box-shadow: inset 0 0 0 1px rgb(28 43 34 / 0.22);
  }
  :global(.dark) .spine {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.14);
  }
  .spine::before,
  .spine::after {
    content: "";
    position: absolute;
    inset-inline: 0;
    height: 2px;
    background: currentColor;
    opacity: 0.85;
  }
  .spine::before {
    top: 10px;
  }
  .spine::after {
    bottom: 10px;
  }
  .spine-title {
    max-height: calc(100% - 1.75rem);
    writing-mode: vertical-rl;
    transform: rotate(180deg);
  }
</style>
