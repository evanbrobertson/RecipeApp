<script lang="ts">
  /**
   * Edits a recipe. Ingredients and steps are edited as one textarea per section
   * (one item per line), which is much faster than item-by-item inputs, especially on phones.
   */
  import Check from "@lucide/svelte/icons/check"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import { autosize } from "../lib/autosize"
  import type { RecipeFields, RecipeSection } from "../lib/recipe"

  interface Props {
    initial?: Partial<RecipeFields>
    saving?: boolean
    submitLabel?: string
    onsave: (fields: RecipeFields) => void
    oncancel: () => void
  }
  let { initial = {}, saving = false, submitLabel = "Save", onsave, oncancel }: Props = $props()

  interface DraftSection {
    name: string
    text: string
  }

  const toDraft = (sections?: RecipeSection[] | null): DraftSection[] =>
    sections?.length
      ? sections.map((s) => ({ name: s.name ?? "", text: s.items.join("\n") }))
      : [{ name: "", text: "" }]

  const fromDraft = (sections: DraftSection[]): RecipeSection[] =>
    sections
      .map((s) => ({
        name: s.name.trim() || null,
        items: s.text
          .split("\n")
          .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, "").trim())
          .filter(Boolean),
      }))
      .filter((s) => s.items.length > 0)

  const str = (v: string | null | undefined) => v ?? ""
  const nullable = (v: string) => v.trim() || null

  // Snapshot the initial values once; the form owns the draft from here on
  const i = $state.snapshot(initial)
  let draft = $state({
    title: str(i.title),
    description: str(i.description),
    author: str(i.author),
    prepTime: str(i.prepTime),
    cookTime: str(i.cookTime),
    freezeTime: str(i.freezeTime),
    totalTime: str(i.totalTime),
    recipeYield: str(i.recipeYield),
    recipeCategory: str(i.recipeCategory),
    recipeCuisine: str(i.recipeCuisine),
    url: str(i.url),
    image: str(i.image),
    notes: str(i.notes),
    nutrition: Object.entries(i.nutrition ?? {})
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n"),
    ingredients: toDraft(i.ingredients),
    instructions: toDraft(i.instructions),
  })

  const metaFields = [
    { key: "prepTime", label: "Prep time", placeholder: "15m" },
    { key: "cookTime", label: "Cook time", placeholder: "30m" },
    { key: "freezeTime", label: "Extra time", placeholder: "Chill 1h" },
    { key: "totalTime", label: "Total time", placeholder: "1h 45m" },
    { key: "recipeYield", label: "Yield", placeholder: "4 servings" },
    { key: "recipeCategory", label: "Category", placeholder: "Dinner" },
    { key: "recipeCuisine", label: "Cuisine", placeholder: "Italian" },
    { key: "author", label: "Author", placeholder: "" },
  ] as const

  let error = $state("")

  function submit(e: SubmitEvent) {
    e.preventDefault()
    error = ""
    if (!draft.title.trim()) {
      error = "Give the recipe a title."
      return
    }
    const nutrition = Object.fromEntries(
      draft.nutrition
        .split("\n")
        .map((line) => line.split(/:(.*)/s).map((p) => p?.trim()))
        .filter(([k, v]) => k && v),
    ) as Record<string, string>

    onsave({
      title: draft.title.trim(),
      description: nullable(draft.description),
      author: nullable(draft.author),
      prepTime: nullable(draft.prepTime),
      cookTime: nullable(draft.cookTime),
      freezeTime: nullable(draft.freezeTime),
      totalTime: nullable(draft.totalTime),
      recipeYield: nullable(draft.recipeYield),
      recipeCategory: nullable(draft.recipeCategory),
      recipeCuisine: nullable(draft.recipeCuisine),
      url: nullable(draft.url),
      image: nullable(draft.image),
      notes: nullable(draft.notes),
      nutrition: Object.keys(nutrition).length ? nutrition : null,
      ingredients: fromDraft(draft.ingredients),
      instructions: fromDraft(draft.instructions),
    })
  }
</script>

