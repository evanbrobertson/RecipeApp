<script lang="ts">
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import Sparkles from "@lucide/svelte/icons/sparkles"
  import EmptyState from "../components/EmptyState.svelte"
  import Photo from "../components/Photo.svelte"
  import { api } from "../lib/api"
  import { pageState } from "../lib/page.svelte"

  interface Pending {
    id: number
    title: string
    image?: string | null
    count: number
    /** Suggestions per field, e.g. `{ instructions: 2, ingredients: 1 }`. */
    fields: Record<string, number>
  }

  const page = pageState<{ recipes: Pending[] }>(() => api("/api/checks/review"))

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
    <p class="text-ink-muted">Wee Chef spotted a few things worth a look. Nothing changes until you say so.</p>
  </header>

  {#if !page.data}
    <div class="skeleton h-48 w-full"></div>
  {:else if !page.data.recipes.length}
    <EmptyState
      icon={Sparkles}
      title="All caught up"
      description="Nothing waiting. Wee Chef checks new imports as they arrive."
    />
  {:else}
    <div class="list-card">
      {#each page.data.recipes as r (r.id)}
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
