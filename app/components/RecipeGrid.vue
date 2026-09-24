<script setup lang="ts">
defineProps<{ loading?: boolean; count?: number }>()
</script>

<template>
  <div class="grid grid-cols-2 gap-3 sm:gap-4 md:grid-cols-3 lg:grid-cols-4">
    <template v-if="loading">
      <div
        v-for="i in count ?? 8"
        :key="i"
        class="border-default overflow-hidden rounded-xl border"
      >
        <USkeleton class="aspect-[4/3] w-full rounded-none" />
        <div class="space-y-2 p-3">
          <USkeleton class="h-4 w-4/5" />
          <USkeleton class="h-3 w-2/5" />
        </div>
      </div>
    </template>
    <slot v-else />
  </div>
</template>
