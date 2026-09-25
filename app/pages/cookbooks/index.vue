<script setup lang="ts">
import { BOOK_COLORS } from "#shared/utils/recipe"

useHead({ title: "Shelf · Just the Recipe" })

const toast = useToast()
const { data: cookbooks, status, refresh } = useFetch("/api/cookbooks", { key: "cookbooks" })

const opened = ref<ShelfBook | null>(null)

const showCreate = ref(false)
const form = reactive({ name: "", description: "", color: "tomato" as string })
const creating = ref(false)

function startCreate() {
  form.name = ""
  form.description = ""
  form.color = BOOK_COLORS[Math.floor(Math.random() * BOOK_COLORS.length)]!
  showCreate.value = true
}

async function create() {
  if (!form.name.trim()) return
  creating.value = true
  try {
    const book = await $fetch("/api/cookbooks", { method: "POST", body: form })
    showCreate.value = false
    await refresh()
    toast.add({ title: `“${book.name}” is on the shelf`, icon: "i-lucide-library-big" })
  } catch (e) {
    toast.add({ title: "Couldn't create cookbook", description: errorMessage(e), color: "error" })
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <div>
    <div class="mb-8 flex items-end justify-between gap-3">
      <div>
        <h1 class="font-serif text-3xl font-semibold sm:text-4xl">Your shelf</h1>
        <p class="text-muted mt-1">Pull a cookbook off the shelf to open it.</p>
      </div>
      <UButton
        label="New cookbook"
        icon="i-lucide-plus"
        class="rounded-full"
        @click="startCreate"
      />
    </div>

    <USkeleton v-if="status === 'pending' && !cookbooks" class="h-64 w-full rounded-2xl" />

    <EmptyState
      v-else-if="!cookbooks?.length"
      icon="i-lucide-library-big"
      title="An empty shelf"
      description="Cookbooks group recipes, like “Weeknight dinners” or “Christmas baking”."
    >
      <UButton label="Make your first cookbook" @click="startCreate" />
    </EmptyState>

    <Bookshelf
      v-else
      :books="cookbooks"
      :pulled-id="opened?.id"
      addable
      @open="opened = $event"
      @add="startCreate"
    />

    <OpenBook :book="opened" @close="opened = null" />

    <UModal v-model:open="showCreate" title="A new cookbook">
      <template #body>
        <form id="create-cookbook" class="space-y-4" @submit.prevent="create">
          <UFormField label="Name" required>
            <UInput v-model="form.name" class="w-full" placeholder="Weeknight dinners" autofocus />
          </UFormField>
          <UFormField label="Description">
            <UTextarea v-model="form.description" autoresize class="w-full" />
          </UFormField>
          <UFormField label="Cover">
            <BookColorPicker v-model="form.color" />
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
            label="Put it on the shelf"
            :loading="creating"
            :disabled="!form.name.trim()"
          />
        </div>
      </template>
    </UModal>
  </div>
</template>
