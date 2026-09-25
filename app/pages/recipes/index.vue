<script setup lang="ts">
useHead({ title: "Recipes · Just the Recipe" })

const toast = useToast()
const route = useRoute()
const router = useRouter()

// Search is server-side (title, ingredients, category, cuisine) and mirrored in the URL
const search = ref(typeof route.query.q === "string" ? route.query.q : "")
const q = refDebounced(search, 250)
watch(q, (value) => router.replace({ query: value ? { q: value } : {} }))

const {
  data: recipes,
  status,
  refresh,
} = useFetch("/api/recipes", {
  query: { q },
  key: "recipes",
})

const category = ref<string>("all")
const categories = computed(() => {
  const set = new Set(
    (recipes.value ?? []).map((r) => r.recipeCategory).filter(Boolean) as string[],
  )
  return [...set].toSorted((a, b) => a.localeCompare(b))
})
const categoryItems = computed(() => [
  { label: "Everything", value: "all" },
  ...categories.value.map((c) => ({ label: c, value: c })),
])
const visible = computed(() =>
  category.value === "all"
    ? (recipes.value ?? [])
    : (recipes.value ?? []).filter((r) => r.recipeCategory === category.value),
)

// Selection
const selecting = ref(false)
const selected = ref(new Set<number>())
const busy = ref(false)
const showDelete = ref(false)
const showCookbooks = ref(false)

function toggleSelecting() {
  selecting.value = !selecting.value
  selected.value = new Set()
}

function toggle(id: number) {
  const next = new Set(selected.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  selected.value = next
}

const allSelected = computed(
  () => visible.value.length > 0 && visible.value.every((r) => selected.value.has(r.id)),
)

function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(visible.value.map((r) => r.id))
}

const { data: cookbooks, execute: loadCookbooks } = useFetch("/api/cookbooks", {
  immediate: false,
  key: "cookbooks",
})

async function openCookbooks() {
  await loadCookbooks()
  showCookbooks.value = true
}

async function addToCookbook(id: number, name: string) {
  busy.value = true
  try {
    const ids = [...selected.value]
    await $fetch(`/api/cookbooks/${id}/recipes`, { method: "POST", body: { recipeIds: ids } })
    toast.add({ title: `Added ${ids.length} to ${name}`, color: "success" })
    showCookbooks.value = false
    toggleSelecting()
  } catch (e) {
    toast.add({ title: "Couldn't add to cookbook", description: errorMessage(e), color: "error" })
  } finally {
    busy.value = false
  }
}

