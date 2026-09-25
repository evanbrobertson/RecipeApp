<script lang="ts" module>
  export type AddMode = "auto" | "link" | "text" | "file" | "claude" | "scratch" | "apps"

  export const MODES: { mode: AddMode; label: string; hint: string }[] = [
    { mode: "auto", label: "Auto-detect", hint: "Crumb works out what you pasted" },
    { mode: "link", label: "Link", hint: "Import from a website" },
    { mode: "text", label: "Recipe text", hint: "Paste the whole recipe" },
    { mode: "file", label: "File", hint: "PDF, saved web page, text or backup" },
    { mode: "claude", label: "Claude", hint: "Send a recipe from a Claude chat" },
    { mode: "scratch", label: "From scratch", hint: "Write it out yourself" },
    { mode: "apps", label: "Other apps", hint: "Import from another recipe app" },
  ]

  const URL_RE = /^https?:\/\/\S+$/i
  // A line that reads like an ingredient: starts with an amount or a bullet, or names a unit
  const INGREDIENT_RE =
    /^\s*([-•*▢□]|\d|[½¼¾⅓⅔⅛]|a (pinch|handful|few))|\b(cups?|tbsp|tsp|tablespoons?|teaspoons?|grams?|g|kg|ml|l|oz|ounces?|lbs?|pounds?|cloves?|pinch)\b/i

  export interface Detected {
    mode: AddMode
    summary: string
  }

  /** What was pasted, and a short summary for the footer (it has about 100px). */
  export function detect(raw: string): Detected {
    const text = raw.trim()
    if (!text) return { mode: "auto", summary: "" }
    const tokens = text.split(/\s+/)
    const urls = tokens.filter((t) => URL_RE.test(t))
    if (urls.length && urls.length === tokens.length) {
      if (urls.length > 1) return { mode: "link", summary: `${urls.length} links` }
      try {
        return { mode: "link", summary: new URL(urls[0]!).hostname.replace(/^www\./, "") }
      } catch {
        return { mode: "link", summary: "" }
      }
    }
    const lines = text.split(/\n+/).filter((l) => l.trim())
    // Shared text like "Best lasagna https://…": one link and a few words on one line
    if (urls.length === 1 && lines.length === 1) {
      try {
        return { mode: "link", summary: new URL(urls[0]!).hostname.replace(/^www\./, "") }
      } catch {
        return { mode: "link", summary: "" }
      }
    }
    if (lines.length >= 3) {
      const n = lines.filter((l) => INGREDIENT_RE.test(l)).length
      return {
        mode: "text",
        summary: n ? `${n} ingredient${n === 1 ? "" : "s"}` : `${lines.length} lines`,
      }
    }
    if (lines.length === 1 && text.length <= 80) return { mode: "scratch", summary: "New recipe" }
    return { mode: "text", summary: `${lines.length} line${lines.length === 1 ? "" : "s"}` }
  }
</script>

