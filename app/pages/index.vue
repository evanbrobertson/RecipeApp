<script setup lang="ts">
useHead({ title: "Add a recipe · Just the Recipe" })

const toast = useToast()
const input = ref("")
const saving = ref(false)

const trimmed = computed(() => input.value.trim())
const isUrl = computed(() => /^https?:\/\/\S+$/i.test(trimmed.value))
const canSubmit = computed(() => isUrl.value || trimmed.value.length >= 10)

// Prefill from the PWA share target (/?url=…&text=…)
const route = useRoute()
onMounted(() => {
  const shared = [route.query.url, route.query.text].find(
    (v): v is string => typeof v === "string" && v.trim().length > 0,
  )
  if (shared) input.value = shared.trim()
})

const { data: recent, status: recentStatus } = useFetch("/api/recipes", {
  query: { limit: 8 },
  key: "recent-recipes",
})

async function pasteFromClipboard() {
  try {
    input.value = await navigator.clipboard.readText()
  } catch {
    toast.add({
      title: "Clipboard access was blocked",
      description: "Paste with Ctrl/⌘+V instead.",
    })
  }
}

async function submit() {
  if (!canSubmit.value || saving.value) return
  saving.value = true
  try {
    const body = isUrl.value ? { url: trimmed.value } : { text: input.value }
    const res = await $fetch("/api/recipes/import", { method: "POST", body })
    if (!res.isNew)
      toast.add({ title: "You already saved this one", icon: "i-lucide-bookmark-check" })
    input.value = ""
    await navigateTo(`/recipes/${res.id}`)
  } catch (e) {
    toast.add({
      title: isUrl.value ? "Couldn't get that recipe" : "Couldn't read that recipe",
      description: errorMessage(e),
      color: "error",
    })
  } finally {
    saving.value = false
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit()
}
</script>

<template>
  <div class="space-y-12">
    <section class="mx-auto max-w-2xl text-center">
      <h1 class="font-serif text-3xl font-semibold tracking-tight sm:text-4xl">Just the recipe.</h1>
      <p class="text-muted mt-2 sm:text-lg">
        Paste a link or the recipe text. We'll keep the ingredients and steps and skip the life
        story.
      </p>

      <form class="mt-6 text-left" @submit.prevent="submit">
        <div
          class="border-default bg-default focus-within:ring-primary rounded-2xl border shadow-sm focus-within:ring-2"
        >
          <UTextarea
            v-model="input"
            :rows="isUrl || !input ? 2 : 8"
            :maxrows="16"
            autoresize
            variant="none"
            size="xl"
            class="w-full"
            :ui="{ base: 'px-4 pt-4' }"
            placeholder="https://… or paste the whole recipe here"
            aria-label="Recipe link or text"
            :disabled="saving"
            autofocus
            @keydown="onKeydown"
          />
          <div class="flex items-center justify-between gap-2 px-3 pb-3">
            <div class="text-muted flex items-center gap-2 text-xs">
              <UBadge
                v-if="trimmed"
                :icon="isUrl ? 'i-lucide-link' : 'i-lucide-text'"
                :label="isUrl ? 'Link' : 'Text'"
                variant="subtle"
                color="neutral"
                size="sm"
              />
              <UButton
                v-else
                label="Paste"
                icon="i-lucide-clipboard"
                variant="ghost"
                color="neutral"
                size="xs"
                @click="pasteFromClipboard"
              />
            </div>
            <UButton
              type="submit"
              :label="saving ? (isUrl ? 'Fetching…' : 'Reading…') : 'Save recipe'"
              icon="i-lucide-arrow-right"
              trailing
              :loading="saving"
              :disabled="!canSubmit"
            />
          </div>
        </div>
      </form>

      <p class="text-muted mt-3 text-xs">
        Or
        <NuxtLink to="/recipes/new" class="text-primary underline-offset-2 hover:underline"
          >write one from scratch</NuxtLink
        >
        ·
        <NuxtLink to="/connect" class="text-primary underline-offset-2 hover:underline"
          >save from Claude</NuxtLink
        >
      </p>
    </section>

    <section v-if="recentStatus === 'pending' || recent?.length">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="font-serif text-xl font-semibold">Recently added</h2>
        <UButton
          to="/recipes"
          label="All recipes"
          variant="link"
          trailing
          icon="i-lucide-arrow-right"
        />
      </div>
      <RecipeGrid :loading="recentStatus === 'pending'" :count="4">
        <RecipeCard v-for="recipe in recent" :key="recipe.id" :recipe="recipe" />
      </RecipeGrid>
    </section>
  </div>
</template>
