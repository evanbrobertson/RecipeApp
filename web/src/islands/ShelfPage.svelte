<script lang="ts">
  import LibraryBig from "@lucide/svelte/icons/library-big"
  import Plus from "@lucide/svelte/icons/plus"
  import BookColorPicker from "../components/BookColorPicker.svelte"
  import Bookshelf from "../components/Bookshelf.svelte"
  import EmptyState from "../components/EmptyState.svelte"
  import Modal from "../components/Modal.svelte"
  import OpenBook from "../components/OpenBook.svelte"
  import { api, errorMessage } from "../lib/api"
  import { randomBookColor, type ShelfBook } from "../lib/books"
  import { autosize } from "../lib/autosize"
  import { pageState } from "../lib/page.svelte"
  import type { Cookbook } from "../lib/recipe"
  import { toast } from "../lib/toast"

  const page = pageState<{ cookbooks: Cookbook[] }>(async () => ({
    cookbooks: await api<Cookbook[]>("/api/cookbooks"),
  }))

  let opened = $state<ShelfBook | null>(null)
  let showCreate = $state(false)
  let form = $state({ name: "", description: "", color: "tomato" as string })
  let creating = $state(false)

  function startCreate() {
    form = { name: "", description: "", color: randomBookColor() }
    showCreate = true
  }

  async function create(e: SubmitEvent) {
    e.preventDefault()
    if (!form.name.trim() || !page.data) return
    creating = true
    try {
      const book = await api<Cookbook>("/api/cookbooks", { method: "POST", body: form })
      page.data.cookbooks = await api<Cookbook[]>("/api/cookbooks")
      showCreate = false
      toast({ title: `“${book.name}” is on the shelf`, tone: "success" })
    } catch (err) {
      toast({ title: "Couldn't create cookbook", description: errorMessage(err), tone: "error" })
    } finally {
      creating = false
    }
  }
</script>

<div>
  <div class="mb-8 flex items-end justify-between gap-3">
    <div>
      <h1 class="font-serif text-3xl font-semibold sm:text-4xl">Your shelf</h1>
      <p class="text-ink-muted mt-1">Pull a cookbook off the shelf to open it.</p>
    </div>
    <button type="button" class="btn btn-primary" onclick={startCreate}>
      <Plus /> New cookbook
    </button>
  </div>

  {#if page.loading}
    <div class="skeleton h-64 w-full"></div>
  {:else if !page.data?.cookbooks.length}
    <EmptyState
      icon={LibraryBig}
      title="An empty shelf"
      description="Cookbooks group recipes, like “Weeknight dinners” or “Christmas baking”."
    >
      <button type="button" class="btn btn-primary" onclick={startCreate}>
        Make your first cookbook
      </button>
    </EmptyState>
  {:else}
    <Bookshelf
      books={page.data.cookbooks}
      pulledId={opened?.id}
      addable
      onopen={(b) => (opened = b)}
      onadd={startCreate}
    />
  {/if}

  <OpenBook book={opened} onclose={() => (opened = null)} />

  <Modal bind:open={showCreate} title="A new cookbook">
    <form id="create-cookbook" class="space-y-4" onsubmit={create}>
      <div>
        <label class="label" for="cb-name">Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="cb-name" bind:value={form.name} class="input" placeholder="Weeknight dinners" autofocus />
      </div>
      <div>
        <label class="label" for="cb-desc">Description</label>
        <textarea id="cb-desc" use:autosize bind:value={form.description} rows="2" class="input"></textarea>
      </div>
      <div>
        <span class="label">Cover</span>
        <BookColorPicker bind:value={form.color} />
      </div>
    </form>
    {#snippet footer()}
      <button type="button" class="btn btn-ghost" onclick={() => (showCreate = false)}>Cancel</button>
      <button
        type="submit"
        form="create-cookbook"
        class="btn btn-primary"
        disabled={creating || !form.name.trim()}
      >
        Put it on the shelf
      </button>
    {/snippet}
  </Modal>
</div>
