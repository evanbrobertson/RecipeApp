<script setup lang="ts">
defineProps<{
  recipe: {
    id: number
    title: string
    image: string | null
    totalTime: string | null
    recipeYield?: string | null
    recipeCategory: string | null
    recipeCuisine: string | null
  }
  selectable?: boolean
  selected?: boolean
}>()

const emit = defineEmits<{ toggle: [id: number] }>()
const NuxtLink = resolveComponent("NuxtLink")
const imageFailed = ref(false)

// A stable, friendly placeholder colour per recipe when there's no photo
const PLACEHOLDERS = [
  "bg-tomato-100 text-tomato-500",
  "bg-amber-100 text-amber-600",
  "bg-lime-100 text-lime-700",
  "bg-sky-100 text-sky-600",
  "bg-rose-100 text-rose-500",
  "bg-violet-100 text-violet-500",
]
const DARK = [
  "dark:bg-tomato-950/60",
  "dark:bg-amber-950/60",
  "dark:bg-lime-950/60",
  "dark:bg-sky-950/60",
  "dark:bg-rose-950/60",
  "dark:bg-violet-950/60",
]
const ICONS = [
  "i-lucide-cooking-pot",
  "i-lucide-soup",
  "i-lucide-salad",
  "i-lucide-croissant",
  "i-lucide-cake-slice",
  "i-lucide-sandwich",
]
</script>

<template>
  <component
    :is="selectable ? 'button' : NuxtLink"
    :to="selectable ? undefined : `/recipes/${recipe.id}`"
    :type="selectable ? 'button' : undefined"
    class="group focus-visible:ring-primary relative flex flex-col text-left focus-visible:rounded-2xl focus-visible:ring-2 focus-visible:outline-none"
    @click="selectable && emit('toggle', recipe.id)"
  >
    <div
      class="relative aspect-[4/3] w-full overflow-hidden rounded-2xl shadow-sm transition duration-300 group-hover:-translate-y-0.5 group-hover:shadow-md"
      :class="[selected ? 'ring-primary ring-3 ring-offset-2 ring-offset-(--ui-bg)' : '']"
    >
      <img
        v-if="recipe.image && !imageFailed"
        :src="recipe.image"
        :alt="recipe.title"
        loading="lazy"
        decoding="async"
        referrerpolicy="no-referrer"
        class="size-full object-cover transition duration-500 group-hover:scale-[1.04]"
        @error="imageFailed = true"
      />
      <div
        v-else
        class="flex size-full items-center justify-center"
        :class="[PLACEHOLDERS[recipe.id % 6], DARK[recipe.id % 6]]"
      >
        <UIcon :name="ICONS[recipe.id % 6]!" class="size-12 opacity-80" />
      </div>
      <span
        v-if="recipe.totalTime"
        class="bg-default/90 absolute bottom-2 left-2 flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-semibold shadow-sm backdrop-blur"
      >
        <UIcon name="i-lucide-clock" class="size-3.5" />{{ recipe.totalTime }}
      </span>
      <span
        v-if="selectable"
        class="absolute top-2 left-2 grid size-7 place-items-center rounded-full border-2 border-white shadow"
        :class="selected ? 'bg-primary' : 'bg-black/20'"
      >
        <UIcon v-if="selected" name="i-lucide-check" class="size-4 text-white" />
      </span>
    </div>
    <div class="px-1 pt-2.5">
      <h3 class="line-clamp-2 leading-snug font-semibold">{{ recipe.title }}</h3>
      <p
        v-if="recipe.recipeCategory || recipe.recipeCuisine"
        class="text-muted mt-0.5 truncate text-sm"
      >
        {{ [recipe.recipeCategory, recipe.recipeCuisine].filter(Boolean).join(" · ") }}
      </p>
    </div>
  </component>
</template>
