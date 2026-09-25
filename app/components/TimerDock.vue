<script setup lang="ts">
const { timers, remove, remaining } = useKitchenTimers()
</script>

<template>
  <TransitionGroup
    tag="div"
    class="pointer-events-none flex flex-wrap justify-center gap-2"
    enter-from-class="scale-75 opacity-0"
    leave-to-class="scale-75 opacity-0"
    enter-active-class="transition"
    leave-active-class="transition"
  >
    <button
      v-for="t in timers"
      :key="t.id"
      type="button"
      class="pointer-events-auto flex items-center gap-2 rounded-full py-1.5 pr-2 pl-3 text-sm font-semibold tabular-nums shadow-md"
      :class="t.done ? 'bg-primary animate-pulse text-white' : 'bg-default border-default border'"
      :aria-label="`Timer ${t.label}. Tap to dismiss`"
      @click="remove(t.id)"
    >
      <UIcon name="i-lucide-alarm-clock" class="size-4" />
      <span>{{ t.done ? "Done!" : formatClock(remaining(t)) }}</span>
      <span class="max-w-28 truncate text-xs font-normal opacity-70">{{ t.label }}</span>
      <UIcon name="i-lucide-x" class="size-3.5 opacity-60" />
    </button>
  </TransitionGroup>
</template>
