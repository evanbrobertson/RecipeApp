<script lang="ts">
  let { value = $bindable(1), class: className }: { value?: number; class?: string } = $props()
  const options = [0.5, 1, 2, 3]
  const label = (n: number) => (n === 0.5 ? "½×" : `${n}×`)
</script>

<div
  class={["bg-raised rounded-ui inline-flex p-1", className]}
  role="radiogroup"
  aria-label="Scale recipe"
>
  {#each options as n (n)}
    <button
      type="button"
      role="radio"
      aria-checked={value === n}
      class={[
        "rounded-ui px-3 py-1 text-sm font-semibold transition",
        value === n ? "bg-canvas text-primary shadow-sm" : "text-ink-muted hover:text-ink",
      ]}
      onclick={() => (value = n)}
    >
      {label(n)}
    </button>
  {/each}
</div>
