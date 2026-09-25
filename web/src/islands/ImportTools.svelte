<script lang="ts">
  import BookmarkCheck from "@lucide/svelte/icons/bookmark-check"
  import CircleCheck from "@lucide/svelte/icons/circle-check"
  import CircleX from "@lucide/svelte/icons/circle-x"
  import Clock from "@lucide/svelte/icons/clock"
  import Download from "@lucide/svelte/icons/download"
  import FileUp from "@lucide/svelte/icons/file-up"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import { api, errorMessage } from "../lib/api"
  import { autosize } from "../lib/autosize"
  import { toast } from "../lib/toast"

  // ─── Links ───
  type LinkState = "waiting" | "working" | "saved" | "duplicate" | "failed"
  interface LinkJob {
    url: string
    state: LinkState
    id?: number
    title?: string
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
          const res = await api<{ id: number; title: string; isNew: boolean }>(
            "/api/recipes/import",
            { method: "POST", body: { url: job.url } },
          )
          Object.assign(job, { state: res.isNew ? "saved" : "duplicate", id: res.id, title: res.title })
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
  const ACCEPT = ".pdf,.paprikarecipes,.paprikarecipe,.json,.html,.htm,.txt,.md,.zip"

  async function uploadFiles(list: FileList | null | undefined) {
    for (const file of list ?? []) {
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

<!-- Links -->
<section>
  <h2 class="font-serif text-xl font-semibold">Paste links</h2>
  <p class="text-ink-muted mt-1 text-sm">
    One per line, or any text with links in it. Already-saved recipes are skipped.
  </p>
  <textarea
    use:autosize
    bind:value={linksText}
    rows="5"
    class="input mt-3 max-h-80 font-mono text-sm"
    aria-label="Links to import"
    placeholder={"https://www.justtherecipe.com/?url=https://…\nhttps://cooking.site/recipe/…"}
  ></textarea>
  <div class="mt-3 flex items-center justify-between gap-3">
    <span class="text-ink-muted text-sm">
      {parsedLinks.length} link{parsedLinks.length === 1 ? "" : "s"} found
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
    <ul class="border-line divide-line rounded-ui mt-4 divide-y border">
      {#each jobs as job (job.url)}
        {@const Icon = linkIcon[job.state]}
        <li class="flex items-start gap-3 px-4 py-3 text-sm">
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
              <a href={`/recipes/${job.id}`} class="font-semibold hover:underline">{job.title}</a>
            {/if}
            <p class={["text-ink-muted truncate", job.id && "text-xs"]}>{job.url}</p>
            {#if job.state === "duplicate"}<p class="text-ink-muted text-xs">Already saved</p>{/if}
            {#if job.message}<p class="text-error text-xs">{job.message}</p>{/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<!-- Files -->
<section>
  <h2 class="font-serif text-xl font-semibold">Upload files</h2>
  <p class="text-ink-muted mt-1 text-sm">
    Just the Recipe PDFs · Paprika (<code>.paprikarecipes</code>) · Mealie / schema.org JSON · saved
    web pages · text files (separate recipes with <code>---</code>) · a <code>.zip</code> of any of
    these · a backup from Crumb.
  </p>
  <button
    type="button"
    class={[
      "rounded-ui mt-3 flex w-full flex-col items-center justify-center gap-2 border-2 border-dashed px-6 py-10 transition",
      dragging ? "border-primary bg-primary/5" : "border-line-strong hover:border-primary",
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
    <FileUp class="text-primary size-10" />
    <span class="font-semibold">Drop files here or tap to choose</span>
    <span class="text-ink-muted text-xs">Up to 50 MB each</span>
  </button>
  <input
    bind:this={fileInput}
    type="file"
    multiple
    accept={ACCEPT}
    class="hidden"
    onchange={(e) => uploadFiles(e.currentTarget.files)}
  />
  {#if fileJobs.length}
    <ul class="mt-4 space-y-2">
      {#each fileJobs as job, i (i)}
        <li class="card px-4 py-3 text-sm">
          <div class="flex items-center gap-2">
            {#if job.state === "working"}
              <LoaderCircle class="text-primary size-5 animate-spin" />
            {:else if job.state === "done"}
              <CircleCheck class="text-success size-5" />
            {:else}
              <CircleX class="text-error size-5" />
            {/if}
            <span class="truncate font-semibold">{job.name}</span>
            {#if job.state === "done"}
              <span class="text-ink-muted ml-auto shrink-0">
                {job.created.length} added{#if job.duplicates}, {job.duplicates} already saved{/if}
              </span>
            {/if}
          </div>
          {#if job.message}<p class="text-error mt-1 text-xs">{job.message}</p>{/if}
          {#if job.created.length}
            <div class="mt-2 flex flex-wrap gap-1.5">
              {#each job.created.slice(0, 12) as r (r.id)}
                <a href={`/recipes/${r.id}`} class="chip bg-canvas h-7 text-xs font-medium hover:underline">
                  {r.title}
                </a>
              {/each}
              {#if job.created.length > 12}
                <span class="text-ink-muted px-1 py-1 text-xs">+{job.created.length - 12} more</span>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
