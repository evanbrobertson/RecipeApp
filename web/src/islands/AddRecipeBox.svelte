<script lang="ts">
  import ArrowRight from "@lucide/svelte/icons/arrow-right"
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste"
  import Link from "@lucide/svelte/icons/link"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import TextIcon from "@lucide/svelte/icons/text"
  import { onMount } from "svelte"
  import { api, errorMessage } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { flash, toast } from "../lib/toast"

  let { autofocus = false }: { autofocus?: boolean } = $props()

  let input = $state("")
  let saving = $state(false)
  let field: HTMLTextAreaElement | undefined = $state()

  const trimmed = $derived(input.trim())
  const tokens = $derived(trimmed.split(/\s+/).filter(Boolean))
  const urls = $derived(tokens.filter((t) => /^https?:\/\/\S+$/i.test(t)))
  // Several links pasted at once → bulk import page
  const isMultiUrl = $derived(urls.length > 1 && tokens.length === urls.length)
  const isUrl = $derived(urls.length === 1 && trimmed === urls[0])
  const canSubmit = $derived(isUrl || isMultiUrl || trimmed.length >= 10)

  const status = $derived.by(() => {
    if (saving)
      return isUrl ? "Fetching the recipe… tricky sites can take ~20s" : "Reading your recipe…"
    if (isMultiUrl) return `${urls.length} links, imported one by one`
    if (isUrl) return "Link detected"
    if (trimmed) return "Recipe text"
    return ""
  })

  onMount(() => {
    // Prefill from the PWA share target (/add?url=…&text=…)
    const params = new URLSearchParams(location.search)
    const shared = [params.get("url"), params.get("text")].find((v) => v && v.trim())
    if (shared) input = shared.trim()
    if (autofocus) field?.focus()
  })

  async function pasteFromClipboard() {
    try {
      input = await navigator.clipboard.readText()
      field?.focus()
    } catch {
      toast({ title: "Clipboard access was blocked", description: "Paste with Ctrl/⌘+V instead." })
    }
  }

  async function submit(e?: Event) {
    e?.preventDefault()
    if (!canSubmit || saving) return
    if (isMultiUrl) {
      location.href = `/import?links=${encodeURIComponent(urls.join("\n"))}`
      return
    }
    saving = true
    try {
      const body = isUrl ? { url: trimmed } : { text: input }
      const res = await api<{ id: number; isNew: boolean }>("/api/recipes/import", {
        method: "POST",
        body,
      })
      if (!res.isNew) flash({ title: "Already in your recipes" })
      location.href = `/recipes/${res.id}`
    } catch (err) {
      toast({
        title: isUrl ? "Couldn't get that recipe" : "Couldn't read that recipe",
        description: errorMessage(err),
        tone: "error",
      })
      saving = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit()
    // A single pasted link submits on Enter
    else if (e.key === "Enter" && !e.shiftKey && isUrl) {
      e.preventDefault()
      submit()
    }
  }
</script>

<form class="paste-box" onsubmit={submit}>
  <textarea
    bind:this={field}
    bind:value={input}
    use:autosize
    {onkeydown}
    rows="1"
    class="paste-input"
    placeholder="Paste a recipe link, or the whole recipe text…"
    aria-label="Recipe link or text"
    disabled={saving}
  ></textarea>
  <div class="paste-bar">
    <div class="text-ink-muted flex min-w-0 items-center gap-2 text-sm">
      {#if status}
        {#if saving}
          <LoaderCircle class="size-4 shrink-0 animate-spin" />
        {:else if isUrl || isMultiUrl}
          <Link class="size-4 shrink-0" />
        {:else}
          <TextIcon class="size-4 shrink-0" />
        {/if}
        <span class="truncate">{status}</span>
      {:else}
        <button type="button" class="btn btn-ghost btn-sm -ml-2" onclick={pasteFromClipboard}>
          <ClipboardPaste /> Paste
        </button>
      {/if}
    </div>
    <button type="submit" class="btn btn-primary shrink-0" disabled={!canSubmit || saving}>
      {isMultiUrl ? "Import all" : saving ? "Saving…" : "Save recipe"}
      {#if saving}<LoaderCircle class="animate-spin" />{:else}<ArrowRight />{/if}
    </button>
  </div>
</form>

<style>
  .paste-box {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-accented);
    border-radius: var(--radius);
    background: var(--paper);
    box-shadow: 0 1px 2px rgb(60 30 10 / 0.06);
    text-align: left;
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
  }
  .paste-box:focus-within {
    border-color: var(--primary);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .paste-input {
    display: block;
    width: 100%;
    min-height: 3.5rem;
    max-height: 22rem;
    padding: 1rem 1rem 0.5rem;
    border: 0;
    background: transparent;
    color: var(--text);
    font-size: 1.0625rem;
    line-height: 1.5;
    resize: none;
    field-sizing: content;
    outline: none;
  }
  .paste-input::placeholder {
    color: var(--text-dimmed);
  }
  .paste-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.5rem 0.5rem 0.5rem 1rem;
  }
</style>
