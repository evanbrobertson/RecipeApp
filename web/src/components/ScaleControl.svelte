<script lang="ts">
  let { value = $bindable(1), class: className }: { value?: number; class?: string } = $props()
  const options = [0.5, 1, 2, 3]
  const label = (n: number) => (n === 0.5 ? "½×" : `${n}×`)
</script>

<div
  class={["bg-tint rounded-ui inline-flex gap-0.5 p-1", className]}
  role="radiogroup"
  aria-label="Scale recipe"
>
  {#each options as n (n)}
    <button
      type="button"
      role="radio"
      aria-checked={value === n}
      class={[
        "rounded-ctl h-11 min-w-11 px-2.5 text-[15px] font-bold tabular-nums transition-colors",
        value === n ? "bg-tile text-on-tile" : "text-ink-muted hover:text-ink",
      ]}
      onclick={() => (value = n)}
    >
      {label(n)}
    </button>
  {/each}
</div>
