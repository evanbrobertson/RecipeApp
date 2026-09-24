<script setup lang="ts">
useHead({ title: "Cookbooks · Just the Recipe" })

const toast = useToast()
const { data: cookbooks, status, refresh } = useFetch("/api/cookbooks", { key: "cookbooks" })

const showCreate = ref(false)
const form = reactive({ name: "", description: "" })
const creating = ref(false)

async function create() {
  if (!form.name.trim()) return
  creating.value = true
  try {
    const book = await $fetch("/api/cookbooks", { method: "POST", body: form })
    showCreate.value = false
    form.name = ""
    form.description = ""
    await refresh()
    toast.add({ title: `Created “${book.name}”`, color: "success" })
  } catch (e) {
    toast.add({ title: "Couldn't create cookbook", description: errorMessage(e), color: "error" })
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <div>
    <div class="mb-5 flex items-center justify-between gap-3">
      <h1 class="font-serif text-2xl font-semibold sm:text-3xl">Cookbooks</h1>
      <UButton
        label="New cookbook"
        icon="i-lucide-plus"
        @click="
          () => {
            showCreate = true
          }
        "
      />
    </div>

    <div v-if="status === 'pending' && !cookbooks" class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <USkeleton v-for="i in 3" :key="i" class="h-20 rounded-xl" />
    </div>

    <EmptyState
      v-else-if="!cookbooks?.length"
      icon="i-lucide-library"
      title="No cookbooks yet"
      description="Group recipes into collections like “Weeknight dinners” or “Christmas baking”."
    >
      <UButton
        label="Create a cookbook"
        variant="soft"
        @click="
          () => {
            showCreate = true
          }
        "
      />
    </EmptyState>

    <div v-else class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <CookbookCard v-for="book in cookbooks" :key="book.id" :cookbook="book" />
    </div>

    <UModal v-model:open="showCreate" title="New cookbook">
      <template #body>
        <form id="create-cookbook" class="space-y-3" @submit.prevent="create">
          <UFormField label="Name" required>
            <UInput v-model="form.name" class="w-full" autofocus />
          </UFormField>
          <UFormField label="Description">
            <UTextarea v-model="form.description" autoresize class="w-full" />
          </UFormField>
        </form>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton
            label="Cancel"
            variant="ghost"
            color="neutral"
            @click="
              () => {
                showCreate = false
              }
            "
          />
          <UButton
            type="submit"
            form="create-cookbook"
            label="Create"
            :loading="creating"
            :disabled="!form.name.trim()"
          />
        </div>
      </template>
    </UModal>
  </div>
</template>
