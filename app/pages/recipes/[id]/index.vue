<script setup lang="ts">
import type { DropdownMenuItem } from "@nuxt/ui"
import type { RecipeFields } from "#shared/utils/recipe"
import { nutritionLabels } from "#shared/utils/recipe"
import { recipeToMarkdown } from "#shared/utils/format"
import { scaleIngredient } from "#shared/utils/ingredients"

const toast = useToast()
const { id, recipe, status, error } = useRecipe()
const { remember, forget } = useRecentlyViewed()

useHead(() => ({ title: recipe.value ? `${recipe.value.title} · Just the Recipe` : "Recipe" }))

watch(
  recipe,
  (r) => {
    if (r && import.meta.client) remember(r)
  },
  { immediate: true },
)

// ─── Scaling & checklists ───
const scale = useState(`scale-${id.value}`, () => 1)
const checked = ref(new Set<string>())
function toggle(key: string) {
  const next = new Set(checked.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  checked.value = next
}

const scaledYield = computed(() => {
  const y = recipe.value?.recipeYield
  if (!y || scale.value === 1) return y
  return scaleIngredient(y, scale.value)
})

const meta = computed(() => {
  const r = recipe.value
  if (!r) return []
  return [
    { icon: "i-lucide-timer", label: "Prep", value: r.prepTime },
    { icon: "i-lucide-flame", label: "Cook", value: r.cookTime },
    { icon: "i-lucide-snowflake", label: "Extra", value: r.freezeTime },
    { icon: "i-lucide-clock", label: "Total", value: r.totalTime },
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

const ingredientCount = computed(
  () => recipe.value?.ingredients.reduce((n, s) => n + s.items.length, 0) ?? 0,
)
const imageFailed = ref(false)

function stepOffset(sectionIndex: number) {
  return (recipe.value?.instructions ?? [])
    .slice(0, sectionIndex)
    .reduce((n, s) => n + s.items.length, 0)
}

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
    toast.add({ title: "Saved", icon: "i-lucide-check", color: "success" })
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
    forget(id.value)
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
  {
    lazy: true,
    default: () => [] as number[],
  },
)

async function toggleCookbook(bookId: number) {
  const inBook = inCookbooks.value.includes(bookId)
  try {
    if (inBook) await $fetch(`/api/cookbooks/${bookId}/recipes/${id.value}`, { method: "DELETE" })
    else
      await $fetch(`/api/cookbooks/${bookId}/recipes`, {
        method: "POST",
        body: { recipeIds: [id.value] },
      })
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
    <div v-if="status === 'pending' && !recipe" class="mx-auto max-w-4xl space-y-4">
      <USkeleton class="aspect-[16/8] w-full rounded-3xl" />
      <USkeleton class="h-10 w-2/3" />
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

    <div v-else-if="editing" class="mx-auto max-w-4xl">
      <h1 class="mb-6 font-serif text-3xl font-semibold">Edit recipe</h1>
      <RecipeEditor
        :initial="recipe"
        :saving="saving"
        @save="save"
        @cancel="
          () => {
            editing = false
          }
        "
      />
    </div>

    <article v-else class="mx-auto max-w-4xl">
      <!-- Hero -->
      <div class="relative mb-6">
        <div
          v-if="recipe.image && !imageFailed"
          class="relative aspect-[16/10] overflow-hidden rounded-3xl shadow-sm sm:aspect-[16/8]"
        >
          <img
            :src="recipe.image"
            :alt="recipe.title"
            referrerpolicy="no-referrer"
            decoding="async"
            fetchpriority="high"
            class="size-full object-cover"
            @error="imageFailed = true"
          />
          <div
            class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/10 to-transparent"
          />
          <div class="absolute inset-x-0 bottom-0 p-5 text-white sm:p-8">
            <p
              v-if="recipe.recipeCategory || recipe.recipeCuisine"
              class="text-sm font-medium opacity-90"
            >
              {{ [recipe.recipeCategory, recipe.recipeCuisine].filter(Boolean).join(" · ") }}
            </p>
            <h1 class="font-serif text-3xl leading-tight font-semibold text-balance sm:text-5xl">
              {{ recipe.title }}
            </h1>
          </div>
        </div>
        <div v-else class="pt-2 pr-12">
          <p
            v-if="recipe.recipeCategory || recipe.recipeCuisine"
            class="text-primary text-sm font-semibold"
          >
            {{ [recipe.recipeCategory, recipe.recipeCuisine].filter(Boolean).join(" · ") }}
          </p>
          <h1 class="font-serif text-4xl leading-tight font-semibold text-balance sm:text-5xl">
            {{ recipe.title }}
          </h1>
        </div>
        <div
          class="no-print absolute right-3"
          :class="recipe.image && !imageFailed ? 'top-3' : 'top-2 right-0'"
        >
          <UDropdownMenu :items="menuItems" :content="{ align: 'end' }">
            <UButton
              icon="i-lucide-ellipsis"
              color="neutral"
              :variant="recipe.image && !imageFailed ? 'solid' : 'outline'"
              class="rounded-full"
              aria-label="More"
            />
          </UDropdownMenu>
        </div>
      </div>

      <p v-if="recipe.author || sourceHost" class="text-muted -mt-2 mb-4 text-sm">
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

      <p v-if="recipe.description" class="text-muted mb-6 text-lg text-pretty">
        {{ recipe.description }}
      </p>

      <!-- Primary actions -->
      <div class="no-print mb-6 grid grid-cols-2 gap-3 sm:flex">
        <UButton
          :to="`/recipes/${recipe.id}/cook`"
          label="Start cooking"
          icon="i-lucide-flame"
          size="xl"
          class="justify-center rounded-2xl"
        />
        <UButton
          :to="`/recipes/${recipe.id}/prep`"
          label="Mise en place"
          icon="i-lucide-soup"
          size="xl"
          variant="soft"
          class="justify-center rounded-2xl"
        />
      </div>

      <div class="border-default mb-8 flex flex-wrap items-center gap-x-6 gap-y-3 border-y py-4">
        <div v-for="m in meta" :key="m.label" class="flex items-center gap-2 text-sm">
          <UIcon :name="m.icon" class="text-primary size-4" />
          <span class="text-muted">{{ m.label }}</span>
          <span class="font-semibold">{{ m.value }}</span>
        </div>
        <div v-if="scaledYield" class="flex items-center gap-2 text-sm">
          <UIcon name="i-lucide-users" class="text-primary size-4" />
          <span class="font-semibold">{{ scaledYield }}</span>
        </div>
      </div>

      <div class="grid gap-10 md:grid-cols-5">
        <section class="md:col-span-2">
          <div class="md:sticky md:top-20">
            <div class="mb-3 flex items-center justify-between gap-2">
              <h2 class="font-serif text-2xl font-semibold">Ingredients</h2>
              <ScaleControl v-if="ingredientCount" v-model="scale" class="no-print" />
            </div>
            <p v-if="!ingredientCount" class="text-muted text-sm">No ingredients listed.</p>
            <div v-for="(section, si) in recipe.ingredients" :key="si" :class="{ 'mt-5': si > 0 }">
              <h3
                v-if="section.name"
                class="text-primary mb-1 text-xs font-bold tracking-wider uppercase"
              >
                {{ section.name }}
              </h3>
              <ul>
                <li v-for="(item, ii) in section.items" :key="ii">
                  <button
                    type="button"
                    class="flex w-full items-start gap-3 rounded-xl px-2 py-2.5 text-left transition hover:bg-(--ui-bg-elevated)/60"
                    @click="toggle(`${si}-${ii}`)"
                  >
                    <span
                      class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-md border-2 transition"
                      :class="
                        checked.has(`${si}-${ii}`)
                          ? 'bg-primary border-primary'
                          : 'border-(--ui-border-accented)'
                      "
                    >
                      <UIcon
                        v-if="checked.has(`${si}-${ii}`)"
                        name="i-lucide-check"
                        class="size-3.5 text-white"
                      />
                    </span>
                    <span
                      class="leading-snug transition"
                      :class="{ 'text-dimmed line-through': checked.has(`${si}-${ii}`) }"
                    >
                      {{ scaleIngredient(item, scale) }}
                    </span>
                  </button>
                </li>
              </ul>
            </div>
          </div>
        </section>

        <section class="md:col-span-3">
          <h2 class="mb-3 font-serif text-2xl font-semibold">Method</h2>
          <p v-if="!recipe.instructions.length" class="text-muted text-sm">No steps listed.</p>
          <div v-for="(section, si) in recipe.instructions" :key="si" :class="{ 'mt-6': si > 0 }">
            <h3
              v-if="section.name"
              class="text-primary mb-2 text-xs font-bold tracking-wider uppercase"
            >
              {{ section.name }}
            </h3>
            <ol class="space-y-4">
              <li v-for="(step, i) in section.items" :key="i" class="flex gap-4">
                <span
                  class="bg-primary/10 text-primary grid size-8 shrink-0 place-items-center rounded-full font-serif font-semibold"
                >
                  {{ stepOffset(si) + i + 1 }}
                </span>
                <p class="pt-1 text-[1.05rem] leading-relaxed">{{ step }}</p>
              </li>
            </ol>
          </div>

          <div v-if="recipe.notes" class="mt-8 rounded-2xl bg-amber-50 p-5 dark:bg-amber-950/30">
            <h2
              class="mb-1 flex items-center gap-2 font-semibold text-amber-800 dark:text-amber-300"
            >
              <UIcon name="i-lucide-sticky-note" class="size-4" /> Notes
            </h2>
            <p class="text-sm whitespace-pre-line text-amber-900/90 dark:text-amber-100/80">
              {{ recipe.notes }}
            </p>
          </div>

          <div v-if="recipe.nutrition" class="mt-8">
            <h2 class="mb-3 font-semibold">Nutrition</h2>
            <dl class="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <div
                v-for="(value, key) in recipe.nutrition"
                :key="key"
                class="bg-elevated/60 rounded-xl px-3 py-2"
              >
                <dt class="text-muted text-xs">
                  {{ nutritionLabels[key] ?? key.replace(/Content$/, "") }}
                </dt>
                <dd class="text-sm font-semibold">{{ value }}</dd>
              </div>
            </dl>
          </div>
        </section>
      </div>

      <section v-if="cookbooks?.length" class="border-default no-print mt-12 border-t pt-6">
        <h2 class="text-muted mb-3 text-sm font-semibold">On the shelf in</h2>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="book in cookbooks"
            :key="book.id"
            type="button"
            class="flex items-center gap-2 rounded-full border py-1 pr-3 pl-1 text-sm font-medium transition"
            :class="
              inCookbooks.includes(book.id)
                ? 'border-transparent bg-(--ui-bg-elevated)'
                : 'border-default text-muted hover:text-default'
            "
            @click="toggleCookbook(book.id)"
          >
            <span
              class="h-6 w-2.5 rounded-sm"
              :style="{ background: bookPalette(book.color).cloth }"
            />
            {{ book.name }}
            <UIcon
              :name="inCookbooks.includes(book.id) ? 'i-lucide-check' : 'i-lucide-plus'"
              class="size-3.5"
            />
          </button>
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