async function deleteSelected() {
  busy.value = true
  try {
    const ids = [...selected.value]
    await $fetch("/api/recipes/bulk-delete", { method: "POST", body: { ids } })
    toast.add({
      title: `Deleted ${ids.length} recipe${ids.length === 1 ? "" : "s"}`,
      color: "success",
    })
    showDelete.value = false
    toggleSelecting()
    await refresh()
  } catch (e) {
    toast.add({ title: "Couldn't delete", description: errorMessage(e), color: "error" })
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div>
    <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
      <h1 class="font-serif text-3xl font-semibold sm:text-4xl">Recipes</h1>
      <div class="flex gap-2">
        <UButton
          v-if="recipes?.length"
          :label="selecting ? 'Done' : 'Select'"
          :icon="selecting ? 'i-lucide-x' : 'i-lucide-square-check'"
          variant="outline"
          color="neutral"
          @click="toggleSelecting"
        />
        <UButton to="/add" label="Add" icon="i-lucide-plus" class="rounded-full" />
      </div>
    </div>

    <div class="mb-6 space-y-3">
      <UInput
        v-model="search"
        icon="i-lucide-search"
        placeholder="Search by name or ingredient…"
        class="w-full"
        size="xl"
        :ui="{ base: 'rounded-full bg-(--paper)' }"
        :loading="status === 'pending' && !!search"
      >
        <template v-if="search" #trailing>
          <UButton
            icon="i-lucide-x"
            variant="link"
            color="neutral"
            size="xs"
            aria-label="Clear"
            @click="
              () => {
                search = ''
              }
            "
          />
        </template>
      </UInput>
      <div v-if="categories.length > 1" class="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4">
        <button
          v-for="c in categoryItems"
          :key="c.value"
          type="button"
          class="flex-none rounded-full px-4 py-1.5 text-sm font-semibold transition"
          :class="
            category === c.value
              ? 'bg-primary text-white'
              : 'bg-elevated text-muted hover:text-default'
          "
          @click="
            () => {
              category = c.value
            }
          "
        >
          {{ c.label }}
        </button>
      </div>
    </div>

    <RecipeGrid v-if="status === 'pending' && !recipes" loading />

    <EmptyState
      v-else-if="!recipes?.length && !q"
      icon="i-lucide-book-open"
      title="No recipes yet"
      description="Paste a link or some recipe text to save your first one."
    >
      <UButton to="/add" label="Add a recipe" icon="i-lucide-plus" />
    </EmptyState>

    <EmptyState
      v-else-if="!visible.length"
      icon="i-lucide-search-x"
      title="Nothing matches"
      :description="q ? `No recipes found for “${q}”.` : 'Try another category.'"
    >
      <UButton
        label="Clear search"
        variant="soft"
        @click="
          () => {
            search = ''
            category = 'all'
          }
        "
      />
    </EmptyState>

    <RecipeGrid v-else>
      <RecipeCard
        v-for="recipe in visible"
        :key="recipe.id"
        :recipe="recipe"
        :selectable="selecting"
        :selected="selected.has(recipe.id)"
        @toggle="toggle"
      />
    </RecipeGrid>

    <!-- Selection action bar -->
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-y-full opacity-0"
      leave-active-class="transition duration-150 ease-in"
      leave-to-class="translate-y-full opacity-0"
    >
      <div
        v-if="selecting"
        class="bg-elevated border-default fixed inset-x-0 bottom-[calc(4.5rem+env(safe-area-inset-bottom))] z-50 border-t px-4 py-3 shadow-lg sm:bottom-0"
      >
        <div class="mx-auto flex max-w-5xl items-center justify-between gap-2">
          <UButton
            :label="allSelected ? 'Clear' : 'Select all'"
            variant="ghost"
            color="neutral"
            size="sm"
            @click="toggleAll"
          />
          <span class="text-sm font-medium">{{ selected.size }} selected</span>
          <div class="flex gap-2">
            <UButton
              icon="i-lucide-book-plus"
              label="Cookbook"
              variant="soft"
              size="sm"
              :disabled="!selected.size"
              @click="openCookbooks"
            />
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="soft"
              size="sm"
              aria-label="Delete selected"
              :disabled="!selected.size"
              @click="
                () => {
                  showDelete = true
                }
              "
            />
          </div>
        </div>
      </div>
    </Transition>

    <UModal v-model:open="showCookbooks" title="Add to cookbook">
      <template #body>
        <div v-if="!cookbooks?.length" class="text-center">
          <p class="text-muted">You don't have any cookbooks yet.</p>
          <UButton to="/cookbooks" label="Create a cookbook" variant="soft" class="mt-3" />
        </div>
        <div v-else class="flex flex-col gap-1">
          <UButton
            v-for="book in cookbooks"
            :key="book.id"
            :label="book.name"
            icon="i-lucide-book-marked"
            variant="ghost"
            color="neutral"
            block
            class="justify-start"
            :disabled="busy"
            @click="addToCookbook(book.id, book.name)"
          />
        </div>
      </template>
    </UModal>

    <UModal
      v-model:open="showDelete"
      title="Delete recipes?"
      :description="`${selected.size} recipe${selected.size === 1 ? '' : 's'} will be permanently deleted.`"
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
          <UButton label="Delete" color="error" :loading="busy" @click="deleteSelected" />
        </div>
      </template>
    </UModal>
  </div>
</template>
