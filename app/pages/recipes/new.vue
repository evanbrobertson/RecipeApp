<script setup lang="ts">
import type { RecipeFields } from "#shared/utils/recipe"

useHead({ title: "New recipe · Just the Recipe" })

const toast = useToast()
const saving = ref(false)

async function save(fields: RecipeFields) {
  saving.value = true
  try {
    const recipe = await $fetch("/api/recipes", { method: "POST", body: fields })
    await navigateTo(`/recipes/${recipe.id}`, { replace: true })
  } catch (e) {
    toast.add({ title: "Couldn't save", description: errorMessage(e), color: "error" })
    saving.value = false
  }
}
</script>

<template>
  <div>
    <h1 class="mb-6 font-serif text-2xl font-semibold sm:text-3xl">New recipe</h1>
    <RecipeEditor
      :saving="saving"
      submit-label="Save recipe"
      @save="save"
      @cancel="navigateTo('/')"
    />
  </div>
</template>
