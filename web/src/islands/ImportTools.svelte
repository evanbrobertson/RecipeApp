<script lang="ts">
  import BookmarkCheck from "@lucide/svelte/icons/bookmark-check"
  import CircleCheck from "@lucide/svelte/icons/circle-check"
  import CircleX from "@lucide/svelte/icons/circle-x"
  import Clock from "@lucide/svelte/icons/clock"
  import Download from "@lucide/svelte/icons/download"
  import FileUp from "@lucide/svelte/icons/file-up"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { api, errorMessage } from "../lib/api"
  import { bookImportedTitle } from "../lib/format"
  import type { BookImported } from "../lib/recipe"
  import { importPhotos, isPhoto, MAX_PHOTOS, shrink } from "../lib/photos"
  import { autosize } from "../lib/autosize"
  import { toast } from "../lib/toast"

  // ─── Links ───
  type LinkState = "waiting" | "working" | "saved" | "duplicate" | "failed"
  interface LinkJob {
    url: string
    state: LinkState
    id?: number
    title?: string
    /** Another Crumb's shared cookbook: where its recipes went. */
    href?: string
    message?: string
  }

  let linksText = $state(new URLSearchParams(location.search).get("links") ?? "")
  let jobs = $state<LinkJob[]>([])
  let running = $state(false)

  const parsedLinks = $derived([
    ...new Set(
      linksText.match(/https?:\/\/[^\s<>"']+/gi)?.map((u) => u.replace(/[),.;]+$/, "")) ?? [],
    ),
  ])

  async function runLinks() {
    if (!parsedLinks.length || running) return
    running = true
    jobs = parsedLinks.map((url) => ({ url, state: "waiting" }))
    // Two at a time: fast enough, and gentle on the server's headless browser
    let cursor = 0
    async function worker() {
      while (cursor < jobs.length) {
        const job = jobs[cursor++]!
        job.state = "working"
        try {
          const res = await api<{
            id: number
            title: string
            isNew: boolean
            cookbook?: BookImported
          }>("/api/recipes/import", { method: "POST", body: { url: job.url } })
          if (res.cookbook) {
            Object.assign(job, {
              state: res.cookbook.added ? "saved" : "duplicate",
              id: res.cookbook.id,
              title: bookImportedTitle(res.cookbook),
              href: `/cookbooks/${res.cookbook.id}`,
            })
          } else {
            Object.assign(job, {
              state: res.isNew ? "saved" : "duplicate",
              id: res.id,
              title: res.title,
              href: `/recipes/${res.id}`,
            })
          }
        } catch (e) {
          Object.assign(job, { state: "failed", message: errorMessage(e) })
        }
      }
    }
    await Promise.all([worker(), worker()])
    running = false
    const saved = jobs.filter((j) => j.state === "saved").length
    toast({ title: `Imported ${saved} of ${jobs.length} links`, tone: "success" })
  }

  const linkIcon = {
    waiting: Clock,
    working: LoaderCircle,
    saved: CircleCheck,
    duplicate: BookmarkCheck,
    failed: CircleX,
  }

  // ─── Files ───
  interface FileJob {
    name: string
    state: "working" | "done" | "failed"
    created: { id: number; title: string }[]
    duplicates: number
    message?: string
    /** Open the editor: the recipe was read by OCR and needs checking. */
    edit?: boolean
  }
  interface ImportSummary {
    file: string
    created: { id: number; title: string }[]
    duplicates: number
    error?: string
  }

  let fileJobs = $state<FileJob[]>([])
  let dragging = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()
  const ACCEPT = ".pdf,.paprikarecipes,.paprikarecipe,.json,.html,.htm,.txt,.md,.zip,image/*"

  /** Photos chosen together are the pages of one recipe (up to six). */
  async function uploadPhotos(photos: File[]) {
    const n = Math.min(photos.length, MAX_PHOTOS)
    fileJobs.unshift({
      name: n === 1 ? photos[0]!.name : `${n} photos`,
      state: "working",
      created: [],
      duplicates: 0,
    })
    const job = fileJobs[0]!
    try {
      if (photos.length > MAX_PHOTOS)
        throw new Error(`Up to ${MAX_PHOTOS} photos at a time, all pages of one recipe`)
      const pages = await Promise.all(photos.map(async (p) => (await shrink(p)).blob))
      const res = await importPhotos(pages, { status: (s) => (job.message = s) })
      Object.assign(job, {
        state: "done",
        created: res.isNew ? [{ id: res.id, title: res.title }] : [],
        duplicates: res.isNew ? 0 : 1,
        message: res.onDevice ? "Read on this device: check the amounts" : undefined,
        edit: res.onDevice,
      })
    } catch (e) {
      Object.assign(job, { state: "failed", message: errorMessage(e) })
    }
  }

  async function uploadFiles(list: FileList | null | undefined) {
    const all = [...(list ?? [])]
    const photos = all.filter(isPhoto)
    if (photos.length) await uploadPhotos(photos)
    for (const file of all.filter((f) => !isPhoto(f))) {
      fileJobs.unshift({ name: file.name, state: "working", created: [], duplicates: 0 })
      const job = fileJobs[0]!
      try {
        const form = new FormData()
        form.append("file", file)
        const [result] = await api<ImportSummary[]>("/api/import/files", { method: "POST", form })
        if (!result || result.error)
          Object.assign(job, { state: "failed", message: result?.error ?? "Nothing imported" })
        else Object.assign(job, { state: "done", created: result.created, duplicates: result.duplicates })
      } catch (e) {
        Object.assign(job, { state: "failed", message: errorMessage(e) })
      }
    }
  }
</script>

<div class="space-y-8">
<!-- Links -->
<section class="min-w-0">
  <h2 class="settings-heading" id="links-title">Links</h2>
  <textarea
    use:autosize
    bind:value={linksText}
    rows="5"
    class="input max-h-80 font-mono text-sm"
    aria-labelledby="links-title"
    placeholder={"https://…\nhttps://…"}
  ></textarea>
  <div class="mt-3 flex items-center justify-between gap-3">
    <span class="meta">
      {parsedLinks.length
        ? `${parsedLinks.length} link${parsedLinks.length === 1 ? "" : "s"} found`
        : "One per line, or any text with links in it"}
    </span>
    <button
      type="button"
      class="btn btn-primary"
      disabled={!parsedLinks.length || running}
      onclick={runLinks}
    >
      {#if running}<LoaderCircle class="animate-spin" /> Importing…{:else}<Download /> Import links{/if}
    </button>
  </div>
  {#if jobs.length}
    <ul class="list-card mt-4">
      {#each jobs as job (job.url)}
        {@const Icon = linkIcon[job.state]}
        <li class="list-row items-start text-sm">
          <Icon
            class={[
              "mt-0.5 size-5 shrink-0",
              job.state === "working" && "text-primary animate-spin",
              job.state === "saved" && "text-success",
              (job.state === "waiting" || job.state === "duplicate") && "text-ink-muted",
              job.state === "failed" && "text-error",
            ]}
          />
          <div class="min-w-0 flex-1">
            {#if job.id}
              <a href={job.href} class="block text-[15px] font-bold hover:underline"
                >{job.title}</a
              >
            {/if}
            <p class={["text-ink-muted truncate", job.id && "meta font-normal"]}>{job.url}</p>
            {#if job.state === "duplicate" && !job.href?.startsWith("/cookbooks/")}
              <p class="meta">Already saved</p>
            {/if}
            {#if job.message}<p class="text-error text-[13px]">{job.message}</p>{/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<!-- Files -->
<section class="min-w-0">
  <h2 class="settings-heading">Files</h2>
  <button
    type="button"
    class={[
      "rounded-ui bg-paper flex w-full flex-col items-center justify-center gap-1.5 border-2 border-dashed px-6 py-8 transition",
      dragging ? "border-tile bg-tint" : "border-line-strong hover:border-tile",
    ]}
    onclick={() => fileInput?.click()}
    ondragover={(e) => {
      e.preventDefault()
      dragging = true
    }}
    ondragleave={() => (dragging = false)}
    ondrop={(e) => {
      e.preventDefault()
      dragging = false
      void uploadFiles(e.dataTransfer?.files)
    }}
  >
    <FileUp class="settings-icon mb-1 size-6" />
    <span class="font-bold">Drop files or tap to choose</span>
    <span class="meta"
      >PDF, Paprika, Mealie JSON, web pages, text, .zip, a Crumb backup, or photos of a recipe</span
    >
  </button>
  <input
    bind:this={fileInput}
    type="file"
    multiple
    accept={ACCEPT}
    class="hidden"
    onchange={(e) => uploadFiles(e.currentTarget.files)}
  />
  <p class="hint mt-2 px-4">
    Up to 50 MB each. In text files, put <code>---</code> between recipes. Photos chosen together
    are read as the pages of one recipe (up to {MAX_PHOTOS}).
  </p>
  {#if fileJobs.length}
    <ul class="mt-4 space-y-2">
      {#each fileJobs as job, i (i)}
        <li class="card p-3 text-sm">
          <div class="flex items-center gap-2">
            {#if job.state === "working"}
              <LoaderCircle class="text-primary size-5 animate-spin" />
            {:else if job.state === "done"}
              <CircleCheck class="text-success size-5" />
            {:else}
              <CircleX class="text-error size-5" />
            {/if}
            <span class="truncate text-[15px] font-bold">{job.name}</span>
            {#if job.state === "done"}
              <span class="text-ink-muted ml-auto shrink-0">
                {job.created.length} added{#if job.duplicates}, {job.duplicates} already saved{/if}
              </span>
            {/if}
          </div>
          {#if job.message}
            <p class={["mt-1 text-[13px]", job.state === "failed" ? "text-error" : "text-ink-muted"]}>
              {job.message}
            </p>
          {/if}
          {#if job.created.length}
            <div class="mt-2.5 flex flex-wrap gap-2">
              {#each job.created.slice(0, 12) as r (r.id)}
                <a href={`/recipes/${r.id}${job.edit ? "/edit" : ""}`} class="chip bg-tint text-primary h-11 hover:underline">
                  {r.title}
                </a>
              {/each}
              {#if job.created.length > 12}
                <span class="meta self-center px-1">+{job.created.length - 12} more</span>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
</div>
