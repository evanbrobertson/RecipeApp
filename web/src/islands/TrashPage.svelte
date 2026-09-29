<script lang="ts">
  /**
   * The trash (/more/trash): recipes deleted in the last 30 days (src/trash.rs). Each can be
   * put back, with its cookbooks and cook log, or deleted for good; so can all of them.
   */
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import { onMount } from "svelte"
  import Modal from "../components/Modal.svelte"
  import Photo from "../components/Photo.svelte"
  import { api, errorMessage } from "../lib/api"
  import { toast } from "../lib/toast"

  interface Trashed {
    id: number
    title: string
    url: string | null
    image: string | null
    deletedAt: string
    purgeAt: string
  }

  let items = $state<Trashed[] | null>(null)
  let busy = $state<number | "all" | null>(null)
  let confirmEmpty = $state(false)

  onMount(async () => {
    try {
      items = await api<Trashed[]>("/api/trash")
    } catch (e) {
      items = []
      toast({ title: "Couldn't open the trash", description: errorMessage(e), tone: "error" })
    }
  })

  /** "12 days left", counted to the day it goes. */
  function left(purgeAt: string): string {
    const days = Math.max(0, Math.ceil((Date.parse(purgeAt) - Date.now()) / 86_400_000))
    return days <= 1 ? "Goes for good today" : `${days} days left`
  }

  async function restore(item: Trashed) {
    busy = item.id
    try {
      const recipe = await api<{ id: number; isNew: boolean }>(`/api/trash/${item.id}/restore`, {
        method: "POST",
      })
      items = items?.filter((i) => i.id !== item.id) ?? null
      toast({
        title: recipe.isNew ? "Put back" : "It's already in your box",
        description: recipe.isNew ? item.title : "Its link was saved again since",
        tone: "success",
        action: { label: "Open", onselect: () => (location.href = `/recipes/${recipe.id}`) },
      })
    } catch (e) {
      toast({ title: "Couldn't put it back", description: errorMessage(e), tone: "error" })
    } finally {
      busy = null
    }
  }

  async function purge(item: Trashed) {
    busy = item.id
    try {
      await api(`/api/trash/${item.id}`, { method: "DELETE" })
      items = items?.filter((i) => i.id !== item.id) ?? null
    } catch (e) {
      toast({ title: "Couldn't delete it", description: errorMessage(e), tone: "error" })
    } finally {
      busy = null
    }
  }

  async function emptyAll() {
    busy = "all"
    try {
      await api("/api/trash", { method: "DELETE" })
      items = []
      confirmEmpty = false
      toast({ title: "Trash emptied" })
    } catch (e) {
      toast({ title: "Couldn't empty the trash", description: errorMessage(e), tone: "error" })
    } finally {
      busy = null
    }
  }
</script>

{#if items === null}
  <div class="skeleton rounded-ui h-40"></div>
{:else if !items.length}
  <div class="card text-ink-muted p-6 text-center">Nothing in the trash.</div>
{:else}
  <section class="space-y-4">
    <div class="list-card">
      {#each items as item (item.id)}
        <div class="list-row">
          <Photo
            recipe={item}
            width={56}
            class="rounded-ctl size-14 flex-none object-cover"
            iconClass="size-6"
          />
          <span class="min-w-0 flex-1">
            <span class="block truncate font-bold">{item.title}</span>
            <span class="text-ink-muted block text-sm">{left(item.purgeAt)}</span>
          </span>
          <button
            type="button"
            class="btn btn-soft"
            disabled={busy !== null}
            onclick={() => restore(item)}
          >
            <RotateCcw class="size-4" /> Put back
          </button>
          <button
            type="button"
            class="btn btn-ghost px-3"
            aria-label={`Delete ${item.title} for good`}
            title="Delete for good"
            disabled={busy !== null}
            onclick={() => purge(item)}
          >
            <Trash2 class="size-4" />
          </button>
        </div>
      {/each}
    </div>
    <button
      type="button"
      class="btn btn-ghost"
      disabled={busy !== null}
      onclick={() => (confirmEmpty = true)}
    >
      Empty the trash
    </button>
  </section>
{/if}

<Modal
  bind:open={confirmEmpty}
  title="Empty the trash?"
  description={`${items?.length ?? 0} recipe${items?.length === 1 ? "" : "s"} will be deleted for good. This can't be undone.`}
>
  {#snippet footer()}
    <button type="button" class="btn btn-ghost" onclick={() => (confirmEmpty = false)}>
      Cancel
    </button>
    <button type="button" class="btn btn-danger" disabled={busy !== null} onclick={emptyAll}>
      Empty the trash
    </button>
  {/snippet}
</Modal>
