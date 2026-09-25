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
  <div class="mb-3.5 flex items-center justify-between gap-3">
    <div>
      <h2 class="section-title">{kind === "ingredients" ? "Ingredients" : "Instructions"}</h2>
      <p class="hint">One {kind === "ingredients" ? "ingredient" : "step"} per line.</p>
    </div>
    <button type="button" class="btn btn-soft" onclick={() => list.push({ name: "", text: "" })}>
      <Plus /> Section
    </button>
  </div>
  <div class="space-y-4">
    {#each list as section, si (si)}
      <div class="space-y-2">
        {#if list.length > 1 || section.name}
          <div class="flex gap-2">
            <input
              bind:value={section.name}
              class="input flex-1 font-bold"
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
          class="input leading-relaxed"
          aria-label={kind === "ingredients" ? "Ingredients" : "Steps"}
          placeholder={kind === "ingredients"
            ? "2 cups flour\n1 tsp salt"
            : "Preheat the oven to 180°C.\nMix the dry ingredients."}
        ></textarea>
      </div>
    {/each}
  </div>
{/snippet}

<form class="space-y-7" onsubmit={submit}>
  <div class="card space-y-4 p-4 sm:p-5">
    <div>
      <label class="label" for="r-title">Title <span class="text-error">*</span></label>
      <input
        id="r-title"
        bind:value={draft.title}
        class="input input-lg"
        placeholder="Grandma's lasagne"
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? "r-title-error" : undefined}
      />
      {#if error}<p id="r-title-error" class="text-error mt-1.5 text-sm font-bold">{error}</p>{/if}
    </div>
    <div>
      <label class="label" for="r-desc">Description</label>
      <textarea use:autosize id="r-desc" bind:value={draft.description} rows="2" class="input"
      ></textarea>
    </div>
  </div>

  <section>
    <h2 class="section-title mb-3.5">Details</h2>
    <div class="card grid grid-cols-2 gap-x-3 gap-y-4 p-4 sm:grid-cols-4 sm:p-5">
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
  </section>

  <div class="grid gap-7 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] lg:gap-10">
    <section>{@render sections("ingredients")}</section>
    <section>{@render sections("instructions")}</section>
  </div>

  <section>
    <h2 class="section-title mb-3.5">Notes and sources</h2>
    <div class="card grid gap-4 p-4 sm:grid-cols-2 sm:p-5">
      <div>
        <label class="label" for="r-notes">Notes</label>
        <!-- The cook's own notes, in their own hand (as on the recipe page) -->
        <textarea
          use:autosize
          id="r-notes"
          bind:value={draft.notes}
          rows="3"
          class="input hand text-[24px] leading-snug"
          placeholder="Less sugar next time…"
        ></textarea>
      </div>
      <div>
        <label class="label" for="r-nutrition">Nutrition</label>
        <textarea
          use:autosize
          id="r-nutrition"
          bind:value={draft.nutrition}
          rows="3"
          class="input"
          aria-describedby="r-nutrition-hint"
        ></textarea>
        <p id="r-nutrition-hint" class="hint">One per line, e.g. calories: 320</p>
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
  </section>

  <div class="border-line flex justify-end gap-2 border-t pt-5">
    <button type="button" class="btn btn-ghost btn-lg" onclick={oncancel}>Cancel</button>
    <button type="submit" class="btn btn-primary btn-lg" disabled={saving}>
      {#if saving}<LoaderCircle class="animate-spin" />{:else}<Check />{/if}
      {submitLabel}
    </button>
  </div>
</form>
