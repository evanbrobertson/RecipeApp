<script lang="ts" module>
  import type { Component } from "svelte"

  export interface MenuItem {
    label: string
    icon: Component<{ class?: string }>
    onselect: () => void
    danger?: boolean
  }
</script>

<script lang="ts">
  /** A small dropdown menu anchored to its trigger button. */
  import Ellipsis from "@lucide/svelte/icons/ellipsis"

  interface Props {
    groups: MenuItem[][]
    triggerClass?: string
  }
  let { groups, triggerClass = "btn btn-outline btn-icon" }: Props = $props()

  let open = $state(false)
  let root: HTMLElement | undefined = $state()

  $effect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      if (root && !root.contains(e.target as Node)) open = false
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") open = false
    }
    document.addEventListener("pointerdown", onDown)
    document.addEventListener("keydown", onKey)
    return () => {
      document.removeEventListener("pointerdown", onDown)
      document.removeEventListener("keydown", onKey)
    }
  })
</script>

<div class="relative" bind:this={root}>
  <button
    type="button"
    class={triggerClass}
    aria-label="More"
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <Ellipsis />
  </button>
  {#if open}
    <div
      role="menu"
      class="border-line bg-paper rounded-ui absolute top-full right-0 z-30 mt-2 w-52 border p-1.5 shadow-xl"
    >
      {#each groups as group, gi (gi)}
        {#if gi > 0}<div class="bg-line my-1.5 h-px"></div>{/if}
        {#each group as item (item.label)}
          <button
            type="button"
            role="menuitem"
            class={[
              "hover:bg-raised rounded-ui flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm font-medium",
              item.danger && "text-error",
            ]}
            onclick={() => {
              open = false
              item.onselect()
            }}
          >
            <item.icon class="size-4" />
            {item.label}
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>
