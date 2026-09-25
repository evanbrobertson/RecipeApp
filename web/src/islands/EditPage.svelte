<script lang="ts">
  import ArrowLeft from "@lucide/svelte/icons/arrow-left"
  import FileQuestion from "@lucide/svelte/icons/file-question"
  import EmptyState from "../components/EmptyState.svelte"
  import RecipeEditor from "../components/RecipeEditor.svelte"
  import { api, errorMessage, pathId } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { CheckFlag, Recipe, RecipeChecks, RecipeFields } from "../lib/recipe"
  import { flash, toast } from "../lib/toast"

  const id = pathId()
  const page = pageState<{ recipe: Recipe; checks?: RecipeChecks | null }>(async () => ({
    recipe: await api<Recipe>(`/api/recipes/${id}`),
    checks: await api<RecipeChecks | null>(`/api/recipes/${id}/checks`).catch(() => null),
  }))
  let saving = $state(false)

  // "Keep as is" is remembered, so Wee Chef won't raise that line again
  let flags = $state<CheckFlag[]>(page.data?.checks?.flags ?? [])
  $effect(() => {
    if (page.data?.checks) flags = page.data.checks.flags
  })
  async function dismiss(flag: CheckFlag) {
    flags = flags.filter((f) => f.id !== flag.id)
    try {
      await api(`/api/recipes/${id}/flags/${flag.id}/dismiss`, { method: "POST" })
    } catch (e) {
      flags = [...flags, flag]
      toast({ title: "Couldn't save that", description: errorMessage(e), tone: "error" })
    }
  }

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
  <div class="space-y-4">
    <div class="skeleton h-5 w-40"></div>
    <div class="skeleton h-9 w-1/2"></div>
    <div class="skeleton h-64 w-full"></div>
  </div>
{:else if !page.data}
  <EmptyState icon={FileQuestion} title="Recipe not found">
    <a href="/recipes" class="btn btn-soft">Back to recipes</a>
  </EmptyState>
{:else}
  <div>
    <a
      href={`/recipes/${id}`}
      class="link -ml-1 inline-flex min-h-11 max-w-full items-center gap-1.5 px-1"
    >
      <ArrowLeft class="size-[18px] flex-none" />
      <span class="truncate">{page.data.recipe.title}</span>
    </a>
    <h1 class="page-title mt-1 mb-7">Edit recipe</h1>
    <RecipeEditor
      initial={page.data.recipe}
      {saving}
      {flags}
      ondismiss={dismiss}
      onsave={save}
      oncancel={() => (location.href = `/recipes/${id}`)}
    />
  </div>
{/if}
