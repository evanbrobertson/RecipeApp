<script setup lang="ts">
defineProps<{ icon: string; title: string; description?: string }>()
</script>

<template>
  <div
    class="border-default flex flex-col items-center rounded-2xl border border-dashed px-6 py-14 text-center"
  >
    <UIcon :name="icon" class="text-dimmed size-10" />
    <p class="mt-3 font-medium">{{ title }}</p>
    <p v-if="description" class="text-muted mt-1 max-w-sm text-sm">{{ description }}</p>
    <div class="mt-5 flex flex-wrap justify-center gap-2"><slot /></div>
  </div>
</template>
