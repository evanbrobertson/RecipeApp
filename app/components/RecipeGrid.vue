<script setup lang="ts">
defineProps<{ loading?: boolean; count?: number }>()
</script>

<template>
  <div class="grid grid-cols-2 gap-x-3 gap-y-6 sm:gap-x-5 md:grid-cols-3 lg:grid-cols-4">
    <template v-if="loading">
      <div v-for="i in count ?? 8" :key="i">
        <USkeleton class="aspect-[4/3] w-full rounded-2xl" />
        <USkeleton class="mt-3 h-4 w-4/5" />
        <USkeleton class="mt-2 h-3 w-2/5" />
      </div>
    </template>
    <slot v-else />
  </div>
</template>
