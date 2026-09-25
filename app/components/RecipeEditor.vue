<script setup lang="ts">
import type { RecipeFields, RecipeSection } from "#shared/utils/recipe"

/**
 * Edits a recipe. Ingredients and steps are edited as one textarea per section
 * (one item per line), which is much faster than item-by-item inputs, especially on phones.
 */
const props = defineProps<{
  initial?: Partial<RecipeFields>
  saving?: boolean
  submitLabel?: string
}>()

const emit = defineEmits<{ save: [fields: RecipeFields]; cancel: [] }>()

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

const i = props.initial ?? {}
const draft = reactive({
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

const error = ref("")

function submit() {
  error.value = ""
  if (!draft.title.trim()) {
    error.value = "Give the recipe a title."
    return
  }
  const nutrition = Object.fromEntries(
    draft.nutrition
      .split("\n")
      .map((line) => line.split(/:(.*)/s).map((p) => p?.trim()))
      .filter(([k, v]) => k && v),
  ) as Record<string, string>

  emit("save", {
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

<template>
  <form class="space-y-8" @submit.prevent="submit">
    <div class="space-y-3">
      <UFormField label="Title" required :error="error || undefined">
        <UInput v-model="draft.title" size="xl" class="w-full" placeholder="Grandma's lasagne" />
      </UFormField>
      <UFormField label="Description">
        <UTextarea v-model="draft.description" autoresize :rows="2" class="w-full" />
      </UFormField>
    </div>

    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <UFormField v-for="f in metaFields" :key="f.key" :label="f.label">
        <UInput v-model="draft[f.key]" :placeholder="f.placeholder" class="w-full" />
      </UFormField>
    </div>

    <div class="grid gap-8 lg:grid-cols-5">
      <div class="space-y-3 lg:col-span-2">
        <div class="flex items-center justify-between">
          <h2 class="font-serif text-lg font-semibold">Ingredients</h2>
          <UButton
            label="Section"
            icon="i-lucide-plus"
            size="xs"
            variant="soft"
            @click="
              () => {
                draft.ingredients.push({ name: '', text: '' })
              }
            "
          />
        </div>
        <p class="text-muted text-xs">One ingredient per line.</p>
        <div v-for="(section, si) in draft.ingredients" :key="si" class="space-y-2">
          <div v-if="draft.ingredients.length > 1 || section.name" class="flex gap-2">
            <UInput
              v-model="section.name"
              placeholder="Section name, e.g. For the sauce"
              size="sm"
              class="flex-1"
            />
            <UButton
              icon="i-lucide-trash-2"
              size="sm"
              color="error"
              variant="ghost"
              aria-label="Remove section"
              @click="
                () => {
                  draft.ingredients.splice(si, 1)
                }
              "
            />
          </div>
          <UTextarea
            v-model="section.text"
            autoresize
            :rows="6"
            class="w-full"
            placeholder="2 cups flour&#10;1 tsp salt"
          />
        </div>
      </div>

      <div class="space-y-3 lg:col-span-3">
        <div class="flex items-center justify-between">
          <h2 class="font-serif text-lg font-semibold">Instructions</h2>
          <UButton
            label="Section"
            icon="i-lucide-plus"
            size="xs"
            variant="soft"
            @click="
              () => {
                draft.instructions.push({ name: '', text: '' })
              }
            "
          />
        </div>
        <p class="text-muted text-xs">One step per line.</p>
        <div v-for="(section, si) in draft.instructions" :key="si" class="space-y-2">
          <div v-if="draft.instructions.length > 1 || section.name" class="flex gap-2">
            <UInput
              v-model="section.name"
              placeholder="Section name, e.g. Make the icing"
              size="sm"
              class="flex-1"
            />
            <UButton
              icon="i-lucide-trash-2"
              size="sm"
              color="error"
              variant="ghost"
              aria-label="Remove section"
              @click="
                () => {
                  draft.instructions.splice(si, 1)
                }
              "
            />
          </div>
          <UTextarea
            v-model="section.text"
            autoresize
            :rows="8"
            class="w-full"
            placeholder="Preheat the oven to 180°C.&#10;Mix the dry ingredients."
          />
        </div>
      </div>
    </div>

    <div class="grid gap-4 sm:grid-cols-2">
      <UFormField label="Notes">
        <UTextarea v-model="draft.notes" autoresize :rows="3" class="w-full" />
      </UFormField>
      <UFormField label="Nutrition" help="One per line, e.g. calories: 320">
        <UTextarea v-model="draft.nutrition" autoresize :rows="3" class="w-full" />
      </UFormField>
      <UFormField label="Source link">
        <UInput v-model="draft.url" type="url" placeholder="https://…" class="w-full" />
      </UFormField>
      <UFormField label="Image link">
        <UInput v-model="draft.image" type="url" placeholder="https://…" class="w-full" />
      </UFormField>
    </div>

    <div class="border-default flex justify-end gap-2 border-t pt-4">
      <UButton label="Cancel" variant="ghost" color="neutral" @click="emit('cancel')" />
      <UButton
        type="submit"
        :label="submitLabel ?? 'Save'"
        icon="i-lucide-check"
        :loading="saving"
      />
    </div>
  </form>
</template>
