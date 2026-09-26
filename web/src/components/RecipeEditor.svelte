<script lang="ts">
  /**
   * Edits a recipe. Ingredients and steps are edited as one textarea per section
   * (one item per line), which is much faster than item-by-item inputs, especially on phones.
   */
  import Check from "@lucide/svelte/icons/check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import ChevronDown from "@lucide/svelte/icons/chevron-down"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import { autosize } from "../lib/autosize"
  import { CATEGORIES, isCategory } from "../lib/categories"
  import { quote, reviewText } from "../lib/checks"
  import type { CheckFlag, RecipeFields, RecipeSection } from "../lib/recipe"

  interface Props {
    initial?: Partial<RecipeFields>
    saving?: boolean
    submitLabel?: string
    onsave: (fields: RecipeFields) => void
    oncancel: () => void
    /** Lines Wee Chef thinks might need a look (review flags from its import check). */
    flags?: CheckFlag[]
    /** "Keep as is" on a flagged line. */
    ondismiss?: (flag: CheckFlag) => void
  }
  let {
    initial = {},
    saving = false,
    submitLabel = "Save",
    onsave,
    oncancel,
    flags = [],
    ondismiss,
  }: Props = $props()

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
  // A category from before the fixed list stays selected (marked old) until it's changed
  const oldCategory = i.recipeCategory && !isCategory(i.recipeCategory) ? i.recipeCategory : null

  const metaFields = [
    { key: "prepTime", label: "Prep time", placeholder: "15m" },
    { key: "cookTime", label: "Cook time", placeholder: "30m" },
    { key: "freezeTime", label: "Extra time", placeholder: "Chill 1h" },
    { key: "totalTime", label: "Total time", placeholder: "1h 45m" },
    { key: "recipeYield", label: "Yield", placeholder: "4 servings" },
    { key: "recipeCategory", label: "Category", placeholder: "" },
    { key: "recipeCuisine", label: "Cuisine", placeholder: "Italian" },
    { key: "author", label: "Author", placeholder: "" },
  ] as const

  // ─── Wee Chef's hints ───
  // A flag shows under the section that still has its line; editing the line away (or
  // one of the fixes below) clears it, and the server resolves it on save.
  const lines = (text: string) => text.split("\n").map((l) => l.trim())
  function hintsFor(kind: "ingredients" | "instructions", section: DraftSection) {
    const here = lines(section.text)
    return flags.filter((f) => f.field === kind && f.state === "review" && here.includes(f.itemText))
  }

  type Fix = { label: string; run: () => void }
  function fixFor(kind: "ingredients" | "instructions", si: number, f: CheckFlag): Fix | null {
    const list = draft[kind]
    const section = list[si]!
    const all = section.text.split("\n")
    const at = all.findIndex((l) => l.trim() === f.itemText)
    if (at < 0) return null
    const without = (drop: number) => all.filter((_, j) => j !== drop)
    switch (f.kind) {
      case "heading":
        // A heading needs lines under it, or the empty section would vanish on save
        if (!all.slice(at + 1).some((l) => l.trim())) return null
        return {
          label: "Make it a heading",
          run: () => {
            const name = f.itemText.replace(/[:.\s]+$/, "")
            const before = all.slice(0, at).join("\n").trim()
            const after = all.slice(at + 1).join("\n")
            if (!before && !section.name.trim()) {
              section.name = name
              section.text = after
            } else {
              section.text = before
              list.splice(si + 1, 0, { name, text: after })
            }
          },
        }
      case "junk":
        return { label: "Remove it", run: () => (section.text = without(at).join("\n")) }
      case "not_instruction":
        return kind === "instructions"
          ? {
              label: "Move to notes",
              run: () => {
                section.text = without(at).join("\n")
                draft.notes = [draft.notes.trim(), f.itemText].filter(Boolean).join("\n\n")
              },
            }
          : null
      case "fragment":
        return at > 0
          ? {
              label: "Join with the step above",
              run: () => {
                const next = [...all]
                next[at - 1] = `${next[at - 1]!.trimEnd()} ${next[at]!.trim()}`
                next.splice(at, 1)
                section.text = next.join("\n")
              },
            }
          : null
      default:
        return null
    }
  }

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
        {#each hintsFor(kind, section) as f (f.id)}
          {@const fix = fixFor(kind, si, f)}
          <div class="bg-tint rounded-ctl flex flex-wrap items-center gap-x-3 gap-y-1 py-1.5 pr-1.5 pl-3">
            <p class="flex min-w-0 flex-[1_1_16rem] items-start gap-2 py-1.5 text-sm">
              <ChefHat class="text-primary mt-px size-4 flex-none" aria-hidden="true" />
              <span class="min-w-0 break-words">
                <span class="sr-only">Wee Chef: </span>
                <span class="font-bold">{quote(f.itemText, 60)}</span>
                {reviewText(f)}
              </span>
            </p>
            <div class="ml-auto flex flex-wrap gap-1">
              {#if fix}
                <button type="button" class="btn btn-outline px-3.5" onclick={fix.run}>
                  {fix.label}
                </button>
              {/if}
              {#if ondismiss}
                <button type="button" class="btn btn-ghost px-3.5" onclick={() => ondismiss(f)}>
                  Keep as is
                </button>
              {/if}
            </div>
          </div>
        {/each}
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
          {#if f.key === "recipeCategory"}
            <div class="relative">
              <select id="r-recipeCategory" bind:value={draft.recipeCategory} class="input">
                <option value="">None</option>
                {#if oldCategory && draft.recipeCategory === oldCategory}
                  <option value={oldCategory}>(old) {oldCategory}</option>
                {/if}
                {#each CATEGORIES as c (c)}
                  <option value={c}>{c}</option>
                {/each}
              </select>
              <ChevronDown
                class="text-ink-muted pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2"
                aria-hidden="true"
              />
            </div>
          {:else}
            <input
              id={`r-${f.key}`}
              bind:value={draft[f.key]}
              placeholder={f.placeholder}
              class="input"
            />
          {/if}
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
