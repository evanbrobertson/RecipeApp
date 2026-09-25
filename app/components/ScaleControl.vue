<script setup lang="ts">
const model = defineModel<number>({ default: 1 })
const options = [0.5, 1, 2, 3]
const label = (n: number) => (n === 0.5 ? "½×" : `${n}×`)
</script>

<template>
  <div class="bg-elevated inline-flex rounded-full p-1" role="radiogroup" aria-label="Scale recipe">
    <button
      v-for="n in options"
      :key="n"
      type="button"
      role="radio"
      :aria-checked="model === n"
      class="rounded-full px-3 py-1 text-sm font-semibold transition"
      :class="model === n ? 'bg-default text-primary shadow-sm' : 'text-muted hover:text-default'"
      @click="model = n"
    >
      {{ label(n) }}
    </button>
  </div>
</template>
