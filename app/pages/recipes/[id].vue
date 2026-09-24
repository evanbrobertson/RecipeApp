<script setup lang="ts">
import type { DropdownMenuItem } from "@nuxt/ui"
import type { RecipeFields } from "#shared/utils/recipe"
import { nutritionLabels } from "#shared/utils/recipe"
import { recipeToMarkdown } from "#shared/utils/format"

const route = useRoute()
const toast = useToast()
const id = computed(() => Number(route.params.id))

const { data: recipe, status, error } = useFetch(() => `/api/recipes/${id.value}`)

useHead(() => ({ title: recipe.value ? `${recipe.value.title} · Just the Recipe` : "Recipe" }))

// ─── Cooking helpers: tick off ingredients/steps, keep the screen awake ───
const checkedIngredients = ref(new Set<string>())
const currentStep = ref<number | null>(null)
const wakeLock = useWakeLock()

function toggleIngredient(key: string) {
  const next = new Set(checkedIngredients.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  checkedIngredients.value = next
}

async function toggleCookMode() {
  if (wakeLock.isActive.value) await wakeLock.release()
  else await wakeLock.request("screen")
  toast.add({
    title: wakeLock.isActive.value ? "Cook mode on" : "Cook mode off",
    description: wakeLock.isActive.value ? "Your screen will stay awake." : undefined,
    icon: "i-lucide-flame",
  })
}

function stepOffset(sectionIndex: number) {
  return (recipe.value?.instructions ?? [])
    .slice(0, sectionIndex)
    .reduce((n, s) => n + s.items.length, 0)
}

const meta = computed(() => {
  const r = recipe.value
  if (!r) return []
  return [
    { icon: "i-lucide-timer", label: "Prep", value: r.prepTime },
    { icon: "i-lucide-flame", label: "Cook", value: r.cookTime },
    { icon: "i-lucide-snowflake", label: "Extra", value: r.freezeTime },
    { icon: "i-lucide-clock", label: "Total", value: r.totalTime },
    { icon: "i-lucide-users", label: "Yield", value: r.recipeYield },
  ].filter((m) => m.value)
})

const sourceHost = computed(() => {
  if (!recipe.value?.url) return null
  try {
    return new URL(recipe.value.url).hostname.replace(/^www\./, "")
  } catch {
    return null
  }
})

const imageFailed = ref(false)

// ─── Actions ───
const { copy, isSupported: canCopy } = useClipboard()
const { share: webShare, isSupported: canShare } = useShare()

async function copyRecipe() {
  if (!recipe.value) return
  await copy(recipeToMarkdown(recipe.value))
  toast.add({ title: "Recipe copied", icon: "i-lucide-clipboard-check" })
}

function shareRecipe() {
  if (!recipe.value) return
  void webShare({ title: recipe.value.title, text: recipeToMarkdown(recipe.value) })
}

const editing = ref(false)
const saving = ref(false)

async function save(fields: RecipeFields) {
  saving.value = true
  try {
    recipe.value = await $fetch(`/api/recipes/${id.value}`, { method: "PATCH", body: fields })
    editing.value = false
    toast.add({ title: "Recipe saved", color: "success" })
  } catch (e) {
    toast.add({ title: "Couldn't save", description: errorMessage(e), color: "error" })
  } finally {
    saving.value = false
  }
}

const showDelete = ref(false)
async function deleteRecipe() {
  try {
    await $fetch(`/api/recipes/${id.value}`, { method: "DELETE" })
    toast.add({ title: "Recipe deleted" })
    await navigateTo("/recipes")
  } catch (e) {
    toast.add({ title: "Couldn't delete", description: errorMessage(e), color: "error" })
  }
}

// ─── Cookbooks ───
const { data: cookbooks } = useFetch("/api/cookbooks", { key: "cookbooks", lazy: true })
const { data: inCookbooks, refresh: refreshInCookbooks } = useFetch(
  () => `/api/recipes/${id.value}/cookbooks`,
  { lazy: true, default: () => [] as number[] },
)

async function toggleCookbook(bookId: number) {
  const inBook = inCookbooks.value.includes(bookId)
  try {
    if (inBook) {
      await $fetch(`/api/cookbooks/${bookId}/recipes/${id.value}`, { method: "DELETE" })
    } else {
      await $fetch(`/api/cookbooks/${bookId}/recipes`, {
        method: "POST",
        body: { recipeIds: [id.value] },
      })
    }
    await refreshInCookbooks()
  } catch (e) {
    toast.add({ title: "Couldn't update cookbook", description: errorMessage(e), color: "error" })
  }
}

const menuItems = computed<DropdownMenuItem[][]>(() => [
  [
    {
      label: "Edit",
      icon: "i-lucide-pencil",
      onSelect: () => {
        editing.value = true
      },
    },
    ...(canCopy.value
      ? [{ label: "Copy as text", icon: "i-lucide-copy", onSelect: copyRecipe }]
      : []),
    ...(canShare.value ? [{ label: "Share", icon: "i-lucide-share", onSelect: shareRecipe }] : []),
    { label: "Print", icon: "i-lucide-printer", onSelect: () => window.print() },
  ],
  [
    {
      label: "Delete",
      icon: "i-lucide-trash-2",
      color: "error",
      onSelect: () => {
        showDelete.value = true
      },
    },
  ],
])
</script>

<template>
  <div>
    <div v-if="status === 'pending' && !recipe" class="space-y-4">
      <USkeleton class="h-8 w-2/3" />
      <USkeleton class="h-64 w-full rounded-2xl" />
      <USkeleton class="h-4 w-1/2" />
    </div>

    <EmptyState
      v-else-if="error || !recipe"
      icon="i-lucide-file-question"
      title="Recipe not found"
      description="It may have been deleted."
    >
      <UButton to="/recipes" label="Back to recipes" variant="soft" />
    </EmptyState>

    <div v-else-if="editing">
      <h1 class="mb-6 font-serif text-2xl font-semibold">Edit recipe</h1>
      <RecipeEditor :initial="recipe" :saving="saving" @save="save" @cancel="editing = false" />
    </div>

    <article v-else class="mx-auto max-w-4xl">
      <!-- Header -->
      <div class="mb-6 flex items-start justify-between gap-4">
        <div class="min-w-0">
          <div class="text-muted mb-1 flex flex-wrap items-center gap-x-2 text-sm">
            <span v-if="recipe.recipeCategory">{{ recipe.recipeCategory }}</span>
            <span v-if="recipe.recipeCategory && recipe.recipeCuisine">·</span>
            <span v-if="recipe.recipeCuisine">{{ recipe.recipeCuisine }}</span>
          </div>
          <h1 class="font-serif text-3xl leading-tight font-semibold text-balance sm:text-4xl">
            {{ recipe.title }}
          </h1>
          <p v-if="recipe.author || sourceHost" class="text-muted mt-2 text-sm">
            <span v-if="recipe.author">By {{ recipe.author }}</span>
            <span v-if="recipe.author && sourceHost"> · </span>
            <a
              v-if="sourceHost"
              :href="recipe.url!"
              target="_blank"
              rel="noopener noreferrer"
              class="hover:text-primary inline-flex items-center gap-1 underline-offset-2 hover:underline"
            >
              {{ sourceHost }}<UIcon name="i-lucide-external-link" class="size-3" />
            </a>
          </p>
        </div>
        <div class="no-print flex shrink-0 gap-1">
          <UButton
            v-if="wakeLock.isSupported.value"
            :icon="wakeLock.isActive.value ? 'i-lucide-flame' : 'i-lucide-flame-kindling'"
            :variant="wakeLock.isActive.value ? 'solid' : 'outline'"
            :color="wakeLock.isActive.value ? 'primary' : 'neutral'"
            :label="wakeLock.isActive.value ? 'Cooking' : 'Cook mode'"
            class="hidden sm:inline-flex"
            @click="toggleCookMode"
          />
          <UButton
            v-if="wakeLock.isSupported.value"
            icon="i-lucide-flame"
            :variant="wakeLock.isActive.value ? 'solid' : 'outline'"
            :color="wakeLock.isActive.value ? 'primary' : 'neutral'"
            aria-label="Cook mode"
            class="sm:hidden"
            @click="toggleCookMode"
          />
          <UDropdownMenu :items="menuItems" :content="{ align: 'end' }">
            <UButton icon="i-lucide-ellipsis" variant="outline" color="neutral" aria-label="More" />
          </UDropdownMenu>
        </div>
      </div>

      <img
        v-if="recipe.image && !imageFailed"
        :src="recipe.image"
        :alt="recipe.title"
        referrerpolicy="no-referrer"
        decoding="async"
        fetchpriority="high"
        class="mb-6 aspect-[16/9] w-full rounded-2xl object-cover"
        @error="imageFailed = true"
      />

      <p v-if="recipe.description" class="text-muted mb-6 text-pretty">{{ recipe.description }}</p>

      <dl
        v-if="meta.length"
        class="border-default mb-8 flex flex-wrap gap-x-6 gap-y-3 border-y py-4"
      >
        <div v-for="m in meta" :key="m.label" class="flex items-center gap-2">
          <UIcon :name="m.icon" class="text-primary size-4" />
          <dt class="text-muted text-sm">{{ m.label }}</dt>
          <dd class="text-sm font-medium">{{ m.value }}</dd>
        </div>
      </dl>

      <div class="grid gap-10 md:grid-cols-5">
        <!-- Ingredients -->
        <section class="md:col-span-2">
          <div class="md:sticky md:top-20">
            <h2 class="mb-3 font-serif text-xl font-semibold">Ingredients</h2>
            <p v-if="!recipe.ingredients.length" class="text-muted text-sm">
              No ingredients listed.
            </p>
            <div v-for="(section, si) in recipe.ingredients" :key="si" :class="{ 'mt-5': si > 0 }">
              <h3
                v-if="section.name"
                class="text-muted mb-2 text-xs font-semibold tracking-wider uppercase"
              >
                {{ section.name }}
              </h3>
              <ul class="divide-default divide-y">
                <li v-for="(item, ii) in section.items" :key="ii">
                  <label class="flex cursor-pointer items-start gap-3 py-2.5">
                    <UCheckbox
                      :model-value="checkedIngredients.has(`${si}-${ii}`)"
                      class="no-print mt-0.5"
                      @update:model-value="toggleIngredient(`${si}-${ii}`)"
                    />
                    <span
                      class="leading-snug transition"
                      :class="{ 'text-dimmed line-through': checkedIngredients.has(`${si}-${ii}`) }"
                    >
                      {{ item }}
                    </span>
                  </label>
                </li>
              </ul>
            </div>
          </div>
        </section>

        <!-- Instructions -->
        <section class="md:col-span-3">
          <h2 class="mb-3 font-serif text-xl font-semibold">Instructions</h2>
          <p v-if="!recipe.instructions.length" class="text-muted text-sm">No steps listed.</p>
          <div v-for="(section, si) in recipe.instructions" :key="si" :class="{ 'mt-6': si > 0 }">
            <h3
              v-if="section.name"
              class="text-muted mb-3 text-xs font-semibold tracking-wider uppercase"
            >
              {{ section.name }}
            </h3>
            <ol class="space-y-2">
              <li v-for="(step, i) in section.items" :key="i">
                <button
                  type="button"
                  class="flex w-full gap-4 rounded-xl p-3 text-left transition"
                  :class="
                    currentStep === stepOffset(si) + i
                      ? 'bg-primary/10 ring-primary/30 ring-1'
                      : 'hover:bg-elevated/60'
                  "
                  @click="
                    () => {
                      currentStep = currentStep === stepOffset(si) + i ? null : stepOffset(si) + i
                    }
                  "
                >
                  <span
                    class="grid size-7 shrink-0 place-items-center rounded-full text-sm font-semibold"
                    :class="
                      currentStep === stepOffset(si) + i
                        ? 'bg-primary text-inverted'
                        : 'bg-elevated text-muted'
                    "
                  >
                    {{ stepOffset(si) + i + 1 }}
                  </span>
                  <span class="pt-0.5 leading-relaxed">{{ step }}</span>
                </button>
              </li>
            </ol>
          </div>

          <div v-if="recipe.notes" class="bg-elevated/50 mt-8 rounded-xl p-4">
            <h2 class="mb-2 flex items-center gap-2 font-semibold">
              <UIcon name="i-lucide-sticky-note" class="text-primary size-4" /> Notes
            </h2>
            <p class="text-muted text-sm whitespace-pre-line">{{ recipe.notes }}</p>
          </div>

          <div v-if="recipe.nutrition" class="mt-8">
            <h2 class="mb-3 font-semibold">Nutrition</h2>
            <dl class="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <div
                v-for="(value, key) in recipe.nutrition"
                :key="key"
                class="bg-elevated/50 rounded-lg px-3 py-2"
              >
                <dt class="text-muted text-xs">
                  {{ nutritionLabels[key] ?? key.replace(/Content$/, "") }}
                </dt>
                <dd class="text-sm font-medium">{{ value }}</dd>
              </div>
            </dl>
          </div>
        </section>
      </div>

      <!-- Cookbooks -->
      <section v-if="cookbooks?.length" class="border-default no-print mt-10 border-t pt-6">
        <h2 class="text-muted mb-3 text-sm font-semibold">In cookbooks</h2>
        <div class="flex flex-wrap gap-2">
          <UButton
            v-for="book in cookbooks"
            :key="book.id"
            :label="book.name"
            :icon="inCookbooks.includes(book.id) ? 'i-lucide-check' : 'i-lucide-plus'"
            :variant="inCookbooks.includes(book.id) ? 'soft' : 'outline'"
            :color="inCookbooks.includes(book.id) ? 'primary' : 'neutral'"
            size="sm"
            @click="toggleCookbook(book.id)"
          />
        </div>
      </section>
    </article>

    <UModal
      v-model:open="showDelete"
      title="Delete this recipe?"
      description="This can't be undone."
    >
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton
            label="Cancel"
            variant="ghost"
            color="neutral"
            @click="
              () => {
                showDelete = false
              }
            "
          />
          <UButton label="Delete" color="error" @click="deleteRecipe" />
        </div>
      </template>
    </UModal>
  </div>
</template>
