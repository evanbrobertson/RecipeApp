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

  interface Props {
    /** Times the step mentions, from `findTimers`. */
    found: { label: string; seconds: number }[]
    /** The length the cook chose for this step, if they changed it. */
    seconds: number | null
    onstart: (seconds: number) => void
  }
  let { found, seconds = $bindable(), onstart }: Props = $props()

  const MIN = 15
  const MAX = 24 * 3600

  // One button per distinct length ("15 minutes, then 1 hour, then 1 hour" is two)
  const choices = $derived(
    found.filter((t, i) => found.findIndex((u) => u.seconds === t.seconds) === i),
  )
  const length = $derived(seconds ?? choices[0]?.seconds ?? null)

  function words(s: number) {
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = s % 60
    return [h && `${h} hr`, m && `${m} min`, sec && `${sec} sec`].filter(Boolean).join(" ")
  }

  // Finer steps for short times: 15 s under a minute, a minute to half an hour, 5 to two hours
  function nudge(dir: 1 | -1) {
    if (length === null) return
    const from = dir > 0 ? length : length - 1
    const step = from < 60 ? 15 : from < 1800 ? 60 : from < 7200 ? 300 : 900
    const next =
      dir > 0 ? Math.floor(length / step) * step + step : Math.ceil(length / step) * step - step
    seconds = Math.min(MAX, Math.max(MIN, next))
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
    const minutes = Number(input?.value)
    if (minutes > 0) seconds = Math.min(MAX, Math.max(MIN, Math.round(minutes * 60)))
    typing = false
  }
</script>

{#if length === null}
  <button type="button" class="btn btn-soft btn-lg" onclick={() => (seconds = 300)}>
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
        disabled={length <= MIN}
        onclick={() => nudge(-1)}
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
          aria-label={`${words(length)}. Tap to type a length`}
          onclick={type}
        >
          {words(length)}
        </button>
      {/if}
      <button
        type="button"
        class="btn btn-ghost btn-icon"
        aria-label="Longer"
        disabled={length >= MAX}
        onclick={() => nudge(1)}
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
