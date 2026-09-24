<script setup lang="ts">
defineProps<{
  recipe: {
    id: number
    title: string
    image: string | null
    totalTime: string | null
    recipeCategory: string | null
    recipeCuisine: string | null
  }
  selectable?: boolean
  selected?: boolean
}>()

const emit = defineEmits<{ toggle: [id: number] }>()
const NuxtLink = resolveComponent("NuxtLink")
const imageFailed = ref(false)
</script>

<template>
  <component
    :is="selectable ? 'button' : NuxtLink"
    :to="selectable ? undefined : `/recipes/${recipe.id}`"
    :type="selectable ? 'button' : undefined"
    class="group border-default bg-default focus-visible:ring-primary relative flex flex-col overflow-hidden rounded-xl border text-left transition focus-visible:ring-2 focus-visible:outline-none"
    :class="selected ? 'ring-primary ring-2' : 'hover:shadow-md'"
    @click="selectable && emit('toggle', recipe.id)"
  >
    <div class="bg-elevated relative aspect-[4/3] w-full overflow-hidden">
      <img
        v-if="recipe.image && !imageFailed"
        :src="recipe.image"
        :alt="recipe.title"
        loading="lazy"
        decoding="async"
        referrerpolicy="no-referrer"
        class="size-full object-cover transition duration-300 group-hover:scale-[1.03]"
        @error="imageFailed = true"
      />
      <div v-else class="flex size-full items-center justify-center">
        <UIcon name="i-lucide-cooking-pot" class="text-dimmed size-10" />
      </div>
      <UCheckbox
        v-if="selectable"
        :model-value="selected"
        class="bg-default/90 pointer-events-none absolute top-2 left-2 rounded p-1"
        tabindex="-1"
      />
    </div>
    <div class="flex flex-1 flex-col gap-2 p-3">
      <h3 class="line-clamp-2 leading-snug font-semibold">{{ recipe.title }}</h3>
      <div class="text-muted mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
        <span v-if="recipe.totalTime" class="flex items-center gap-1">
          <UIcon name="i-lucide-clock" class="size-3.5" />{{ recipe.totalTime }}
        </span>
        <span v-if="recipe.recipeCategory" class="truncate">{{ recipe.recipeCategory }}</span>
        <span v-if="recipe.recipeCuisine" class="truncate">{{ recipe.recipeCuisine }}</span>
      </div>
    </div>
  </component>
</template>
