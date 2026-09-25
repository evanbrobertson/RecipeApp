<script setup lang="ts">
import { BOOK_COLORS } from "#shared/utils/recipe"

const model = defineModel<string>({ default: "tomato" })
</script>

<template>
  <div class="flex flex-wrap gap-2" role="radiogroup" aria-label="Cover colour">
    <button
      v-for="c in BOOK_COLORS"
      :key="c"
      type="button"
      role="radio"
      :aria-checked="model === c"
      :aria-label="c"
      class="size-9 rounded-lg shadow-sm transition hover:-translate-y-0.5"
      :class="model === c ? 'ring-3 ring-(--ui-text) ring-offset-2 ring-offset-(--ui-bg)' : ''"
      :style="{ background: bookPalette(c).cloth }"
      @click="model = c"
    />
  </div>
</template>
