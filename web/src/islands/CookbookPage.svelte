<script lang="ts">
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import BookOpen from "@lucide/svelte/icons/book-open"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import X from "@lucide/svelte/icons/x"
  import BookColorPicker from "../components/BookColorPicker.svelte"
  import EmptyState from "../components/EmptyState.svelte"
  import Menu, { type MenuItem } from "../components/Menu.svelte"
  import Modal from "../components/Modal.svelte"
  import RecipeCard from "../components/RecipeCard.svelte"
  import { api, errorMessage, pathId } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { bookPalette } from "../lib/books"
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
  let form = $state({ name: "", description: "", color: "tomato" })
  let showDelete = $state(false)

  function startEdit() {
    if (!cookbook) return
    form = {
      name: cookbook.name,
      description: cookbook.description ?? "",
      color: cookbook.color ?? "tomato",
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

  const menu: MenuItem[][] = [
    [{ label: "Rename", icon: Pencil, onselect: startEdit }],
    [{ label: "Delete cookbook", icon: Trash2, danger: true, onselect: () => (showDelete = true) }],
  ]
</script>

{#if page.loading}
  <div class="skeleton h-64 w-full"></div>
{:else if !cookbook}
  <EmptyState icon={FileQuestion} title="Cookbook not found">
    <a href="/cookbooks" class="btn btn-soft">All cookbooks</a>
  </EmptyState>
{:else}
  <div class="mb-6">
    <a href="/cookbooks" class="btn btn-ghost btn-sm mb-2 -ml-3"><ArrowLeft /> Shelf</a>
    {#if editing}
      <form class="space-y-3" onsubmit={saveEdit}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={form.name} class="input input-lg" aria-label="Name" autofocus />
        <textarea
          use:autosize
          bind:value={form.description}
          placeholder="Description (optional)"
          rows="2"
          class="input"
          aria-label="Description"
        ></textarea>
        <BookColorPicker bind:value={form.color} />
        <div class="flex gap-2">
          <button type="submit" class="btn btn-primary">Save</button>
          <button type="button" class="btn btn-ghost" onclick={() => (editing = false)}>
            Cancel
          </button>
        </div>
      </form>
    {:else}
      <div class="flex items-start justify-between gap-3">
        <div class="flex items-start gap-4">
          <!-- a little cover, in the book's cloth -->
          <div
            class="rounded-ui grid h-24 w-18 shrink-0 place-items-center shadow-md"
            style:background={bookPalette(cookbook.color).cloth}
            style:color={bookPalette(cookbook.color).foil}
          >
            <ChefHat class="size-7" />
          </div>
          <div>
            <h1 class="font-serif text-3xl font-semibold sm:text-4xl">{cookbook.name}</h1>
            <p class="text-ink-muted mt-1">
              {cookbook.recipes.length} recipe{cookbook.recipes.length === 1 ? "" : "s"}
              {#if cookbook.description}· {cookbook.description}{/if}
            </p>
          </div>
        </div>
        <Menu groups={menu} />
      </div>
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
    <div class="grid grid-cols-2 gap-x-3 gap-y-6 sm:gap-x-5 md:grid-cols-3 lg:grid-cols-4">
      {#each cookbook.recipes as recipe (recipe.id)}
        <div class="relative">
          <RecipeCard {recipe} />
          <button
            type="button"
            class="btn btn-sm btn-icon bg-canvas/90 text-ink absolute top-3 right-3 shadow-sm backdrop-blur"
            aria-label="Remove from cookbook"
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
