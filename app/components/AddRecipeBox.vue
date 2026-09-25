<script setup lang="ts">
const props = defineProps<{ autofocus?: boolean; compact?: boolean }>()

const toast = useToast()
const input = ref("")
const saving = ref(false)

const trimmed = computed(() => input.value.trim())
const urls = computed(() => trimmed.value.split(/\s+/).filter((t) => /^https?:\/\/\S+$/i.test(t)))
// Several links pasted at once → bulk import page
const isMultiUrl = computed(
  () => urls.value.length > 1 && trimmed.value.split(/\s+/).length === urls.value.length,
)
const isUrl = computed(() => urls.value.length === 1 && trimmed.value === urls.value[0])
const canSubmit = computed(() => isUrl.value || isMultiUrl.value || trimmed.value.length >= 10)
const expanded = computed(() => !props.compact || !!input.value)

const route = useRoute()
onMounted(() => {
  // Prefill from the PWA share target (/add?url=…&text=…)
  const shared = [route.query.url, route.query.text].find(
    (v): v is string => typeof v === "string" && v.trim().length > 0,
  )
  if (shared) input.value = shared.trim()
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
  if (isMultiUrl.value) {
    await navigateTo({ path: "/import", query: { links: urls.value.join("\n") } })
    return
  }
  saving.value = true
  try {
    const body = isUrl.value ? { url: trimmed.value } : { text: input.value }
    const res = await $fetch("/api/recipes/import", { method: "POST", body })
    if (!res.isNew) toast.add({ title: "Already in your recipes", icon: "i-lucide-bookmark-check" })
    input.value = ""
    await navigateTo(`/recipes/${res.id}`)
  } catch (e) {
    toast.add({
      title: isUrl.value ? "Couldn't get that recipe" : "Couldn't read that recipe",
      description: errorMessage(e),
      color: "error",
      duration: 8000,
    })
  } finally {
    saving.value = false
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit()
  // A single pasted link submits on Enter
  if (e.key === "Enter" && !e.shiftKey && isUrl.value) {
    e.preventDefault()
    submit()
  }
}

const status = computed(() => {
  if (saving.value)
    return isUrl.value
      ? "Fetching the recipe… (tricky sites can take ~20s)"
      : "Reading your recipe…"
  if (isMultiUrl.value) return `${urls.value.length} links. They'll be imported one by one`
  if (isUrl.value) return "Link detected"
  if (trimmed.value) return "Recipe text"
  return ""
})
</script>

<template>
  <form @submit.prevent="submit">
    <div
      class="border-default focus-within:border-primary focus-within:ring-primary/20 rounded-3xl border bg-(--paper) shadow-sm transition focus-within:ring-4"
    >
      <UTextarea
        v-model="input"
        :rows="expanded && !isUrl && input ? 7 : 2"
        :maxrows="16"
        autoresize
        variant="none"
        size="xl"
        class="w-full"
        :ui="{ base: 'px-5 pt-4 text-base sm:text-lg placeholder:text-dimmed' }"
        placeholder="Paste a recipe link, or the whole recipe text…"
        aria-label="Recipe link or text"
        :disabled="saving"
        :autofocus="autofocus"
        @keydown="onKeydown"
      />
      <div class="flex items-center justify-between gap-2 px-3 pb-3">
        <div class="text-muted flex min-w-0 items-center gap-2 pl-2 text-xs">
          <template v-if="status">
            <UIcon
              :name="
                saving
                  ? 'i-lucide-loader-circle'
                  : isUrl || isMultiUrl
                    ? 'i-lucide-link'
                    : 'i-lucide-text'
              "
              class="size-4 shrink-0"
              :class="{ 'animate-spin': saving }"
            />
            <span class="truncate">{{ status }}</span>
          </template>
          <UButton
            v-else
            label="Paste"
            icon="i-lucide-clipboard-paste"
            variant="soft"
            color="neutral"
            size="sm"
            class="rounded-full"
            @click="pasteFromClipboard"
          />
        </div>
        <UButton
          type="submit"
          :label="isMultiUrl ? 'Import all' : 'Save recipe'"
          trailing-icon="i-lucide-arrow-right"
          size="lg"
          class="rounded-full"
          :loading="saving"
          :disabled="!canSubmit"
        />
      </div>
    </div>
  </form>
</template>
