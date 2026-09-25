<script lang="ts" module>
  import type { Component } from "svelte"

  export interface MenuItem {
    label: string
    icon: Component<{ class?: string }>
    /** A button's action. Items with `href` are links instead. */
    onselect?: () => void
    /** A link (e.g. a file download); never prerendered. */
    href?: string
    /** With `href`: download the response instead of navigating. */
    download?: boolean
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
    <div role="menu" class="menu-surface menu-pop absolute top-full right-0 z-30 mt-2 w-56 py-1">
      {#each groups as group, gi (gi)}
        {#if gi > 0}<div class="bg-line mx-3 my-1 h-px"></div>{/if}
        {#each group as item (item.label)}
          {@const itemClass = [
            "hover:bg-tint active:bg-tint flex min-h-11 w-full items-center gap-3 px-4 py-2 text-left text-[15px] font-bold transition-colors",
            item.danger ? "text-error" : "text-ink",
          ]}
          {@const iconClass = item.danger
            ? "size-[18px] flex-none"
            : "text-primary size-[18px] flex-none"}
          {#if item.href}
            <a
              href={item.href}
              role="menuitem"
              class={itemClass}
              download={item.download ? "" : undefined}
              data-no-prerender
              onclick={() => {
                open = false
                item.onselect?.()
              }}
            >
              <item.icon class={iconClass} />
              {item.label}
            </a>
          {:else}
            <button
              type="button"
              role="menuitem"
              class={itemClass}
              onclick={() => {
                open = false
                item.onselect?.()
              }}
            >
              <item.icon class={iconClass} />
              {item.label}
            </button>
          {/if}
        {/each}
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu-pop {
    animation: menu-in 0.16s ease-out;
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
