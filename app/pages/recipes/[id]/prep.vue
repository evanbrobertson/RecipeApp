<script setup lang="ts">
import {
  formatQuantity,
  miseEnPlace,
  pluralUnit,
  type MiseItem,
  type Vessel,
} from "#shared/utils/ingredients"

/**
 * Mise en place: everything prepped, measured and in its place before the heat goes on.
 * Each ingredient gets a vessel sized to its quantity; tap it once it's ready.
 */
const { id, recipe, status } = useRecipe()
useHead(() => ({ title: recipe.value ? `Mise en place: ${recipe.value.title}` : "Mise en place" }))

const awake = useScreenAwake()
const scale = useState(`scale-${id.value}`, () => 1)

const items = computed(() =>
  miseEnPlace((recipe.value?.ingredients ?? []).flatMap((s) => s.items)).map((item, i) => ({
    ...item,
    key: i,
  })),
)

const SIZE_ORDER: Vessel[] = ["large", "medium", "small", "ramekin", "pinch"]
const groups = computed(() => [
  {
    title: "Chop & prep",
    hint: "Wash, peel and cut these first",
    icon: "i-lucide-slice",
    items: items.value.filter((i) => i.vessel === "board"),
  },
  {
    title: "Measure into bowls",
    hint: "Biggest bowls first, then the little ones",
    icon: "i-lucide-soup",
    items: items.value
      .filter((i) => SIZE_ORDER.includes(i.vessel))
      .toSorted((a, b) => SIZE_ORDER.indexOf(a.vessel) - SIZE_ORDER.indexOf(b.vessel)),
  },
  {
    title: "Keep within reach",
    hint: "Seasoning and extras to have out on the counter",
    icon: "i-lucide-hand",
    items: items.value.filter((i) => i.vessel === "jar"),
  },
])

const VESSEL_LABEL: Record<Vessel, string> = {
  large: "Large bowl",
  medium: "Medium bowl",
  small: "Small bowl",
  ramekin: "Ramekin",
  pinch: "Pinch bowl",
  board: "Board",
  jar: "On hand",
}

function amount(item: MiseItem) {
  if (item.quantity === null) return item.unit ? `a ${item.unit}` : ""
  const q = formatQuantity(item.quantity * scale.value)
  const max = item.quantityMax !== null ? `–${formatQuantity(item.quantityMax * scale.value)}` : ""
  const n = (item.quantityMax ?? item.quantity) * scale.value
  return `${q}${max}${item.unit ? ` ${pluralUnit(item.unit, n)}` : ""}`
}

const ready = useState(`prep-${id.value}`, () => [] as number[])
function toggle(key: number) {
  ready.value = ready.value.includes(key)
    ? ready.value.filter((k) => k !== key)
    : [...ready.value, key]
  if (ready.value.length === items.value.length) navigator.vibrate?.(60)
}
const progress = computed(() => (items.value.length ? ready.value.length / items.value.length : 0))
const allReady = computed(() => items.value.length > 0 && ready.value.length === items.value.length)

const vesselCounts = computed(() => {
  const counts = new Map<Vessel, number>()
  for (const i of items.value) counts.set(i.vessel, (counts.get(i.vessel) ?? 0) + 1)
  // One cutting board does for all the chopping
  if (counts.has("board")) counts.set("board", 1)
  return [...counts.entries()].filter(([v]) => v !== "jar")
})
</script>

