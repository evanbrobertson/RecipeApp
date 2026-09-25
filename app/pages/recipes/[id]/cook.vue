<script setup lang="ts">
import { findTimers, parseIngredient, scaleIngredient } from "#shared/utils/ingredients"

/**
 * Reading mode for the stove: one big step at a time, screen kept awake,
 * swipe or tap to move, one-tap timers for any time mentioned in a step.
 */
definePageMeta({ layout: false })

const { id, recipe, status } = useRecipe()
useHead(() => ({
  title: recipe.value ? `Cooking: ${recipe.value.title}` : "Cooking",
  meta: [{ name: "theme-color", content: "#15110d" }],
}))

const awake = useScreenAwake()
const { start: startTimer } = useKitchenTimers()
const scale = useState(`scale-${id.value}`, () => 1)

const steps = computed(() =>
  (recipe.value?.instructions ?? []).flatMap((s) =>
    s.items.map((text) => ({ text, section: s.name })),
  ),
)
const ingredients = computed(() =>
  (recipe.value?.ingredients ?? []).flatMap((s) =>
    s.items.map((raw) => ({ raw, section: s.name })),
  ),
)

const index = useState(`cook-step-${id.value}`, () => 0)
const finished = ref(false)
const step = computed(() => steps.value[index.value])
const direction = ref<"next" | "prev">("next")

function go(to: number) {
  if (to < 0) return
  if (to >= steps.value.length) {
    finished.value = true
    return
  }
  direction.value = to > index.value ? "next" : "prev"
  finished.value = false
  index.value = to
}

const next = () => go(index.value + 1)
function prev() {
  if (finished.value) finished.value = false
  else go(index.value - 1)
}

onKeyStroke(["ArrowRight", " ", "PageDown"], (e) => {
  e.preventDefault()
  next()
})
onKeyStroke(["ArrowLeft", "PageUp"], (e) => {
  e.preventDefault()
  prev()
})

const stage = ref<HTMLElement | null>(null)
useSwipe(stage, {
  threshold: 60,
  onSwipeEnd: (_, dir) => {
    if (dir === "left") next()
    if (dir === "right") prev()
  },
})

// Ingredients mentioned in this step ("Add the flour and sugar" → flour, sugar)
const stepIngredients = computed(() => {
  const text = step.value?.text.toLowerCase() ?? ""
  if (!text) return []
  return ingredients.value.filter(({ raw }) => {
    const name = parseIngredient(raw)
      .name.toLowerCase()
      .replace(/[^a-z\s-]/g, " ")
    const words = name.split(/\s+/).filter((w) => w.length > 3)
    const key = words[words.length - 1]
    return key ? text.includes(key.replace(/(es|s)$/, "")) : false
  })
})

const timers = computed(() => (step.value ? findTimers(step.value.text) : []))

const showIngredients = ref(false)
const checked = ref(new Set<number>())
function toggle(i: number) {
  const n = new Set(checked.value)
  if (n.has(i)) n.delete(i)
  else n.add(i)
  checked.value = n
}

// Scale text size to step length so long steps still fit
const textSize = computed(() => {
  const len = step.value?.text.length ?? 0
  if (len > 420) return "text-xl sm:text-2xl"
  if (len > 240) return "text-2xl sm:text-3xl"
  return "text-3xl sm:text-4xl"
})
</script>