<script lang="ts">
  /**
   * The top box: Search (the default) and Add, switched by tabs like the dividers in a
   * recipe box. Add is one field; Crumb works out what was pasted and shows it in a chip,
   * which also opens a menu to pick the mode by hand.
   */
  import Check from "@lucide/svelte/icons/check"
  import ChevronDown from "@lucide/svelte/icons/chevron-down"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import Search from "@lucide/svelte/icons/search"
  import { onMount, tick } from "svelte"
  import { api, errorMessage } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { flash, toast } from "../lib/toast"

  interface Props {
    /** Show the Search/Add tabs (Home). Without them it's just the Add block (/add). */
    tabs?: boolean
    /** Sits on the tile header; off it, the block gets a border. */
    onTile?: boolean
    autofocus?: boolean
  }
  let { tabs = true, onTile = true, autofocus = false }: Props = $props()

  let tab = $state<"search" | "add">(tabs ? "search" : "add")
  let query = $state("")
  let input = $state("")
  let file = $state<File | null>(null)
  let manual = $state<AddMode | null>(null)
  let menuOpen = $state(false)
  let saving = $state(false)
  let count = $state<number | null>(null)
  let detected = $state<Detected>({ mode: "auto", summary: "" })

  let addField: HTMLTextAreaElement | undefined = $state()
  let searchField: HTMLInputElement | undefined = $state()
  let picker: HTMLInputElement | undefined = $state()
  let root: HTMLElement | undefined = $state()

  // Debounced detection (≈250ms), so the chip doesn't flicker while typing
  $effect(() => {
    const text = input
    if (file) {
      detected = { mode: "file", summary: file.name }
      return
    }
    const t = setTimeout(() => (detected = detect(text)), text.trim() ? 250 : 0)
    return () => clearTimeout(t)
  })
  // Clearing the field hands the choice back to detection
  function oninput() {
    if (!input.trim() && !file) manual = null
  }

  const mode = $derived<AddMode>(manual ?? detected.mode)
  const modeLabel = $derived(MODES.find((m) => m.mode === mode)!.label)
  const linkCount = $derived(input.trim().split(/\s+/).filter((t) => URL_RE.test(t)).length)
  const summary = $derived.by(() => {
    if (saving) return mode === "link" ? "Fetching… tricky sites take ~20s" : "Reading…"
    if (mode === "claude") return "Opens the connector setup"
    if (mode === "apps") return "Opens the importer"
    if (mode === "file") return file ? file.name : "Choose a file"
    return manual && manual !== detected.mode ? "" : detected.summary
  })
  // What Add would do right now (the chip lags by the debounce; this doesn't)
  const now = $derived<AddMode>(manual ?? (file ? "file" : detect(input).mode))
  const canAdd = $derived(
    !saving &&
      (now === "claude" ||
        now === "apps" ||
        now === "file" ||
        (now === "link" && linkCount > 0) ||
        (now === "scratch" && input.trim().length > 0) ||
        ((now === "text" || now === "auto") && input.trim().length >= 10)),
  )

  onMount(() => {
    try {
      const data = JSON.parse(document.getElementById("page-data")?.textContent ?? "null")
      if (typeof data?.recipeCount === "number") count = data.recipeCount
    } catch {}
    // Prefilled from the PWA share target (/add?url=…&text=…)
    const params = new URLSearchParams(location.search)
    const shared = [params.get("url"), params.get("text")].find((v) => v && v.trim())
    if (shared) {
      input = shared.trim()
      tab = "add"
    }
    if (autofocus) (tab === "add" ? addField : searchField)?.focus()
    const close = (e: PointerEvent) => {
      if (menuOpen && root && !root.contains(e.target as Node)) menuOpen = false
    }
    addEventListener("pointerdown", close)
    return () => removeEventListener("pointerdown", close)
  })

  async function switchTab(next: "search" | "add") {
    tab = next
    menuOpen = false
    await tick()
    ;(next === "add" ? addField : searchField)?.focus()
  }

  function onTabKey(e: KeyboardEvent) {
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault()
      void switchTab(tab === "search" ? "add" : "search")
    }
  }

  function search(e: Event) {
    e.preventDefault()
    const q = query.trim()
    location.href = q ? `/recipes?q=${encodeURIComponent(q)}` : "/recipes"
  }

  function choose(m: AddMode) {
    manual = m === "auto" ? null : m
    menuOpen = false
    if (m !== "file") file = null
    if (m === "file") picker?.click()
    else addField?.focus()
  }

  // The server's importer reads PDFs, saved web pages, text/Markdown and recipe JSON
  async function importFile(f: File) {
    saving = true
    try {
      const form = new FormData()
      form.append("file", f)
      const [result] = await api<
        { created: { id: number; title: string }[]; duplicates: number; error?: string }[]
      >("/api/import/files", { method: "POST", form })
      if (!result || result.error) throw new Error(result?.error ?? "Nothing in it looked like a recipe")
      if (result.created.length === 1) {
        location.href = `/recipes/${result.created[0]!.id}`
        return
      }
      flash({
        title: result.created.length
          ? `Imported ${result.created.length} recipes`
          : "Already in your recipes",
        tone: result.created.length ? "success" : undefined,
      })
      location.href = "/recipes"
    } catch (err) {
      toast({ title: "Couldn't read that file", description: errorMessage(err), tone: "error" })
      saving = false
    }
  }

  async function importBody(body: { url: string } | { text: string }, what: string) {
    saving = true
    try {
      const res = await api<{ id: number; isNew: boolean }>("/api/recipes/import", {
        method: "POST",
        body,
      })
      if (!res.isNew) flash({ title: "Already in your recipes" })
      location.href = `/recipes/${res.id}`
    } catch (err) {
      toast({ title: `Couldn't ${what}`, description: errorMessage(err), tone: "error" })
      saving = false
    }
  }

  async function add(e?: Event) {
    e?.preventDefault()
    if (!canAdd) return
    const text = input.trim()
    // Don't wait for the debounced chip: a paste-and-submit uses what's there now
    switch (now) {
      case "claude":
        location.href = "/connect"
        return
      case "apps":
        location.href = "/import"
        return
      case "scratch":
        location.href = `/recipes/new?title=${encodeURIComponent(text)}`
        return
      case "file":
        if (!file) {
          picker?.click()
          return
        }
        if (/^(image|video|audio)\//.test(file.type) || /\.(docx?|pages|rtf)$/i.test(file.name)) {
          toast({
            title: "Crumb can't read that kind of file yet",
            description: "Open it, copy the recipe text, and paste it here instead.",
          })
          return
        }
        return importFile(file)
      case "link": {
        const urls = text.split(/\s+/).filter((t) => URL_RE.test(t))
        if (urls.length > 1) {
          location.href = `/import?links=${encodeURIComponent(urls.join("\n"))}`
          return
        }
        return importBody({ url: urls[0]! }, "get that recipe")
      }
      default:
        return importBody({ text: input }, "read that recipe")
    }
  }

  function onAddKey(e: KeyboardEvent) {
    if (e.key === "Escape" && menuOpen) menuOpen = false
    else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void add()
    // Links and names submit on Enter; pasted text needs Shift+Enter for new lines anyway
    else if (e.key === "Enter" && !e.shiftKey && (now === "link" || now === "scratch")) {
      e.preventDefault()
      void add()
    }
  }

  function onDrop(e: DragEvent) {
    const f = e.dataTransfer?.files?.[0]
    if (!f) return
    e.preventDefault()
    file = f
    manual = "file"
  }
