<script lang="ts">
  /**
   * A cook mode step's timer: suggested from the times the step mentions, never trusted. The
   * cook nudges or types the length before starting it, and any step can have one.
   */
  import AlarmClock from "@lucide/svelte/icons/alarm-clock"
  import AlarmClockPlus from "@lucide/svelte/icons/alarm-clock-plus"
  import Minus from "@lucide/svelte/icons/minus"
  import Plus from "@lucide/svelte/icons/plus"
  import { tick } from "svelte"
  import {
    TIMER_DEFAULT,
    TIMER_MAX,
    TIMER_MIN,
    nudgeTimer,
    timerFromMinutes,
    timerWords,
  } from "../lib/ingredients"

  interface Props {
    /** The step's timer choices, from `suggestTimers`; the first is suggested. */
    choices: { label: string; seconds: number }[]
    /** The length the cook chose for this step, if they changed it. */
    seconds: number | null
    onstart: (seconds: number) => void
  }
  let { choices, seconds = $bindable(), onstart }: Props = $props()

  const length = $derived(seconds ?? choices[0]?.seconds ?? null)

  function nudge(longer: boolean) {
    if (length !== null) seconds = nudgeTimer(length, longer)
  }

  // Tapping the length types it, in minutes
  let typing = $state(false)
  let input: HTMLInputElement | undefined = $state()
  async function type() {
    typing = true
    await tick()
    input?.select()
  }
  function commit() {
    seconds = timerFromMinutes(Number(input?.value)) ?? seconds
    typing = false
  }
</script>

{#if length === null}
  <button type="button" class="btn btn-soft btn-lg" onclick={() => (seconds = TIMER_DEFAULT)}>
    <AlarmClockPlus /> Set a timer
  </button>
{:else}
  <div class="flex flex-wrap items-center gap-2.5">
    <div
      class="bg-tint rounded-ui inline-flex items-center gap-0.5 p-1"
      role="group"
      aria-label="Timer length"
    >
      <button
        type="button"
        class="btn btn-ghost btn-icon"
        aria-label="Shorter"
        disabled={length <= TIMER_MIN}
        onclick={() => nudge(false)}
      >
        <Minus />
      </button>
      {#if typing}
        <label class="flex h-11 items-center gap-1.5 px-2 text-lg font-bold">
          <input
            bind:this={input}
            type="number"
            inputmode="decimal"
            min="0.25"
            max="1440"
            step="any"
            value={Math.round((length / 60) * 100) / 100}
            class="rounded-ctl bg-paper border-line h-10 w-20 border px-2 text-center tabular-nums"
            onblur={commit}
            onkeydown={(e) => {
              if (e.key === "Enter") commit()
              if (e.key === "Escape") typing = false
            }}
          />
          min
        </label>
      {:else}
        <button
          type="button"
          class="rounded-ctl hover:bg-paper/70 h-11 min-w-20 px-2 text-lg font-bold tabular-nums transition-colors"
          aria-label={`${timerWords(length)}. Tap to type a length`}
          onclick={type}
        >
          {timerWords(length)}
        </button>
      {/if}
      <button
        type="button"
        class="btn btn-ghost btn-icon"
        aria-label="Longer"
        disabled={length >= TIMER_MAX}
        onclick={() => nudge(true)}
      >
        <Plus />
      </button>
    </div>
    <button type="button" class="btn btn-tile btn-lg" onclick={() => onstart(length)}>
      <AlarmClock /> Start timer
    </button>
  </div>
  {#if choices.length > 1}
    <div class="mt-3 flex flex-wrap items-center gap-2">
      <span class="meta">The step mentions</span>
      {#each choices as t (t.seconds)}
        <button
          type="button"
          class={["chip", length === t.seconds ? "bg-tile text-on-tile" : "bg-tint text-primary"]}
          aria-pressed={length === t.seconds}
          onclick={() => (seconds = t.seconds)}
        >
          {t.label}
        </button>
      {/each}
    </div>
  {/if}
{/if}
