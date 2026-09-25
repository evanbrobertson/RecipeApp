<script setup lang="ts">
defineProps<{
  cookbook: { id: number; name: string; description: string | null; recipeCount: number }
}>()
</script>

<template>
  <NuxtLink
    :to="`/cookbooks/${cookbook.id}`"
    class="border-default hover:bg-elevated/50 flex items-start gap-3 rounded-xl border p-4 transition"
  >
    <div class="bg-primary/10 text-primary grid size-10 shrink-0 place-items-center rounded-lg">
      <UIcon name="i-lucide-book-marked" class="size-5" />
    </div>
    <div class="min-w-0 flex-1">
      <h3 class="truncate font-semibold">{{ cookbook.name }}</h3>
      <p class="text-muted text-sm">
        {{ cookbook.recipeCount }} {{ cookbook.recipeCount === 1 ? "recipe" : "recipes" }}
      </p>
      <p v-if="cookbook.description" class="text-muted mt-1 line-clamp-2 text-sm">
        {{ cookbook.description }}
      </p>
    </div>
  </NuxtLink>
</template>
