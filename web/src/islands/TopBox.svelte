<script lang="ts" module>
  export type AddMode =
    | "auto"
    | "link"
    | "text"
    | "photo"
    | "file"
    | "claude"
    | "scratch"
    | "apps"

  export const MODES: { mode: AddMode; label: string; hint: string }[] = [
    { mode: "auto", label: "Auto-detect", hint: "Crumb works out what you pasted" },
    { mode: "link", label: "Link", hint: "Import from a website" },
    { mode: "text", label: "Recipe text", hint: "Paste the whole recipe" },
    { mode: "photo", label: "Photo", hint: "Snap or upload the pages of a recipe" },
    { mode: "file", label: "File", hint: "PDF, saved web page, text or backup" },
    { mode: "claude", label: "Claude", hint: "Send a recipe from a Claude chat" },
    { mode: "scratch", label: "From scratch", hint: "Write it out yourself" },
    { mode: "apps", label: "Other apps", hint: "Import from another recipe app" },
  ]

  /** Handwritten hints beside Add; `short` fits the tab row on narrow screens, on two lines. */
  export const TIPS: { long: string; short: string }[] = [
    { long: "Try pasting the whole recipe", short: "paste the\nwhole recipe" },
    { long: "Try uploading a photo of your recipe", short: "or snap a\nphoto of it" },
    { long: "Try uploading a PDF of your recipe", short: "PDFs\nwork too" },
    { long: "Paste a link from any recipe site", short: "links from\nany site" },
    { long: "Drop in a Crumb or Just the Recipe backup", short: "backups\nwork too" },
    { long: "Paste a few links at once", short: "a few links\nat once" },
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
  import ChevronLeft from "@lucide/svelte/icons/chevron-left"
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import ImageOff from "@lucide/svelte/icons/image-off"
  import ImagePlus from "@lucide/svelte/icons/image-plus"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Plus from "@lucide/svelte/icons/plus"
  import Search from "@lucide/svelte/icons/search"
  import X from "@lucide/svelte/icons/x"
  import { onMount, tick } from "svelte"
  import { api, errorMessage } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { bookImportedTitle } from "../lib/format"
  import type { BookImported } from "../lib/recipe"
  import { importPhotos, isPhoto, MAX_PHOTOS, shrink, visionAvailable } from "../lib/photos"
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
  interface Page {
    key: number
    name: string
    /** The shrunk JPEG to send, once ready (or the original if it can't be decoded). */
    blob: Blob | null
    /** Thumbnail; null while shrinking or when the browser can't show it. */
    url: string | null
  }
  let pages = $state<Page[]>([])
  let pageKey = 0
  /** Wee Chef reads photos here; null until asked. */
  let vision = $state<boolean | null>(null)
  let progress = $state("")
  let manual = $state<AddMode | null>(null)
  let menuOpen = $state(false)
  let saving = $state(false)
  let count = $state<number | null>(null)
  let detected = $state<Detected>({ mode: "auto", summary: "" })
  let tipIndex = $state(-1)
  /** The Add field has focus (a focused note is never hidden out from under the cursor). */
  let fieldFocused = $state(false)
  // Only on Home's tile header, where there's room for it
  const tip = $derived(tabs && onTile && tab === "add" ? TIPS[tipIndex] : undefined)

  /** A different tip each time Add is opened. */
  function pickTip() {
    let i = Math.floor(Math.random() * TIPS.length)
    if (i === tipIndex) i = (i + 1) % TIPS.length
    tipIndex = i
  }

  let addField: HTMLTextAreaElement | undefined = $state()
  let searchField: HTMLInputElement | undefined = $state()
  let picker: HTMLInputElement | undefined = $state()
  let photoPicker: HTMLInputElement | undefined = $state()
  let root: HTMLElement | undefined = $state()

  // Debounced detection (≈250ms), so the chip doesn't flicker while typing
  $effect(() => {
    const text = input
    if (pages.length) {
      detected = {
        mode: "photo",
        summary: `${pages.length} photo${pages.length === 1 ? "" : "s"}`,
      }
      return
    }
    if (file) {
      detected = { mode: "file", summary: file.name }
      return
    }
    const t = setTimeout(() => (detected = detect(text)), text.trim() ? 250 : 0)
    return () => clearTimeout(t)
  })
  // Clearing the field hands the choice back to detection
  function oninput() {
    if (!input.trim() && !file && !pages.length) manual = null
  }

  const mode = $derived<AddMode>(manual ?? detected.mode)
  const modeLabel = $derived(MODES.find((m) => m.mode === mode)!.label)
  const linkCount = $derived(input.trim().split(/\s+/).filter((t) => URL_RE.test(t)).length)
  const summary = $derived.by(() => {
    if (saving && mode === "photo") return progress
    if (saving) return mode === "link" ? "Fetching… tricky sites take ~20s" : "Reading…"
    if (mode === "photo")
      return pages.length ? detected.summary : `Up to ${MAX_PHOTOS} pages of one recipe`
    if (mode === "claude") return "Opens the connector setup"
    if (mode === "apps") return "Opens the importer"
    if (mode === "file") return file ? file.name : "Choose a file"
    return manual && manual !== detected.mode ? "" : detected.summary
  })
  // What Add would do right now (the chip lags by the debounce; this doesn't)
  const now = $derived<AddMode>(
    manual ?? (pages.length ? "photo" : file ? "file" : detect(input).mode),
  )
  const canAdd = $derived(
    !saving &&
      (now === "claude" ||
        now === "apps" ||
        now === "file" ||
        (now === "photo" && pages.every((p) => p.blob)) ||
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
      pickTip()
    }
    if (autofocus) (tab === "add" ? addField : searchField)?.focus()
    const close = (e: PointerEvent) => {
      if (menuOpen && root && !root.contains(e.target as Node)) menuOpen = false
    }
    addEventListener("pointerdown", close)
    return () => removeEventListener("pointerdown", close)
  })

  async function switchTab(next: "search" | "add") {
    if (next === "add" && tab !== "add") pickTip()
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
    if (m !== "photo") clearPages()
    if (m === "file") picker?.click()
    else if (m === "photo") {
      void checkVision()
      if (!pages.length) photoPicker?.click()
    } else addField?.focus()
  }

  // ─── Photos ───
  async function checkVision() {
    vision ??= await visionAvailable()
  }

  function clearPages() {
    for (const p of pages) if (p.url) URL.revokeObjectURL(p.url)
    pages = []
  }

  /**
   * Pasted or dropped photos only take over an empty field: text already there stays what it
   * is, unless Photo was picked by hand (then it's the note).
   */
  function photosWelcome(): boolean {
    if (saving) return false
    if (!input.trim() || manual === "photo") return true
    toast({
      title: "Clear the text to add a photo",
      description: "Or pick Photo from the menu to keep it as a note.",
    })
    return false
  }

  /** Adds photos as pages (up to six), shrinking each for the thumbnail and the upload. */
  function addPhotos(files: File[]) {
    if (saving) return
    const room = MAX_PHOTOS - pages.length
    if (files.length > room)
      toast({ title: `Up to ${MAX_PHOTOS} photos`, description: "They should all be one recipe." })
    const added = files.slice(0, Math.max(0, room))
    if (!added.length) return
    manual = "photo"
    file = null
    void checkVision()
    for (const f of added) {
      const key = ++pageKey
      pages.push({ key, name: f.name, blob: null, url: null })
      void shrink(f).then(({ blob, decoded }) => {
        const page = pages.find((p) => p.key === key)
        if (!page) return
        page.blob = blob
        page.url = decoded ? URL.createObjectURL(blob) : null
      })
    }
  }

  function removePage(i: number) {
    const [gone] = pages.splice(i, 1)
    if (gone?.url) URL.revokeObjectURL(gone.url)
    if (!pages.length && !input.trim()) manual = "photo"
  }

  function movePage(i: number, by: -1 | 1) {
    const j = i + by
    if (j < 0 || j >= pages.length) return
    ;[pages[i], pages[j]] = [pages[j]!, pages[i]!]
  }

  async function readPhotos() {
    saving = true
    progress = ""
    try {
      const res = await importPhotos(
        pages.map((p) => p.blob!),
        { note: vision ? input : "", status: (s) => (progress = s) },
      )
      if (res.onDevice) {
        flash({
          title: "Check the amounts",
          description: "Photos are read on this device, so a few numbers or words may be off.",
        })
        location.href = `/recipes/${res.id}/edit`
        return
      }
      if (!res.isNew) flash({ title: "Already in your recipes" })
      location.href = `/recipes/${res.id}`
    } catch (err) {
      toast({ title: "Couldn't read that photo", description: errorMessage(err), tone: "error" })
      saving = false
    }
  }

  function onPaste(e: ClipboardEvent) {
    // Rich text (a web page, a document) often carries a picture along; the text is the point
    if (e.clipboardData?.getData("text/plain").trim()) return
    const images = [...(e.clipboardData?.files ?? [])].filter(isPhoto)
    if (!images.length) return
    e.preventDefault()
    if (photosWelcome()) addPhotos(images)
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
      const res = await api<{ id: number; isNew: boolean; cookbook?: BookImported }>(
        "/api/recipes/import",
        { method: "POST", body },
      )
      // Another Crumb's shared cookbook: every recipe in it, into a cookbook of that name
      if (res.cookbook) {
        flash({
          title: bookImportedTitle(res.cookbook),
          tone: res.cookbook.added ? "success" : undefined,
        })
        location.href = `/cookbooks/${res.cookbook.id}`
        return
      }
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
      case "photo":
        if (!pages.length) {
          photoPicker?.click()
          return
        }
        return readPhotos()
      case "file":
        if (!file) {
          picker?.click()
          return
        }
        if (/^(video|audio)\//.test(file.type) || /\.(docx?|pages|rtf)$/i.test(file.name)) {
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

  /** Photos become pages; any other file is imported on its own. */
  function takeFiles(list: FileList | null | undefined) {
    if (saving) return
    const files = [...(list ?? [])]
    const images = files.filter(isPhoto)
    if (images.length) {
      if (photosWelcome()) addPhotos(images)
      return
    }
    if (files[0]) {
      clearPages()
      file = files[0]
      manual = "file"
    }
  }

  function onDrop(e: DragEvent) {
    if (!e.dataTransfer?.files?.length) return
    e.preventDefault()
    // Mid-read, a drop would change the pages being sent
    if (saving) return
    takeFiles(e.dataTransfer.files)
  }

  // Photos read on this device have no use for a note, so the field steps aside, but only
  // while it's empty and unfocused: text someone typed stays in view
  const photoNote = $derived(mode === "photo" && pages.length > 0)
  const hideField = $derived(photoNote && vision === false && !input.trim() && !fieldFocused)
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

  {#if tip}
    <div class="tip">
      <p class="tip-text hand">
        <span class="tip-long">{tip.long}</span><span class="tip-short">{tip.short}</span>
      </p>
      <!-- Desktop: a loopy arrow from under the tip, down and left to the end of the field -->
      <svg class="arrow arrow-side" viewBox="0 0 100 64" aria-hidden="true">
        <path
          class="stroke"
          pathLength="1"
          d="M90.5 5.5C89 21 79.5 31.5 65 33 51 34.5 44.5 21 54 16.5 63.5 12 67 29.5 52.5 40 40.5 48.5 22 52.5 5 52"
        />
        <path class="head" pathLength="1" d="M15.5 43.5Q9.5 48 4.5 52.2 10.5 54.5 16 59.5" />
      </svg>
      <!-- Tab row: a short curl from the end of the tip down to the field -->
      <svg class="arrow arrow-down" viewBox="0 0 28 36" aria-hidden="true">
        <path class="stroke" pathLength="1" d="M2.5 13.5C10 8 20.5 9.5 21.5 18.5 22 24.5 19.5 29 18.5 33.5" />
        <path class="head" pathLength="1" d="M13 28.5Q16 31.5 18.3 34.5 21 31.2 24 28" />
      </svg>
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
      {#if mode === "photo" && pages.length}
        <ol class="tray" aria-label="Photos, in page order">
          {#each pages as p, i (p.key)}
            <li class="page">
              <div class="thumb">
                {#if p.url}
                  <img src={p.url} alt={`Page ${i + 1}`} />
                {:else if p.blob}
                  <span class="thumb-note"><ImageOff class="size-5" />{p.name}</span>
                {:else}
                  <LoaderCircle class="text-primary size-5 animate-spin" />
                {/if}
                <span class="page-no" aria-hidden="true">{i + 1}</span>
              </div>
              <div class="page-tools">
                {#if pages.length > 1}
                  <button
                    type="button"
                    class="page-btn"
                    aria-label={i ? `Move page ${i + 1} earlier` : "Move page 1 later"}
                    disabled={saving}
                    onclick={() => movePage(i, i ? -1 : 1)}
                  >
                    {#if i}<ChevronLeft class="size-5" />{:else}<ChevronRight class="size-5" />{/if}
                  </button>
                {/if}
                <button
                  type="button"
                  class="page-btn"
                  aria-label={`Remove page ${i + 1}`}
                  disabled={saving}
                  onclick={() => removePage(i)}
                >
                  <X class="size-5" />
                </button>
              </div>
            </li>
          {/each}
          {#if pages.length < MAX_PHOTOS}
            <li class="page">
              <button
                type="button"
                class="add-page"
                disabled={saving}
                onclick={() => photoPicker?.click()}
              >
                <ImagePlus class="size-6" />
                Add another page
              </button>
            </li>
          {/if}
        </ol>
      {/if}
      <textarea
        bind:this={addField}
        bind:value={input}
        use:autosize
        {oninput}
        onkeydown={onAddKey}
        onpaste={onPaste}
        onfocus={() => (fieldFocused = true)}
        onblur={() => (fieldFocused = false)}
        rows="1"
        style:display={hideField ? "none" : undefined}
        placeholder={!photoNote
          ? "Paste a link, a recipe, or type a name"
          : vision === false
            ? "Photos are read on this device, so a note isn't used"
            : "Add a note for Wee Chef, like which recipe on the page (optional)"}
        aria-label={!photoNote
          ? "Recipe link, text or name"
          : vision === false
            ? "Note (not used: photos are read on this device)"
            : "A note for Wee Chef about the photos"}
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
        multiple
        accept=".pdf,.txt,.md,.markdown,.html,.htm,.json,application/pdf,text/plain,text/markdown,text/html,application/json,image/*"
        onchange={(e) => {
          takeFiles(e.currentTarget.files)
          e.currentTarget.value = ""
        }}
      />
      <!-- No capture attribute: phones offer the camera and the photo library both -->
      <input
        bind:this={photoPicker}
        type="file"
        class="hidden"
        multiple
        accept="image/*"
        onchange={(e) => {
          addPhotos([...(e.currentTarget.files ?? [])])
          e.currentTarget.value = ""
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
    /* The focus ring, solid so the tab's and the field's rings can meet without doubling up */
    --ring: color-mix(in srgb, var(--butter) 80%, var(--tile-base, var(--tile)));
    --foot: var(--radius-ctl);
    container: topbox / inline-size;
    position: relative;
    text-align: left;
  }
  /* Inset past the field's corner and a foot, so the field keeps all four rounded corners */
  .tabs {
    display: flex;
    gap: var(--foot);
    padding-left: calc(var(--radius) + var(--foot));
  }
  .tab {
    position: relative;
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
  /* 1px into the field, so no hairline of tile shows through the join */
  .tab[aria-selected="true"] {
    height: calc(2.75rem + 1px);
    margin-bottom: -1px;
    background: var(--paper);
    color: var(--text);
    font-weight: 700;
  }
  .tab:focus-visible {
    outline-color: var(--butter);
  }
  /* Folder-tab feet: inverted corners where the active tab meets the field */
  .tab[aria-selected="true"]::before,
  .tab[aria-selected="true"]::after {
    --cut: transparent var(--foot), var(--paper) calc(var(--foot) + 0.5px);
    content: "";
    position: absolute;
    bottom: 0;
    width: var(--foot);
    height: var(--foot);
    pointer-events: none;
  }
  .tab[aria-selected="true"]::before {
    right: 100%;
    background: radial-gradient(circle at 0 0, var(--cut));
  }
  .tab[aria-selected="true"]::after {
    left: 100%;
    background: radial-gradient(circle at 100% 0, var(--cut));
  }
  /* With the field focused, the ring runs up and around the active tab and its feet too */
  .topbox:has(.search:focus-within, .add:focus-within) .tab[aria-selected="true"] {
    box-shadow: 0 0 0 3px var(--ring);
    clip-path: inset(-3px calc(-1 * var(--foot)) 0);
  }
  .topbox:has(.search:focus-within, .add:focus-within) .tab[aria-selected="true"]::before,
  .topbox:has(.search:focus-within, .add:focus-within) .tab[aria-selected="true"]::after {
    --cut:
      transparent calc(var(--foot) - 3px), var(--ring) calc(var(--foot) - 2.5px),
      var(--ring) var(--foot), var(--paper) calc(var(--foot) + 0.5px);
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
    box-shadow: 0 0 0 3px var(--ring);
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
    padding: 0 0.625rem 0 0.875rem;
    border-radius: var(--radius-ctl);
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

  /* ── Photo pages ─────────────────────────────────────── */
  .tray {
    display: flex;
    gap: 0.625rem;
    padding: 0.75rem 0.75rem 0.25rem;
    overflow-x: auto;
    list-style: none;
    scroll-snap-type: x proximity;
    scroll-padding-inline: 0.75rem;
  }
  .page {
    flex: none;
    width: 5.5rem;
    scroll-snap-align: start;
  }
  .thumb {
    position: relative;
    display: grid;
    place-items: center;
    height: 7rem;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-ctl);
    background: var(--tint);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb-note {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    width: 100%;
    padding: 0 0.375rem;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 0.8125rem;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .page-no {
    position: absolute;
    top: 0.375rem;
    left: 0.375rem;
    display: grid;
    place-items: center;
    min-width: 1.5rem;
    height: 1.5rem;
    padding: 0 0.375rem;
    border-radius: 999px;
    background: var(--tile);
    color: var(--on-tile);
    font-size: 0.8125rem;
    font-weight: 700;
  }
  .page-tools {
    display: flex;
    justify-content: space-between;
  }
  .page-btn {
    display: grid;
    place-items: center;
    width: 2.75rem;
    height: 2.75rem;
    border-radius: 999px;
    color: var(--text-muted);
  }
  .page-tools .page-btn:only-child {
    margin-left: auto;
  }
  .page-btn:hover:not(:disabled) {
    background: var(--tint);
    color: var(--text);
  }
  .add-page {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    width: 100%;
    height: 7rem;
    padding: 0 0.5rem;
    border: 2px dashed var(--border-accented);
    border-radius: var(--radius-ctl);
    color: var(--primary);
    font-size: 0.8125rem;
    font-weight: 700;
    line-height: 1.2;
    text-align: center;
  }
  .add-page:hover:not(:disabled) {
    border-color: var(--tile);
    background: var(--tint);
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

  /* ── The handwritten tip beside Add ─────────────────────── */
  /* Narrow screens: right-aligned in the tab row, a short curl down to the field */
  .tip {
    position: absolute;
    top: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 2.75rem;
    padding-right: 2.125rem;
    color: color-mix(in srgb, var(--on-tile) 88%, transparent);
    pointer-events: none;
  }
  .tip-text {
    font-size: 1.125rem;
    line-height: 0.95;
    text-align: right;
    white-space: pre-line;
    animation: tip-in 0.4s ease-out 0.1s backwards;
  }
  .tip-long,
  .arrow-side {
    display: none;
  }
  .arrow {
    position: absolute;
    overflow: visible;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.5px;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .arrow path {
    stroke-dasharray: 1;
    animation: draw 0.5s ease-in-out 0.15s backwards;
  }
  .arrow .head {
    animation-duration: 0.18s;
    animation-delay: 0.6s;
  }
  .arrow-down {
    top: 0.375rem;
    right: 0.125rem;
    width: 1.75rem;
    height: 2.25rem;
  }
  /* Phones: slimmer tabs leave room for the tip */
  @container topbox (max-width: 26rem) {
    .tab {
      padding: 0 0.75rem;
    }
  }
  @container topbox (max-width: 21rem) {
    .tab {
      padding: 0 0.625rem;
    }
    .tip {
      padding-right: 1.875rem;
    }
    .tip-text {
      font-size: 1rem;
    }
  }
  /* A wider tab row fits the tip on one line, then the whole of it */
  @container topbox (min-width: 26rem) {
    .tip-text {
      font-size: 1.25rem;
      white-space: normal;
    }
  }
  @container topbox (min-width: 36rem) {
    .tip-long {
      display: inline;
    }
    .tip-short {
      display: none;
    }
  }
  /* Wide screens: out to the right of the field, as if scribbled on the tile */
  /* (From 78rem the nav rail, padding and a 40rem field leave about 17rem to the right) */
  @media (min-width: 78rem) {
    .tip {
      top: -0.25rem;
      left: calc(100% + 0.5rem);
      right: auto;
      display: block;
      width: 17rem;
      height: auto;
      padding: 0;
    }
    .tip-text {
      max-width: 11rem;
      margin-left: 6rem;
      font-size: 1.625rem;
      text-align: left;
      white-space: normal;
    }
    .arrow-side {
      display: block;
      top: 1.5rem;
      left: 0;
      width: 6.25rem;
      height: 4rem;
    }
    .tip-long {
      display: inline;
    }
    .tip-short,
    .arrow-down {
      display: none;
    }
  }
  @keyframes draw {
    from {
      stroke-dashoffset: 1;
    }
  }
  @keyframes tip-in {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .tip-text,
    .arrow path {
      animation: none;
    }
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
