<script lang="ts">
  /**
   * A native <dialog>: focus trapping, Escape and the backdrop come from the browser.
   * `side` turns it into a slide-over panel.
   */
  import X from "@lucide/svelte/icons/x"
  import type { Snippet } from "svelte"

  interface Props {
    open: boolean
    title: string
    description?: string
    side?: boolean
    children?: Snippet
    footer?: Snippet
    class?: string
  }
  let {
    open = $bindable(),
    title,
    description,
    side = false,
    children,
    footer,
    class: className,
  }: Props = $props()

  let dialog: HTMLDialogElement | undefined = $state()

  $effect(() => {
    if (!dialog) return
    if (open && !dialog.open) dialog.showModal()
    if (!open && dialog.open) dialog.close()
  })
</script>

<dialog
  bind:this={dialog}
  class={["modal", side && "modal-side", className]}
  aria-label={title}
  onclose={() => (open = false)}
  onclick={(e) => {
    if (e.target === dialog) open = false
  }}
>
  <div class="modal-panel">
    <header class="flex items-start justify-between gap-3">
      <div>
        <h2 class="section-title">{title}</h2>
        {#if description}<p class="text-ink-muted mt-1.5 text-sm">{description}</p>{/if}
      </div>
      <button
        type="button"
        class="btn btn-ghost btn-icon -mt-1.5 -mr-2.5"
        aria-label="Close"
        onclick={() => (open = false)}
      >
        <X />
      </button>
    </header>
    {#if children}<div class="modal-body">{@render children()}</div>{/if}
    {#if footer}<footer class="mt-6 flex flex-wrap justify-end gap-2">{@render footer()}</footer>{/if}
  </div>
</dialog>

<style>
  .modal {
    margin: auto;
    width: min(28rem, calc(100vw - 2rem));
    max-height: calc(100dvh - 2rem);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--paper);
    color: var(--text);
    box-shadow: var(--menu-shadow);
  }
  .modal[open] {
    animation: modal-in 0.2s ease-out;
  }
  /* Forest wash; the evening kitchen gets a deeper one */
  .modal::backdrop {
    background: rgb(28 43 34 / 0.45);
  }
  :global(.dark) .modal::backdrop {
    background: rgb(5 9 7 / 0.65);
  }
  .modal-panel {
    padding: 1.25rem;
  }
  .modal-body {
    margin-top: 1rem;
  }
  .modal-side {
    margin: 0 0 0 auto;
    width: min(26rem, 90vw);
    height: 100dvh;
    max-height: 100dvh;
    border-radius: var(--radius) 0 0 var(--radius);
    overflow-y: auto;
  }
  .modal-side[open] {
    animation: side-in 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes modal-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  @keyframes side-in {
    from {
      transform: translateX(100%);
    }
  }
</style>
