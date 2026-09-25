<script setup lang="ts">
const props = defineProps<{ books: ShelfBook[]; pulledId?: number | null; addable?: boolean }>()
defineEmits<{ open: [book: ShelfBook]; add: [] }>()

// Pack books into shelf rows that fit the available width
const el = ref<HTMLElement | null>(null)
const { width } = useElementSize(el)

const GAP = 4
const ADD_SLOT = 56
const rows = computed(() => {
  const available = Math.max(260, (width.value || 900) - 48)
  const out: ShelfBook[][] = [[]]
  let used = 0
  for (const book of props.books) {
    const w = bookSize(book).width + GAP
    if (used + w > available && out[out.length - 1]!.length) {
      out.push([])
      used = 0
    }
    out[out.length - 1]!.push(book)
    used += w
  }
  // Keep room for the "new cookbook" slot on the last row
  if (props.addable && used + ADD_SLOT > available) out.push([])
  return out
})
</script>

<template>
  <div ref="el" class="space-y-10">
    <div v-for="(row, ri) in rows" :key="ri" class="shelf-row">
      <div class="books">
        <CookbookSpine
          v-for="book in row"
          :key="book.id"
          :book="book"
          :pulled="pulledId === book.id"
          @open="$emit('open', book)"
        />
        <button
          v-if="addable && ri === rows.length - 1"
          type="button"
          class="new-book"
          aria-label="New cookbook"
          @click="$emit('add')"
        >
          <UIcon name="i-lucide-plus" class="size-6" />
        </button>
        <!-- A little herb pot on the first shelf, because why not -->
        <svg v-if="ri === 0" class="plant" viewBox="0 0 60 90" aria-hidden="true">
          <path d="M30 50 C 18 36, 8 34, 4 22 C 16 22, 26 30, 30 44" fill="#6f9a57" />
          <path d="M30 50 C 42 34, 52 30, 57 16 C 44 18, 34 28, 30 44" fill="#5f8a4e" />
          <path d="M30 52 C 26 34, 28 18, 34 6 C 38 20, 36 36, 31 50" fill="#7fae63" />
          <path d="M14 52 h32 l-4 34 h-24 z" fill="#c8643f" />
          <rect x="11" y="50" width="38" height="8" rx="2" fill="#b3552f" />
        </svg>
      </div>
      <div class="plank" />
    </div>
  </div>
</template>

<style scoped>
.shelf-row {
  position: relative;
  perspective: 1400px;
  perspective-origin: 50% -30%;
}
.books {
  display: flex;
  align-items: flex-end;
  gap: 4px;
  min-height: 240px;
  padding: 0 24px;
  transform-style: preserve-3d;
}
.plank {
  position: relative;
  height: 18px;
  margin-top: -2px;
  border-radius: 4px;
  background: linear-gradient(var(--shelf-wood), var(--shelf-wood-dark));
  box-shadow:
    0 14px 18px -10px rgb(60 35 15 / 0.45),
    inset 0 2px 0 rgb(255 255 255 / 0.15);
}
.plank::before {
  /* the shelf's top surface, seen slightly from above */
  content: "";
  position: absolute;
  left: 6px;
  right: 6px;
  top: -10px;
  height: 12px;
  background: linear-gradient(transparent, rgb(0 0 0 / 0.08));
  clip-path: polygon(2% 0, 98% 0, 100% 100%, 0 100%);
}
.new-book {
  display: grid;
  place-items: center;
  width: 48px;
  height: 190px;
  flex: none;
  margin-left: 6px;
  border: 2px dashed var(--ui-border-accented);
  border-radius: 6px;
  color: var(--ui-text-muted);
  transition: all 0.2s;
}
.new-book:hover {
  color: var(--ui-primary);
  border-color: var(--ui-primary);
  transform: translateY(-4px);
}
.plant {
  width: 54px;
  height: 82px;
  margin-left: auto;
  flex: none;
}
</style>
