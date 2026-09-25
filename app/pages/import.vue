<script setup lang="ts">
useHead({ title: "Import · Just the Recipe" })

const route = useRoute()
const toast = useToast()

// ─── Links ───
type LinkState = "waiting" | "working" | "saved" | "duplicate" | "failed"
interface LinkJob {
  url: string
  state: LinkState
  id?: number
  title?: string
  message?: string
}

const linksText = ref(typeof route.query.links === "string" ? route.query.links : "")
const jobs = ref<LinkJob[]>([])
const running = ref(false)

const parsedLinks = computed(() => [
  ...new Set(
    linksText.value.match(/https?:\/\/[^\s<>"']+/gi)?.map((u) => u.replace(/[),.;]+$/, "")) ?? [],
  ),
])

async function runLinks() {
  if (!parsedLinks.value.length || running.value) return
  running.value = true
  jobs.value = parsedLinks.value.map((url) => ({ url, state: "waiting" }))
  // Two at a time: fast enough, and gentle on the server's headless browser
  let cursor = 0
  async function worker() {
    while (cursor < jobs.value.length) {
      const job = jobs.value[cursor++]!
      job.state = "working"
      try {
        const res = await $fetch("/api/recipes/import", { method: "POST", body: { url: job.url } })
        Object.assign(job, {
          state: res.isNew ? "saved" : "duplicate",
          id: res.id,
          title: res.title,
        })
      } catch (e) {
        Object.assign(job, { state: "failed", message: errorMessage(e) })
      }
    }
  }
  await Promise.all([worker(), worker()])
  running.value = false
  const saved = jobs.value.filter((j) => j.state === "saved").length
  toast.add({ title: `Imported ${saved} of ${jobs.value.length} links`, icon: "i-lucide-check" })
}

const linkIcon: Record<LinkState, string> = {
  waiting: "i-lucide-clock",
  working: "i-lucide-loader-circle",
  saved: "i-lucide-circle-check",
  duplicate: "i-lucide-bookmark-check",
  failed: "i-lucide-circle-x",
}

// ─── Files ───
interface FileJob {
  name: string
  state: "working" | "done" | "failed"
  created: { id: number; title: string }[]
  duplicates: number
  message?: string
}
const fileJobs = ref<FileJob[]>([])
const dragging = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)
const ACCEPT = ".pdf,.paprikarecipes,.paprikarecipe,.json,.html,.htm,.txt,.md,.zip"

async function uploadFiles(list: FileList | File[] | null) {
  const files = [...(list ?? [])]
  for (const file of files) {
    const job = reactive<FileJob>({ name: file.name, state: "working", created: [], duplicates: 0 })
    fileJobs.value.unshift(job)
    try {
      const form = new FormData()
      form.append("file", file)
      const [result] = await $fetch("/api/import/files", { method: "POST", body: form })
      if (!result || result.error)
        Object.assign(job, { state: "failed", message: result?.error ?? "Nothing imported" })
      else
        Object.assign(job, {
          state: "done",
          created: result.created,
          duplicates: result.duplicates,
        })
    } catch (e) {
      Object.assign(job, { state: "failed", message: errorMessage(e) })
    }
  }
}

function onDrop(e: DragEvent) {
  dragging.value = false
  void uploadFiles(e.dataTransfer?.files ?? null)
}
</script>

<template>
  <div class="mx-auto max-w-3xl space-y-10">
    <div>
      <h1 class="font-serif text-3xl font-semibold sm:text-4xl">Bring your recipes over</h1>
      <p class="text-muted mt-1">
        From Just the Recipe, Paprika, Mealie, or a folder of saved pages.
      </p>
    </div>

    <!-- Just the Recipe guide -->
    <section class="rounded-3xl bg-amber-50 p-5 sm:p-6 dark:bg-amber-950/25">
      <div class="flex items-center gap-2">
        <UIcon name="i-lucide-info" class="size-5 text-amber-700 dark:text-amber-300" />
        <h2 class="font-semibold text-amber-900 dark:text-amber-200">
          Moving from Just the Recipe
        </h2>
      </div>
      <p class="mt-2 text-sm text-amber-900/80 dark:text-amber-100/80">
        Just the Recipe doesn't have a “download everything” button, so pick whichever of these is
        easiest:
      </p>
      <ol class="mt-3 space-y-2 text-sm text-amber-950/90 dark:text-amber-50/90">
        <li class="flex gap-2">
          <span class="font-bold">1.</span>
          <span>
            <strong>Links (best):</strong> open each saved recipe, tap <em>Share</em> and copy the
            link (or open the original site and copy its address), and paste them all below. Share
            links are unwrapped to the original page automatically.
          </span>
        </li>
        <li class="flex gap-2">
          <span class="font-bold">2.</span>
          <span>
            <strong>PDFs:</strong> with Premium, use <em>Print → Save as PDF</em> on each recipe and
            drop the PDFs in the box further down. Handy for recipes you edited or typed in
            yourself.
          </span>
        </li>
        <li class="flex gap-2">
          <span class="font-bold">3.</span>
          <span>
            <strong>Text:</strong> copy a recipe's text and paste it on the
            <NuxtLink to="/add" class="font-semibold underline">Add</NuxtLink> page.
          </span>
        </li>
      </ol>
    </section>

    <!-- Links -->
    <section>
      <h2 class="font-serif text-xl font-semibold">Paste links</h2>
      <p class="text-muted mt-1 text-sm">
        One per line, or any text with links in it. Already-saved recipes are skipped.
      </p>
      <UTextarea
        v-model="linksText"
        :rows="5"
        autoresize
        :maxrows="12"
        class="mt-3 w-full"
        :ui="{ base: 'rounded-xl px-4 py-3 font-mono text-sm' }"
        placeholder="https://www.justtherecipe.com/?url=https://…&#10;https://cooking.site/recipe/…"
      />
      <div class="mt-3 flex items-center justify-between gap-3">
        <span class="text-muted text-sm"
          >{{ parsedLinks.length }} link{{ parsedLinks.length === 1 ? "" : "s" }} found</span
        >
        <UButton
          :label="running ? 'Importing…' : 'Import links'"
          icon="i-lucide-download"
          class="rounded-full"
          :loading="running"
          :disabled="!parsedLinks.length"
          @click="runLinks"
        />
      </div>
      <ul v-if="jobs.length" class="border-default divide-default mt-4 divide-y rounded-2xl border">
        <li v-for="job in jobs" :key="job.url" class="flex items-start gap-3 px-4 py-3 text-sm">
          <UIcon
            :name="linkIcon[job.state]"
            class="mt-0.5 size-5 shrink-0"
            :class="{
              'text-primary animate-spin': job.state === 'working',
              'text-green-600': job.state === 'saved',
              'text-muted': job.state === 'waiting' || job.state === 'duplicate',
              'text-error': job.state === 'failed',
            }"
          />
          <div class="min-w-0 flex-1">
            <NuxtLink
              v-if="job.id"
              :to="`/recipes/${job.id}`"
              class="font-semibold hover:underline"
              >{{ job.title }}</NuxtLink
            >
            <p class="text-muted truncate" :class="{ 'text-xs': job.id }">{{ job.url }}</p>
            <p v-if="job.state === 'duplicate'" class="text-muted text-xs">Already saved</p>
            <p v-if="job.message" class="text-error text-xs">{{ job.message }}</p>
          </div>
        </li>
      </ul>
    </section>

    <!-- Files -->
    <section>
      <h2 class="font-serif text-xl font-semibold">Upload files</h2>
      <p class="text-muted mt-1 text-sm">
        Just the Recipe PDFs · Paprika (<code>.paprikarecipes</code>) · Mealie / schema.org JSON ·
        saved web pages · text files (separate recipes with <code>---</code>) · a
        <code>.zip</code> of any of these · a backup from this app.
      </p>
      <button
        type="button"
        class="mt-3 flex w-full flex-col items-center justify-center gap-2 rounded-3xl border-2 border-dashed px-6 py-10 transition"
        :class="
          dragging
            ? 'border-primary bg-primary/5'
            : 'hover:border-primary border-(--ui-border-accented)'
        "
        @click="fileInput?.click()"
        @dragover.prevent="
          () => {
            dragging = true
          }
        "
        @dragleave="
          () => {
            dragging = false
          }
        "
        @drop.prevent="onDrop"
      >
        <UIcon name="i-lucide-file-up" class="text-primary size-10" />
        <span class="font-semibold">Drop files here or tap to choose</span>
        <span class="text-muted text-xs">Up to 50 MB each</span>
      </button>
      <input
        ref="fileInput"
        type="file"
        multiple
        :accept="ACCEPT"
        class="hidden"
        @change="(e) => uploadFiles((e.target as HTMLInputElement).files)"
      />
      <ul v-if="fileJobs.length" class="mt-4 space-y-2">
        <li
          v-for="job in fileJobs"
          :key="job.name"
          class="bg-elevated/50 rounded-2xl px-4 py-3 text-sm"
        >
          <div class="flex items-center gap-2">
            <UIcon
              :name="
                job.state === 'working'
                  ? 'i-lucide-loader-circle'
                  : job.state === 'done'
                    ? 'i-lucide-circle-check'
                    : 'i-lucide-circle-x'
              "
              class="size-5"
              :class="{
                'text-primary animate-spin': job.state === 'working',
                'text-green-600': job.state === 'done',
                'text-error': job.state === 'failed',
              }"
            />
            <span class="truncate font-semibold">{{ job.name }}</span>
            <span v-if="job.state === 'done'" class="text-muted ml-auto shrink-0">
              {{ job.created.length }} added<template v-if="job.duplicates"
                >, {{ job.duplicates }} already saved</template
              >
            </span>
          </div>
          <p v-if="job.message" class="text-error mt-1 text-xs">{{ job.message }}</p>
          <div v-if="job.created.length" class="mt-2 flex flex-wrap gap-1.5">
            <NuxtLink
              v-for="r in job.created.slice(0, 12)"
              :key="r.id"
              :to="`/recipes/${r.id}`"
              class="bg-default rounded-full px-2.5 py-1 text-xs hover:underline"
            >
              {{ r.title }}
            </NuxtLink>
            <span v-if="job.created.length > 12" class="text-muted px-1 py-1 text-xs"
              >+{{ job.created.length - 12 }} more</span
            >
          </div>
        </li>
      </ul>
    </section>
  </div>
</template>
