<script lang="ts">
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import EmptyState from "../components/EmptyState.svelte"
  import RecipeEditor from "../components/RecipeEditor.svelte"
  import { api, errorMessage, pathId } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { Recipe, RecipeFields } from "../lib/recipe"
  import { flash, toast } from "../lib/toast"

  const id = pathId()
  const page = pageState<{ recipe: Recipe }>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
  }))
  let saving = $state(false)

  async function save(fields: RecipeFields) {
    saving = true
    try {
      await api(`/api/recipes/${id}`, { method: "PATCH", body: fields })
      flash({ title: "Saved", tone: "success" })
      location.href = `/recipes/${id}`
    } catch (e) {
      toast({ title: "Couldn't save", description: errorMessage(e), tone: "error" })
      saving = false
    }
  }
</script>

{#if page.loading}
  <div class="mx-auto max-w-4xl space-y-4">
    <div class="skeleton h-10 w-1/2"></div>
    <div class="skeleton h-64 w-full"></div>
  </div>
{:else if !page.data}
  <EmptyState icon={FileQuestion} title="Recipe not found">
    <a href="/recipes" class="btn btn-soft">Back to recipes</a>
  </EmptyState>
{:else}
  <div class="mx-auto max-w-4xl">
    <a href={`/recipes/${id}`} class="text-ink-muted hover:text-primary inline-flex items-center gap-1 text-sm">
      <ArrowLeft class="size-4" /><span class="truncate">{page.data.recipe.title}</span>
    </a>
    <h1 class="mt-1 mb-6 font-serif text-3xl font-semibold">Edit recipe</h1>
    <RecipeEditor
      initial={page.data.recipe}
      {saving}
      onsave={save}
      oncancel={() => (location.href = `/recipes/${id}`)}
    />
  </div>
{/if}
