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
        "pointer-events-auto relative flex h-11 items-center gap-2 rounded-full pr-3 pl-3.5 text-[15px] font-bold tabular-nums",
        t.done ? "bg-tile text-on-tile isolate shadow-(--menu-shadow)" : "menu-surface text-ink rounded-full",
      ]}
      aria-label={`Timer ${t.label}. Tap to dismiss`}
      onclick={() => timers.remove(t.id)}
    >
      {#if t.done}
        <!-- A ring that keeps calling; the pill itself stays solid so its text stays readable -->
        <span class="bg-tile absolute inset-0 -z-10 animate-ping rounded-full opacity-40"></span>
      {/if}
      <AlarmClock class={["size-[18px]", !t.done && "text-primary"]} />
      <span>{t.done ? "Done!" : formatClock(timers.remaining(t))}</span>
      <span class={["max-w-32 truncate text-[13px] font-semibold", !t.done && "text-ink-muted"]}>
        {t.label}
      </span>
      <X class={["size-4", !t.done && "text-ink-muted"]} />
    </button>
  {/each}
</div>