{#snippet sections(kind: "ingredients" | "instructions")}
  {@const list = draft[kind]}
  <div class="flex items-center justify-between">
    <h2 class="font-serif text-lg font-semibold">
      {kind === "ingredients" ? "Ingredients" : "Instructions"}
    </h2>
    <button
      type="button"
      class="btn btn-soft btn-sm"
      onclick={() => list.push({ name: "", text: "" })}
    >
      <Plus /> Section
    </button>
  </div>
  <p class="text-ink-muted text-xs">
    One {kind === "ingredients" ? "ingredient" : "step"} per line.
  </p>
  {#each list as section, si (si)}
    <div class="space-y-2">
      {#if list.length > 1 || section.name}
        <div class="flex gap-2">
          <input
            bind:value={section.name}
            class="input flex-1"
            placeholder={kind === "ingredients"
              ? "Section name, e.g. For the sauce"
              : "Section name, e.g. Make the icing"}
            aria-label="Section name"
          />
          <button
            type="button"
            class="btn btn-ghost btn-icon text-error"
            aria-label="Remove section"
            onclick={() => list.splice(si, 1)}
          >
            <Trash2 />
          </button>
        </div>
      {/if}
      <textarea
        use:autosize
        bind:value={section.text}
        rows={kind === "ingredients" ? 6 : 8}
        class="input"
        aria-label={kind === "ingredients" ? "Ingredients" : "Steps"}
        placeholder={kind === "ingredients"
          ? "2 cups flour\n1 tsp salt"
          : "Preheat the oven to 180°C.\nMix the dry ingredients."}
      ></textarea>
    </div>
  {/each}
{/snippet}

<form class="space-y-8" onsubmit={submit}>
  <div class="space-y-4">
    <div>
      <label class="label" for="r-title">Title <span class="text-error">*</span></label>
      <input
        id="r-title"
        bind:value={draft.title}
        class="input input-lg"
        placeholder="Grandma's lasagne"
      />
      {#if error}<p class="text-error mt-1 text-sm">{error}</p>{/if}
    </div>
    <div>
      <label class="label" for="r-desc">Description</label>
      <textarea use:autosize id="r-desc" bind:value={draft.description} rows="2" class="input"></textarea>
    </div>
  </div>

  <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
    {#each metaFields as f (f.key)}
      <div>
        <label class="label" for={`r-${f.key}`}>{f.label}</label>
        <input
          id={`r-${f.key}`}
          bind:value={draft[f.key]}
          placeholder={f.placeholder}
          class="input"
        />
      </div>
    {/each}
  </div>

  <div class="grid gap-8 lg:grid-cols-5">
    <div class="space-y-3 lg:col-span-2">{@render sections("ingredients")}</div>
    <div class="space-y-3 lg:col-span-3">{@render sections("instructions")}</div>
  </div>

  <div class="grid gap-4 sm:grid-cols-2">
    <div>
      <label class="label" for="r-notes">Notes</label>
      <textarea use:autosize id="r-notes" bind:value={draft.notes} rows="3" class="input"></textarea>
    </div>
    <div>
      <label class="label" for="r-nutrition">Nutrition</label>
      <textarea use:autosize id="r-nutrition" bind:value={draft.nutrition} rows="3" class="input"></textarea>
      <p class="hint">One per line, e.g. calories: 320</p>
    </div>
    <div>
      <label class="label" for="r-url">Source link</label>
      <input
        id="r-url"
        bind:value={draft.url}
        type="url"
        placeholder="https://…"
        class="input"
      />
    </div>
    <div>
      <label class="label" for="r-image">Image link</label>
      <input
        id="r-image"
        bind:value={draft.image}
        type="url"
        placeholder="https://…"
        class="input"
      />
    </div>
  </div>

  <div class="border-line flex justify-end gap-2 border-t pt-4">
    <button type="button" class="btn btn-ghost" onclick={oncancel}>Cancel</button>
    <button type="submit" class="btn btn-primary" disabled={saving}>
      {#if saving}<LoaderCircle class="animate-spin" />{:else}<Check />{/if}
      {submitLabel}
    </button>
  </div>
</form>
