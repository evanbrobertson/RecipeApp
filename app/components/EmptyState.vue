<script setup lang="ts">
defineProps<{ icon: string; title: string; description?: string }>()
</script>

<template>
  <div class="bg-elevated/40 flex flex-col items-center rounded-3xl px-6 py-14 text-center">
    <span class="bg-primary/10 text-primary grid size-16 place-items-center rounded-2xl">
      <UIcon :name="icon" class="size-8" />
    </span>
    <p class="mt-4 font-serif text-xl font-semibold">{{ title }}</p>
    <p v-if="description" class="text-muted mt-1 max-w-sm text-sm">{{ description }}</p>
    <div class="mt-5 flex flex-wrap justify-center gap-2"><slot /></div>
  </div>
</template>
