<script lang="ts">
  import AlarmClock from "@lucide/svelte/icons/alarm-clock"
  import X from "@lucide/svelte/icons/x"
  import { scale } from "svelte/transition"
  import { formatClock, timers } from "../lib/timers.svelte"
</script>

<div class="pointer-events-none flex flex-wrap justify-center gap-2">
  {#each timers.list as t (t.id)}
    <button
      type="button"
      transition:scale={{ start: 0.75, duration: 150 }}
      class={[
        "rounded-ui pointer-events-auto flex h-9 items-center gap-2 pr-2 pl-3 text-sm font-semibold tabular-nums shadow-md",
        t.done ? "bg-primary animate-pulse text-white" : "border-line bg-paper border",
      ]}
      aria-label={`Timer ${t.label}. Tap to dismiss`}
      onclick={() => timers.remove(t.id)}
    >
      <AlarmClock class="size-4" />
      <span>{t.done ? "Done!" : formatClock(timers.remaining(t))}</span>
      <span class="max-w-28 truncate text-xs font-normal opacity-70">{t.label}</span>
      <X class="size-3.5 opacity-60" />
    </button>
  {/each}
</div>