<template>
  <div class="dark bg-default text-default fixed inset-0 flex flex-col" style="color-scheme: dark">
    <!-- Top bar -->
    <header class="flex items-center gap-2 px-3 pt-[max(env(safe-area-inset-top),12px)] pb-2">
      <UButton
        :to="`/recipes/${id}`"
        icon="i-lucide-x"
        variant="ghost"
        color="neutral"
        size="lg"
        aria-label="Exit cook mode"
      />
      <div class="min-w-0 flex-1 text-center">
        <p class="truncate font-serif text-sm font-semibold">{{ recipe?.title }}</p>
        <p class="text-muted text-xs">
          <template v-if="finished">All done</template>
          <template v-else-if="steps.length">Step {{ index + 1 }} of {{ steps.length }}</template>
        </p>
      </div>
      <UButton
        icon="i-lucide-list"
        variant="ghost"
        color="neutral"
        size="lg"
        aria-label="Ingredients"
        @click="
          () => {
            showIngredients = true
          }
        "
      />
    </header>

    <!-- Progress: tap a segment to jump -->
    <div class="flex gap-1 px-4">
      <button
        v-for="(_, i) in steps"
        :key="i"
        type="button"
        class="h-1.5 flex-1 rounded-full transition"
        :class="
          i < index || finished ? 'bg-primary' : i === index ? 'bg-primary/60' : 'bg-elevated'
        "
        :aria-label="`Go to step ${i + 1}`"
        @click="go(i)"
      />
    </div>

    <ClientOnly>
      <div class="flex justify-center px-4 pt-3">
        <button
          v-if="awake.isSupported.value"
          type="button"
          class="text-muted flex items-center gap-1.5 rounded-full px-3 py-1 text-xs"
          :class="awake.isActive.value ? '' : 'bg-primary/15 text-primary'"
          @click="awake.request"
        >
          <UIcon :name="awake.isActive.value ? 'i-lucide-sun' : 'i-lucide-moon'" class="size-3.5" />
          {{ awake.isActive.value ? "Screen will stay on" : "Tap to keep the screen on" }}
        </button>
      </div>
    </ClientOnly>

    <!-- Step -->
    <main ref="stage" class="relative flex flex-1 touch-pan-y items-center overflow-hidden">
      <div v-if="status === 'pending' && !recipe" class="mx-auto w-full max-w-3xl space-y-4 px-6">
        <USkeleton class="h-10 w-3/4" />
        <USkeleton class="h-10 w-2/3" />
      </div>

      <Transition :name="direction === 'next' ? 'slide-left' : 'slide-right'" mode="out-in">
        <section v-if="finished" key="done" class="mx-auto w-full max-w-2xl px-6 text-center">
          <div class="text-6xl">🍽️</div>
          <h2 class="mt-4 font-serif text-4xl font-semibold">Bon appétit!</h2>
          <p class="text-muted mt-2">That's every step. Enjoy it.</p>
          <div class="mt-8 flex justify-center gap-3">
            <UButton :to="`/recipes/${id}`" label="Back to recipe" size="xl" class="rounded-full" />
            <UButton
              label="Start over"
              variant="soft"
              size="xl"
              class="rounded-full"
              @click="go(0)"
            />
          </div>
        </section>

        <section v-else-if="step" :key="index" class="mx-auto w-full max-w-3xl px-6 py-4">
          <p
            v-if="step.section"
            class="text-primary mb-2 text-sm font-bold tracking-wider uppercase"
          >
            {{ step.section }}
          </p>
          <div class="flex items-start gap-4">
            <span class="text-primary font-serif text-5xl leading-none font-semibold sm:text-6xl">{{
              index + 1
            }}</span>
            <p class="font-serif leading-snug text-pretty" :class="textSize">{{ step.text }}</p>
          </div>

          <div v-if="timers.length" class="mt-6 flex flex-wrap gap-2">
            <UButton
              v-for="t in timers"
              :key="t.label"
              :label="`Start ${t.label} timer`"
              icon="i-lucide-alarm-clock"
              variant="soft"
              size="lg"
              class="rounded-full"
              @click="startTimer(`Step ${index + 1}: ${t.label}`, t.seconds)"
            />
          </div>

          <div v-if="stepIngredients.length" class="mt-6">
            <p class="text-muted mb-2 text-xs font-bold tracking-wider uppercase">You'll need</p>
            <div class="flex flex-wrap gap-2">
              <span
                v-for="ing in stepIngredients"
                :key="ing.raw"
                class="bg-elevated rounded-full px-3 py-1.5 text-sm"
              >
                {{ scaleIngredient(ing.raw, scale) }}
              </span>
            </div>
          </div>
        </section>

        <section v-else-if="recipe" key="empty" class="mx-auto px-6 text-center">
          <p class="text-muted">This recipe has no steps yet.</p>
        </section>
      </Transition>
    </main>

    <div class="pointer-events-none fixed inset-x-0 top-24 z-10 px-4">
      <TimerDock />
    </div>

    <!-- Big thumb-zone controls -->
    <footer
      class="grid grid-cols-[auto_1fr] gap-3 px-4 pt-2 pb-[max(env(safe-area-inset-bottom),16px)]"
    >
      <UButton
        icon="i-lucide-arrow-left"
        size="xl"
        variant="soft"
        color="neutral"
        class="h-16 w-16 justify-center rounded-2xl"
        aria-label="Previous step"
        :disabled="index === 0 && !finished"
        @click="prev"
      />
      <UButton
        v-if="!finished"
        :label="index + 1 >= steps.length ? 'Finish' : 'Next step'"
        :trailing-icon="
          index + 1 >= steps.length ? 'i-lucide-party-popper' : 'i-lucide-arrow-right'
        "
        size="xl"
        class="h-16 justify-center rounded-2xl text-lg"
        :disabled="!steps.length"
        @click="next"
      />
      <UButton
        v-else
        :to="`/recipes/${id}`"
        label="Close"
        size="xl"
        variant="soft"
        class="h-16 justify-center rounded-2xl text-lg"
      />
    </footer>

    <USlideover v-model:open="showIngredients" title="Ingredients" side="right" class="dark">
      <template #body>
        <div class="mb-3 flex justify-end"><ScaleControl v-model="scale" /></div>
        <ul>
          <li v-for="(ing, i) in ingredients" :key="i">
            <p
              v-if="ing.section && ing.section !== ingredients[i - 1]?.section"
              class="text-primary mt-4 mb-1 text-xs font-bold tracking-wider uppercase"
            >
              {{ ing.section }}
            </p>
            <button
              type="button"
              class="flex w-full items-start gap-3 py-2.5 text-left text-lg"
              @click="toggle(i)"
            >
              <span
                class="mt-1 grid size-5 shrink-0 place-items-center rounded-md border-2"
                :class="
                  checked.has(i) ? 'bg-primary border-primary' : 'border-(--ui-border-accented)'
                "
              >
                <UIcon v-if="checked.has(i)" name="i-lucide-check" class="size-3.5 text-white" />
              </span>
              <span :class="{ 'text-dimmed line-through': checked.has(i) }">{{
                scaleIngredient(ing.raw, scale)
              }}</span>
            </button>
          </li>
        </ul>
      </template>
    </USlideover>
  </div>
</template>

<style scoped>
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition:
    transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1),
    opacity 0.2s;
}
.slide-left-enter-from,
.slide-right-leave-to {
  transform: translateX(40px);
  opacity: 0;
}
.slide-left-leave-to,
.slide-right-enter-from {
  transform: translateX(-40px);
  opacity: 0;
}
</style>
