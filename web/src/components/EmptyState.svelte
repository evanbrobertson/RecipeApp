<script lang="ts">
  import type { Component, Snippet } from "svelte"

  interface Props {
    icon: Component<{ class?: string }>
    title: string
    description?: string
    children?: Snippet
  }
  let { icon: Icon, title, description, children }: Props = $props()
</script>

<div class="card flex flex-col items-center px-6 py-14 text-center">
  <span class="bg-primary/10 text-primary rounded-ui grid size-16 place-items-center">
    <Icon class="size-8" />
  </span>
  <p class="mt-4 font-serif text-xl font-semibold">{title}</p>
  {#if description}<p class="text-ink-muted mt-1 max-w-sm text-sm">{description}</p>{/if}
  {#if children}<div class="mt-5 flex flex-wrap justify-center gap-2">{@render children()}</div>{/if}
</div>
