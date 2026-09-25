<script setup lang="ts">
import type { DropdownMenuItem } from "@nuxt/ui"

const route = useRoute()
const toast = useToast()
const id = computed(() => Number(route.params.id))

const { data: cookbook, status, error, refresh } = useFetch(() => `/api/cookbooks/${id.value}`)
useHead(() => ({ title: cookbook.value ? `${cookbook.value.name} · Just the Recipe` : "Cookbook" }))

const editing = ref(false)
const form = reactive({ name: "", description: "", color: "tomato" })
const showDelete = ref(false)

function startEdit() {
  if (!cookbook.value) return
  form.name = cookbook.value.name
  form.description = cookbook.value.description ?? ""
  form.color = cookbook.value.color ?? "tomato"
  editing.value = true
}

async function saveEdit() {
  try {
    await $fetch(`/api/cookbooks/${id.value}`, { method: "PATCH", body: form })
    editing.value = false
    await refresh()
  } catch (e) {
    toast.add({ title: "Couldn't save", description: errorMessage(e), color: "error" })
  }
}

async function removeRecipe(recipeId: number) {
  try {
    await $fetch(`/api/cookbooks/${id.value}/recipes/${recipeId}`, { method: "DELETE" })
    await refresh()
  } catch (e) {
    toast.add({ title: "Couldn't remove recipe", description: errorMessage(e), color: "error" })
  }
}

async function deleteCookbook() {
  try {
    await $fetch(`/api/cookbooks/${id.value}`, { method: "DELETE" })
    toast.add({ title: "Cookbook deleted" })
    await navigateTo("/cookbooks")
  } catch (e) {
    toast.add({ title: "Couldn't delete", description: errorMessage(e), color: "error" })
  }
}

const menuItems: DropdownMenuItem[][] = [
  [{ label: "Rename", icon: "i-lucide-pencil", onSelect: startEdit }],
  [
    {
      label: "Delete cookbook",
      icon: "i-lucide-trash-2",
      color: "error",
      onSelect: () => {
        showDelete.value = true
      },
    },
  ],
]
</script>

<template>
  <div>
    <RecipeGrid v-if="status === 'pending' && !cookbook" loading :count="4" />

    <EmptyState
      v-else-if="error || !cookbook"
      icon="i-lucide-file-question"
      title="Cookbook not found"
    >
      <UButton to="/cookbooks" label="All cookbooks" variant="soft" />
    </EmptyState>

    <template v-else>
      <div class="mb-6">
        <UButton
          to="/cookbooks"
          label="Shelf"
          icon="i-lucide-arrow-left"
          variant="link"
          color="neutral"
          class="mb-2 -ml-2.5"
        />
        <form v-if="editing" class="space-y-3" @submit.prevent="saveEdit">
          <UInput v-model="form.name" size="xl" class="w-full" autofocus />
          <UTextarea
            v-model="form.description"
            placeholder="Description (optional)"
            autoresize
            class="w-full"
          />
          <BookColorPicker v-model="form.color" />
          <div class="flex gap-2">
            <UButton type="submit" label="Save" />
            <UButton
              label="Cancel"
              variant="ghost"
              color="neutral"
              @click="
                () => {
                  editing = false
                }
              "
            />
          </div>
        </form>
        <div v-else class="flex items-start justify-between gap-3">
          <div class="flex items-start gap-4">
            <!-- a little cover, in the book's cloth -->
            <div
              class="grid h-24 w-18 shrink-0 place-items-center rounded-l-sm rounded-r-lg shadow-md"
              :style="{
                background: bookPalette(cookbook.color).cloth,
                color: bookPalette(cookbook.color).foil,
              }"
            >
              <UIcon name="i-lucide-chef-hat" class="size-7" />
            </div>
            <div>
              <h1 class="font-serif text-3xl font-semibold sm:text-4xl">{{ cookbook.name }}</h1>
              <p class="text-muted mt-1">
                {{ cookbook.recipes.length }} recipe{{ cookbook.recipes.length === 1 ? "" : "s" }}
                <template v-if="cookbook.description"> · {{ cookbook.description }}</template>
              </p>
            </div>
          </div>
          <UDropdownMenu :items="menuItems" :content="{ align: 'end' }">
            <UButton icon="i-lucide-ellipsis" variant="outline" color="neutral" aria-label="More" />
          </UDropdownMenu>
        </div>
      </div>

      <EmptyState
        v-if="!cookbook.recipes.length"
        icon="i-lucide-book-open"
        title="This cookbook is empty"
        description="Open a recipe and tap the cookbook name, or use Select on the recipes page."
      >
        <UButton to="/recipes" label="Browse recipes" variant="soft" />
      </EmptyState>

      <RecipeGrid v-else>
        <div v-for="recipe in cookbook.recipes" :key="recipe.id" class="relative">
          <RecipeCard :recipe="recipe" />
          <UButton
            icon="i-lucide-x"
            size="xs"
            color="neutral"
            variant="solid"
            class="absolute top-2 right-2 rounded-full opacity-90"
            aria-label="Remove from cookbook"
            @click="removeRecipe(recipe.id)"
          />
        </div>
      </RecipeGrid>
    </template>

    <UModal
      v-model:open="showDelete"
      title="Delete this cookbook?"
      description="Recipes in it are kept."
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
          <UButton label="Delete" color="error" @click="deleteCookbook" />
        </div>
      </template>
    </UModal>
  </div>
</template>
