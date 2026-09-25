<script setup lang="ts">
/**
 * A cookbook pulled off the shelf: it flies to the centre, the cover swings open
 * and the right-hand page is a table of contents.
 */
const props = defineProps<{ book: ShelfBook | null }>()
const emit = defineEmits<{ close: [] }>()

const open = ref(false)
const visible = ref(false)
const palette = computed(() => bookPalette(props.book?.color))

interface BookDetails {
  recipes: { id: number; title: string }[]
}
const details = ref<BookDetails | null>(null)
const loading = ref(false)

watch(
  () => props.book,
  async (book) => {
    if (!book) return
    visible.value = true
    open.value = false
    details.value = null
    loading.value = true
    // Let the closed book land before the cover swings open
    setTimeout(() => (open.value = true), 380)
    try {
      details.value = await $fetch<BookDetails>(`/api/cookbooks/${book.id}`)
    } catch {
      details.value = { recipes: [] }
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

function close() {
  open.value = false
  setTimeout(() => {
    visible.value = false
    emit("close")
  }, 520)
}

onKeyStroke("Escape", () => props.book && close())

const vars = computed(() => ({
  "--cloth": palette.value.cloth,
  "--shade": palette.value.shade,
  "--foil": palette.value.foil,
}))
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible && book" class="backdrop" @click.self="close">
        <div class="stage" :class="{ open }" :style="vars">
          <!-- Right page: contents -->
          <section class="page right">
            <header class="mb-4 flex items-baseline justify-between gap-2">
              <h2 class="font-serif text-2xl font-semibold">Contents</h2>
              <span class="text-muted text-xs">{{ book.recipeCount }} recipes</span>
            </header>
            <div v-if="loading" class="space-y-3">
              <USkeleton v-for="i in 5" :key="i" class="h-4 w-full" />
            </div>
            <p v-else-if="!details?.recipes.length" class="text-muted text-sm italic">
              Blank pages, for now. Add recipes from any recipe page or with “Select” on the recipes
              list.
            </p>
            <ol v-else class="toc">
              <li v-for="(r, i) in details.recipes" :key="r.id">
                <NuxtLink
                  :to="`/recipes/${r.id}`"
                  class="toc-link"
                  @click="
                    () => {
                      visible = false
                    }
                  "
                >
                  <span class="toc-title">{{ r.title }}</span>
                  <span class="leader" />
                  <span class="toc-num">{{ i + 1 }}</span>
                </NuxtLink>
              </li>
            </ol>
            <footer class="mt-auto flex justify-end pt-4">
              <UButton
                :to="`/cookbooks/${book.id}`"
                label="Open cookbook"
                trailing-icon="i-lucide-arrow-right"
                variant="soft"
                size="sm"
                @click="
                  () => {
                    visible = false
                  }
                "
              />
            </footer>
          </section>

          <!-- The cover: front outside, endpaper inside -->
          <div class="cover">
            <div class="cover-front">
              <div class="frame">
                <UIcon name="i-lucide-chef-hat" class="size-8" />
                <h2 class="font-serif text-2xl leading-tight font-semibold sm:text-3xl">
                  {{ book.name }}
                </h2>
                <span class="rule" />
                <span class="text-xs tracking-[0.2em] uppercase">Recipes</span>
              </div>
            </div>
            <div class="cover-inside">
              <p class="font-serif text-xs tracking-[0.3em] uppercase opacity-60">Ex libris</p>
              <h3 class="mt-3 font-serif text-2xl font-semibold">{{ book.name }}</h3>
              <p v-if="book.description" class="mt-3 text-sm opacity-80">{{ book.description }}</p>
              <p class="mt-auto text-xs opacity-60">A cookbook of {{ book.recipeCount }} recipes</p>
            </div>
          </div>

          <button type="button" class="close" aria-label="Close book" @click="close">
            <UIcon name="i-lucide-x" class="size-5" />
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: center;
  padding: 16px;
  background: rgb(20 12 6 / 0.55);
  backdrop-filter: blur(6px);
  perspective: 2200px;
}
.stage {
  --page-w: min(380px, 44vw);
  position: relative;
  width: calc(var(--page-w) * 2);
  height: min(560px, 80dvh);
  transform-style: preserve-3d;
  /* Closed: centre the cover (the right half) */
  transform: translateX(calc(var(--page-w) / -2)) rotateX(8deg) scale(0.92);
  transition: transform 0.6s cubic-bezier(0.2, 0.8, 0.2, 1);
  animation: land 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.stage.open {
  transform: translateX(0) rotateX(0) scale(1);
}
@keyframes land {
  from {
    transform: translateX(calc(var(--page-w) / -2)) translateY(40vh) rotateX(40deg) scale(0.4);
    opacity: 0;
  }
}
.page {
  position: absolute;
  top: 0;
  width: var(--page-w);
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 28px 26px 20px;
  background: linear-gradient(90deg, rgb(0 0 0 / 0.08), transparent 8%), var(--paper);
  color: var(--ui-text);
  border-radius: 2px 10px 10px 2px;
  box-shadow: 0 30px 60px -20px rgb(0 0 0 / 0.5);
  overflow: hidden;
}
.page.right {
  left: var(--page-w);
}
.toc {
  overflow-y: auto;
  margin-right: -8px;
  padding-right: 8px;
}
.toc-link {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 7px 0;
  font-size: 0.95rem;
}
.toc-link:hover .toc-title {
  color: var(--ui-primary);
}
.toc-title {
  font-family: var(--font-serif);
  transition: color 0.15s;
}
.leader {
  flex: 1;
  border-bottom: 2px dotted var(--ui-border-accented);
  transform: translateY(-4px);
  min-width: 16px;
}
.toc-num {
  font-variant-numeric: tabular-nums;
  color: var(--ui-text-muted);
  font-size: 0.85rem;
}

.cover {
  position: absolute;
  top: -6px;
  left: var(--page-w);
  width: calc(var(--page-w) + 6px);
  height: calc(100% + 12px);
  transform-origin: left center;
  transform-style: preserve-3d;
  transition: transform 0.9s cubic-bezier(0.3, 0.7, 0.2, 1);
}
.stage.open .cover {
  transform: rotateY(-180deg);
}
.cover-front,
.cover-inside {
  position: absolute;
  inset: 0;
  backface-visibility: hidden;
  border-radius: 3px 12px 12px 3px;
}
.cover-front {
  display: grid;
  place-items: center;
  padding: 28px;
  color: var(--foil);
  background:
    linear-gradient(90deg, rgb(0 0 0 / 0.35), rgb(255 255 255 / 0.08) 4%, transparent 9%),
    repeating-linear-gradient(45deg, transparent 0 3px, rgb(0 0 0 / 0.03) 3px 4px), var(--cloth);
  box-shadow: 0 30px 60px -20px rgb(0 0 0 / 0.6);
}
.frame {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  width: 100%;
  height: 100%;
  justify-content: center;
  text-align: center;
  border: 2px solid var(--foil);
  outline: 1px solid var(--foil);
  outline-offset: 5px;
  border-radius: 6px;
  padding: 20px;
}
.rule {
  width: 40%;
  height: 1px;
  background: var(--foil);
}
.cover-inside {
  transform: rotateY(180deg);
  display: flex;
  flex-direction: column;
  padding: 34px 30px;
  color: var(--shade);
  background:
    radial-gradient(circle at 20% 20%, rgb(255 255 255 / 0.5), transparent 40%),
    repeating-linear-gradient(
      135deg,
      color-mix(in srgb, var(--cloth) 16%, #fff8ec) 0 10px,
      color-mix(in srgb, var(--cloth) 10%, #fff8ec) 10px 20px
    );
  border-radius: 12px 3px 3px 12px;
}
.close {
  position: absolute;
  top: -44px;
  right: 0;
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 999px;
  color: white;
  background: rgb(255 255 255 / 0.15);
}

/* Phones: a single page; the cover swings away to reveal the contents */
@media (max-width: 640px) {
  .stage {
    --page-w: min(86vw, 380px);
    width: var(--page-w);
    transform: rotateX(8deg) scale(0.92);
  }
  .stage.open {
    transform: none;
  }
  @keyframes land {
    from {
      transform: translateY(40vh) rotateX(40deg) scale(0.4);
      opacity: 0;
    }
  }
  .page.right,
  .cover {
    left: 0;
  }
  .stage.open .cover {
    transform: rotateY(-180deg) translateX(-8px);
    opacity: 0;
    transition:
      transform 0.9s cubic-bezier(0.3, 0.7, 0.2, 1),
      opacity 0.3s 0.6s;
  }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