<template>
  <div class="mx-auto max-w-5xl">
    <div class="mb-6 flex items-start justify-between gap-3">
      <div class="min-w-0">
        <NuxtLink
          :to="`/recipes/${id}`"
          class="text-muted hover:text-primary inline-flex items-center gap-1 text-sm"
        >
          <UIcon name="i-lucide-arrow-left" class="size-4" />
          <span class="truncate">{{ recipe?.title ?? "Recipe" }}</span>
        </NuxtLink>
        <h1 class="mt-1 font-serif text-3xl font-semibold sm:text-4xl">Mise en place</h1>
        <p class="text-muted mt-1">
          <em>“Everything in its place.”</em> Prep and measure it all before you start cooking. Tap
          each one when it's ready.
        </p>
      </div>
      <ScaleControl v-model="scale" class="hidden shrink-0 sm:inline-flex" />
    </div>

    <!-- What to get out -->
    <div
      v-if="vesselCounts.length"
      class="bg-elevated/60 mb-6 flex flex-wrap items-center gap-x-5 gap-y-2 rounded-2xl px-4 py-3 text-sm"
    >
      <span class="font-semibold">Get out:</span>
      <span v-for="[v, n] in vesselCounts" :key="v" class="text-muted">
        <strong class="text-default">{{ n }}</strong> × {{ VESSEL_LABEL[v].toLowerCase()
        }}{{ n > 1 ? "s" : "" }}
      </span>
      <ScaleControl v-model="scale" class="ml-auto sm:hidden" />
    </div>

    <!-- Progress -->
    <div
      class="bg-default/90 border-default sticky top-14 z-20 -mx-4 mb-6 border-b px-4 py-3 backdrop-blur"
    >
      <div class="flex items-center gap-3">
        <div class="bg-elevated h-3 flex-1 overflow-hidden rounded-full">
          <div
            class="bg-primary h-full rounded-full transition-all duration-500"
            :style="{ width: `${progress * 100}%` }"
          />
        </div>
        <span class="text-sm font-semibold tabular-nums"
          >{{ ready.length }}/{{ items.length }} ready</span
        >
      </div>
      <ClientOnly>
        <button
          v-if="awake.isSupported.value && !awake.isActive.value"
          type="button"
          class="text-primary mt-1 text-xs"
          @click="awake.request"
        >
          Tap to keep the screen on while you prep
        </button>
      </ClientOnly>
    </div>

    <USkeleton v-if="status === 'pending' && !recipe" class="h-64 w-full rounded-3xl" />

    <EmptyState v-else-if="!items.length" icon="i-lucide-soup" title="No ingredients to prep">
      <UButton :to="`/recipes/${id}`" label="Back to recipe" variant="soft" />
    </EmptyState>

    <div v-else class="counter space-y-10 rounded-3xl p-4 sm:p-8">
      <section v-for="group in groups.filter((g) => g.items.length)" :key="group.title">
        <div class="mb-4 flex items-center gap-2">
          <UIcon :name="group.icon" class="text-primary size-5" />
          <h2 class="font-serif text-xl font-semibold">{{ group.title }}</h2>
          <span class="text-muted hidden text-sm sm:inline">· {{ group.hint }}</span>
        </div>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          <button
            v-for="item in group.items"
            :key="item.key"
            type="button"
            class="group relative flex flex-col items-center rounded-2xl px-2 pt-4 pb-3 text-center transition active:scale-[0.97]"
            :class="
              ready.includes(item.key)
                ? 'bg-primary/10'
                : 'bg-default/70 hover:bg-default shadow-sm'
            "
            :aria-pressed="ready.includes(item.key)"
            @click="toggle(item.key)"
          >
            <span
              class="absolute top-2 right-2 grid size-6 place-items-center rounded-full transition"
              :class="
                ready.includes(item.key)
                  ? 'bg-primary scale-100 text-white'
                  : 'scale-75 border-2 border-(--ui-border-accented)'
              "
            >
              <UIcon v-if="ready.includes(item.key)" name="i-lucide-check" class="size-4" />
            </span>
            <div class="flex h-28 items-end justify-center">
              <PrepBowl
                :vessel="item.vessel"
                :filled="ready.includes(item.key)"
                :color="ingredientColor(item.name)"
                class="transition group-hover:-translate-y-0.5"
              />
            </div>
            <p class="mt-2 text-lg leading-tight font-semibold tabular-nums">
              {{ amount(item) || " " }}
            </p>
            <p
              class="line-clamp-2 leading-snug"
              :class="{ 'text-muted line-through': ready.includes(item.key) }"
            >
              {{ item.name }}
            </p>
            <span
              v-if="item.task"
              class="mt-1.5 rounded-full bg-amber-100 px-2 py-0.5 text-xs font-semibold text-amber-800 capitalize dark:bg-amber-900/50 dark:text-amber-200"
            >
              {{ item.task }}
            </span>
            <span v-else-if="item.prep" class="text-muted mt-1 line-clamp-1 text-xs">{{
              item.prep
            }}</span>
            <span class="text-dimmed mt-1 text-[11px] tracking-wide uppercase">{{
              VESSEL_LABEL[item.vessel]
            }}</span>
          </button>
        </div>
      </section>
    </div>

    <Transition
      enter-from-class="translate-y-6 opacity-0"
      enter-active-class="transition duration-300"
      leave-to-class="translate-y-6 opacity-0"
      leave-active-class="transition duration-200"
    >
      <div
        v-if="allReady"
        class="fixed inset-x-0 bottom-24 z-30 flex justify-center px-4 md:bottom-8"
      >
        <UButton
          :to="`/recipes/${id}/cook`"
          label="Everything's in its place. Start cooking"
          icon="i-lucide-flame"
          size="xl"
          class="shadow-primary/30 rounded-full shadow-xl"
        />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
/* A warm speckled countertop */
.counter {
  background:
    radial-gradient(circle at 20% 30%, rgb(0 0 0 / 0.025) 0 1px, transparent 1.5px) 0 0 / 14px 14px,
    radial-gradient(circle at 70% 60%, rgb(0 0 0 / 0.03) 0 1px, transparent 1.5px) 0 0 / 19px 19px,
    var(--ui-bg-muted);
  border: 1px solid var(--ui-border-muted);
}
</style>
