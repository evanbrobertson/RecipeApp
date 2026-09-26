<script lang="ts">
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import ClipboardCheck from "@lucide/svelte/icons/clipboard-check"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import Sparkles from "@lucide/svelte/icons/sparkles"
  import { untrack } from "svelte"
  import EmptyState from "../components/EmptyState.svelte"
  import Photo from "../components/Photo.svelte"
  import { api, errorMessage } from "../lib/api"
  import { checksStatusText } from "../lib/checks"
  import { pageState } from "../lib/page.svelte"
  import type { ChecksStatus } from "../lib/recipe"
  import { toast } from "../lib/toast"

  interface Pending {
    id: number
    title: string
    image?: string | null
    count: number
    /** Suggestions per field, e.g. `{ instructions: 2, ingredients: 1 }`. */
    fields: Record<string, number>
  }

  // `checks` only when Wee Chef checks are set up on the server
  const page = pageState<{ recipes: Pending[]; checks?: ChecksStatus }>(async () => {
    const [list, checks] = await Promise.all([
      api<{ recipes: Pending[] }>("/api/checks/review"),
      api<ChecksStatus>("/api/checks"),
    ])
    return { recipes: list.recipes, checks: checks.enabled ? checks : undefined }
  })

  let recipes = $state<Pending[] | undefined>(page.data?.recipes)
  let checks = $state<ChecksStatus | undefined>(page.data?.checks)
  $effect(() => {
    if (!recipes && page.data) {
      recipes = page.data.recipes
      checks = page.data.checks
    }
  })

  let starting = $state(false)
  const running = $derived(!!checks && checks.pending > 0)
  // The most waiting at once in this run: progress is how many of those are through
  let run = $state(0)
  $effect(() => {
    const pending = checks?.pending ?? 0
    if (pending === 0) run = 0
    else if (pending > untrack(() => run)) run = pending
  })
  const progress = $derived(
    checks && run > 0 ? Math.round(((run - checks.pending) / run) * 100) : 0,
  )

  // Keep the nav's count in step with the list (the head script set it from the page load)
  $effect(() => {
    if (!recipes) return
    const root = document.documentElement
    const n = recipes.length
    if (n > 0) {
      root.dataset.review = ""
      root.style.setProperty("--review-count", JSON.stringify(n > 99 ? "99+" : String(n)))
    } else {
      delete root.dataset.review
    }
  })

  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

  /**
   * While a Check all runs: polls its progress with backoff (quicker again whenever a
   * recipe finishes) and reloads the list as results arrive. Stops once nothing is pending.
   */
  let watching = false
  function watch(delay = 1500) {
    if (watching) return
    watching = true
    setTimeout(() => void poll(delay), delay)
  }
  async function poll(delay: number) {
    const before = checks
    const next = await api<ChecksStatus>("/api/checks").catch(() => undefined)
    if (next) {
      checks = next
      const moved = next.checked !== before?.checked || next.toCheck !== before?.toCheck
      if (moved || next.pending === 0) await refreshList()
      delay = moved ? 1500 : Math.min(delay * 1.5, 8000)
    } else {
      delay = Math.min(delay * 2, 10_000)
    }
    watching = false
    if (checks && checks.pending > 0) watch(delay)
  }

  async function refreshList() {
    const list = await api<{ recipes: Pending[] }>("/api/checks/review").catch(() => undefined)
    if (list) recipes = list.recipes
  }

  // Opened while a Check all is already running (from another tab, or before a reload)
  $effect(() => {
    if (running) untrack(() => watch())
  })

  async function checkAll() {
    if (starting || running || !checks?.due) return
    starting = true
    try {
      checks = await api<ChecksStatus>("/api/checks", { method: "POST" })
      if (!checks.queued) toast({ title: "Every recipe has been checked" })
    } catch (e) {
      toast({ title: "Couldn't start the checks", description: errorMessage(e), tone: "error" })
    } finally {
      starting = false
    }
  }

  const NAMES: Record<string, [string, string]> = {
    ingredients: ["ingredient", "ingredients"],
    instructions: ["step", "steps"],
    notes: ["note", "notes"],
    totalTime: ["time", "times"],
  }

  /** "2 steps · 1 ingredient" */
  function summary(fields: Record<string, number>) {
    return Object.entries(fields)
      .sort((a, b) => b[1] - a[1])
      .map(([field, n]) => {
        const [one, many] = NAMES[field] ?? ["thing", "things"]
        return `${n} ${n === 1 ? one : many}`
      })
      .join(" · ")
  }
</script>

<div class="mx-auto flex max-w-[40rem] flex-col gap-6">
  <header class="flex flex-col gap-1">
    <h1 class="page-title">Suggestions</h1>
    <p class="text-ink-muted">
      Wee Chef reads your recipes and flags anything worth a look. Nothing changes until you say
      so.
    </p>
  </header>

  {#if !recipes}
    <div class="skeleton h-28 w-full"></div>
    <div class="skeleton h-48 w-full"></div>
  {:else}
    {#if checks}
      <section class="card flex flex-col gap-4 p-5 sm:flex-row sm:items-center" aria-label="Wee Chef">
        <div class="flex min-w-0 flex-1 items-start gap-3">
          <span class="bg-tint text-primary rounded-ctl grid size-12 flex-none place-items-center">
            <ChefHat class="size-6" />
          </span>
          <div class="min-w-0 flex-1">
            <p class="font-bold">Wee Chef checks</p>
            <p class="text-ink-muted text-sm" aria-live="polite">{checksStatusText(checks, run)}</p>
            {#if running}
              <div
                class="progress mt-2"
                role="progressbar"
                aria-label="Wee Chef checks"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progress}
              >
                <span style:width={`${progress}%`}></span>
              </div>
            {/if}
          </div>
        </div>
        <button
          type="button"
          class="btn btn-primary w-full flex-none sm:w-auto"
          disabled={running || starting || !checks.due}
          aria-busy={running || starting}
          onclick={checkAll}
        >
          {#if running || starting}
            <LoaderCircle class="animate-spin" />
            Checking…
          {:else}
            <ClipboardCheck />
            Check all with Wee Chef
          {/if}
        </button>
      </section>
    {/if}

    {#if !recipes.length}
      <EmptyState
        icon={Sparkles}
        title="Nothing to review"
        description={checks
          ? "Wee Chef will flag anything it isn't sure about."
          : "Wee Chef checks aren't on for this box."}
      />
    {:else}
      <div class="list-card">
        {#each recipes as r (r.id)}
          <a href={`/recipes/${r.id}/edit`} class="list-row settings-row gap-3">
            <Photo
              recipe={r}
              width={56}
              class="rounded-ctl size-14 shrink-0 object-cover"
              iconClass="size-5"
            />
            <span class="flex min-w-0 flex-1 flex-col">
              <span class="truncate font-bold">{r.title}</span>
              <span class="text-ink-muted text-sm">{summary(r.fields)}</span>
            </span>
            <span class="review-count" aria-label={`${r.count} to check`}>{r.count}</span>
            <ChevronRight class="text-ink-muted size-5 shrink-0" />
          </a>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .review-count {
    display: inline-flex;
    min-width: 1.75rem;
    height: 1.75rem;
    align-items: center;
    justify-content: center;
    border-radius: 9999px;
    padding: 0 0.5rem;
    background: var(--tint);
    color: var(--primary);
    font-size: 13px;
    font-weight: 800;
  }
</style>
