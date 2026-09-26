<script lang="ts">
  /**
   * A native <dialog>: focus trapping, Escape and the backdrop come from the browser.
   * `side` turns it into a slide-over panel; `sheet`, a bottom sheet on phones.
   */
  import X from "@lucide/svelte/icons/x"
  import type { Snippet } from "svelte"

  interface Props {
    open: boolean
    title: string
    description?: string
    side?: boolean
    sheet?: boolean
    children?: Snippet
    footer?: Snippet
    class?: string
  }
  let {
    open = $bindable(),
    title,
    description,
    side = false,
    sheet = false,
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
  class={["modal", side && "modal-side", sheet && "modal-sheet", className]}
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
  /*
   * Open and close both animate: transitions on `display`/`overlay` (allow-discrete) keep
   * the dialog in the top layer until it has faded or slid out. Where that isn't supported
   * it simply closes at once.
   */
  .modal {
    --modal-speed: 0.2s;
    margin: auto;
    width: min(28rem, calc(100vw - 2rem));
    max-height: calc(100dvh - 2rem);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--paper);
    color: var(--text);
    box-shadow: var(--menu-shadow);
    opacity: 0;
    transform: translateY(4px);
    transition:
      opacity var(--modal-speed) ease-out,
      transform var(--modal-speed) ease-out,
      overlay var(--modal-speed) allow-discrete,
      display var(--modal-speed) allow-discrete;
  }
  .modal[open] {
    opacity: 1;
    transform: none;
  }
  @starting-style {
    .modal[open] {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  /* Forest wash; the evening kitchen gets a deeper one */
  .modal::backdrop {
    background: rgb(28 43 34 / 0.45);
    opacity: 0;
    transition:
      opacity var(--modal-speed) ease-out,
      overlay var(--modal-speed) allow-discrete,
      display var(--modal-speed) allow-discrete;
  }
  :global(.dark) .modal::backdrop {
    background: rgb(5 9 7 / 0.65);
  }
  .modal[open]::backdrop {
    opacity: 1;
  }
  @starting-style {
    .modal[open]::backdrop {
      opacity: 0;
    }
  }
  .modal-panel {
    padding: 1.25rem;
  }
  .modal-body {
    margin-top: 1rem;
  }
  /* Phones: up from the bottom edge, full width, rounded on top only */
  @media (max-width: 639px) {
    .modal-sheet {
      --modal-speed: 0.26s;
      margin: auto 0 0;
      width: 100vw;
      max-width: 100vw;
      max-height: calc(100dvh - 2.5rem);
      overflow-y: auto;
      border-width: 1px 0 0;
      border-radius: var(--radius) var(--radius) 0 0;
      opacity: 1;
      transform: translateY(100%);
      transition-timing-function: cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    .modal-sheet[open] {
      transform: none;
    }
    @starting-style {
      .modal-sheet[open] {
        opacity: 1;
        transform: translateY(100%);
      }
    }
    .modal-sheet .modal-panel {
      padding-bottom: max(env(safe-area-inset-bottom), 1.25rem);
    }
  }
  .modal-side {
    --modal-speed: 0.28s;
    margin: 0 0 0 auto;
    width: min(26rem, 90vw);
    height: 100dvh;
    max-height: 100dvh;
    border-radius: var(--radius) 0 0 var(--radius);
    overflow-y: auto;
    opacity: 1;
    transform: translateX(100%);
    transition-timing-function: cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .modal-side[open] {
    transform: none;
  }
  @starting-style {
    .modal-side[open] {
      opacity: 1;
      transform: translateX(100%);
    }
  }
</style>