</script>

<div class={["topbox", onTile && "on-tile"]} bind:this={root}>
  {#if tabs}
    <div class="tabs" role="tablist" aria-label="Search or add">
      <button
        type="button"
        role="tab"
        id="tb-search-tab"
        aria-selected={tab === "search"}
        aria-controls="tb-search"
        tabindex={tab === "search" ? 0 : -1}
        class="tab"
        onclick={() => switchTab("search")}
        onkeydown={onTabKey}
      >
        <Search class="size-[18px]" /> Search
      </button>
      <button
        type="button"
        role="tab"
        id="tb-add-tab"
        aria-selected={tab === "add"}
        aria-controls="tb-add"
        tabindex={tab === "add" ? 0 : -1}
        class="tab"
        onclick={() => switchTab("add")}
        onkeydown={onTabKey}
      >
        <Plus class="size-[18px]" /> Add
      </button>
    </div>
  {/if}

  {#if tab === "search"}
    <form
      id="tb-search"
      role="tabpanel"
      aria-labelledby="tb-search-tab"
      class="search"
      onsubmit={search}
    >
      <input
        bind:this={searchField}
        bind:value={query}
        type="search"
        enterkeyhint="search"
        placeholder={count ? `Search ${count} recipes` : "Search recipes"}
        aria-label="Search recipes by title, ingredient or category"
      />
    </form>
  {:else}
    <form
      id="tb-add"
      role={tabs ? "tabpanel" : undefined}
      aria-labelledby={tabs ? "tb-add-tab" : undefined}
      class={["add", tabs && "joined-second"]}
      onsubmit={add}
      ondragover={(e) => e.preventDefault()}
      ondrop={onDrop}
    >
      <textarea
        bind:this={addField}
        bind:value={input}
        use:autosize
        {oninput}
        onkeydown={onAddKey}
        rows="1"
        placeholder="Paste a link, a recipe, or type a name"
        aria-label="Recipe link, text or name"
        disabled={saving}
      ></textarea>
      <div class="footer">
        <button
          type="button"
          class="mode-chip"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          onclick={() => (menuOpen = !menuOpen)}
        >
          {modeLabel}
          <ChevronDown class={["size-4 transition", menuOpen && "rotate-180"]} />
        </button>
        <span class="summary" aria-live="polite">{summary}</span>
        <button type="submit" class="add-btn" disabled={!canAdd}>
          {#if saving}<LoaderCircle class="size-4 animate-spin" />{/if}
          {mode === "claude" || mode === "apps"
            ? "Open"
            : mode === "link" && linkCount > 1
              ? "Import all"
              : "Add"}
        </button>
      </div>
      <input
        bind:this={picker}
        type="file"
        class="hidden"
        accept=".pdf,.txt,.md,.markdown,.html,.htm,.json,application/pdf,text/plain,text/markdown,text/html,application/json"
        onchange={(e) => {
          const f = (e.currentTarget as HTMLInputElement).files?.[0]
          if (f) {
            file = f
            manual = "file"
          }
        }}
      />
    </form>

    {#if menuOpen}
      <div class="menu menu-surface" role="menu" aria-label="How to add">
        {#each MODES as m (m.mode)}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={mode === m.mode}
            class="menu-row"
            onclick={() => choose(m.mode)}
          >
            <span class="min-w-0 flex-1">
              <span class="block text-[15px] font-bold">{m.label}</span>
              <span class="text-ink-muted block text-[13px]">{m.hint}</span>
            </span>
            {#if mode === m.mode}<Check class="text-primary size-5 flex-none" />{/if}
          </button>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .topbox {
    position: relative;
    text-align: left;
  }
  .tabs {
    display: flex;
    gap: 0.25rem;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    height: 2.75rem;
    padding: 0 1rem;
    border-radius: var(--radius-ctl) var(--radius-ctl) 0 0;
    font-size: 0.9375rem;
    font-weight: 600;
    transition:
      background-color 0.18s ease-out,
      color 0.18s ease-out;
  }
  .on-tile .tab {
    background: rgb(255 253 248 / 0.16);
    color: var(--on-tile);
  }
  .tab:not([aria-selected="true"]) {
    background: var(--tint);
    color: var(--text-muted);
  }
  .on-tile .tab:not([aria-selected="true"]) {
    background: rgb(255 253 248 / 0.16);
    color: var(--on-tile);
  }
  .on-tile .tab:not([aria-selected="true"]):hover {
    background: rgb(255 253 248 / 0.24);
  }
  .tab[aria-selected="true"] {
    background: var(--paper);
    color: var(--text);
    font-weight: 700;
  }
  .tab:focus-visible {
    outline-color: var(--butter);
  }

  .search,
  .add {
    background: var(--paper);
    color: var(--text);
    border-radius: var(--radius);
  }
  .topbox:not(.on-tile) .search,
  .topbox:not(.on-tile) .add {
    border: 1px solid var(--border);
  }
  /* The first tab joins the field, so that corner is square */
  .tabs + .search {
    border-top-left-radius: 0;
  }
  .search input {
    display: block;
    width: 100%;
    height: 3.5rem;
    padding: 0 1rem;
    border: 0;
    border-radius: inherit;
    background: transparent;
    font-size: 1rem;
    outline: none;
  }
  .search input::placeholder,
  .add textarea::placeholder {
    color: var(--text-muted);
  }
  .search:focus-within,
  .add:focus-within {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--butter) 80%, transparent);
  }
  .topbox:not(.on-tile) .search:focus-within,
  .topbox:not(.on-tile) .add:focus-within {
    border-color: var(--tile);
    box-shadow: 0 0 0 3px var(--tint);
  }

  .add textarea {
    display: block;
    width: 100%;
    min-height: 3.5rem;
    max-height: 20rem;
    padding: 1rem;
    border: 0;
    background: transparent;
    font-size: 1rem;
    line-height: 1.45;
    resize: none;
    field-sizing: content;
    outline: none;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.25rem;
    border-top: 1px solid var(--border);
  }
  .mode-chip {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 0.25rem;
    height: 2.75rem;
    padding: 0 0.625rem 0 1rem;
    border-radius: 999px;
    background: var(--tint);
    color: var(--primary);
    font-size: 0.875rem;
    font-weight: 700;
  }
  .summary {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 0.875rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .add-btn {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 0.375rem;
    height: 2.75rem;
    padding: 0 1.25rem;
    border-radius: var(--radius-ctl);
    background: var(--butter);
    color: var(--on-butter);
    font-size: 0.9375rem;
    font-weight: 700;
    transition: background-color 0.15s;
  }
  .add-btn:hover:not(:disabled) {
    background: var(--butter-hover);
  }
  .add-btn:disabled {
    background: var(--tint);
    color: var(--text-muted);
  }

  .menu {
    position: absolute;
    top: calc(100% + 0.5rem);
    left: 0;
    right: 0;
    z-index: 30;
    padding: 0.25rem 0;
    color: var(--text);
    animation: menu-in 0.18s ease-out;
  }
  .menu-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    min-height: 3.5rem;
    padding: 0.5rem 1rem;
    text-align: left;
  }
  .menu-row:hover,
  .menu-row:focus-visible {
    background: var(--tint);
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
