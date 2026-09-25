<script lang="ts">
  /** "What should I cook next?": four recipes worth cooking soon, with Shuffle. */
  import Shuffle from "@lucide/svelte/icons/shuffle"
  import { onMount } from "svelte"
  import GridSkeleton from "./GridSkeleton.svelte"
  import RecipeCard from "./RecipeCard.svelte"
  import { api, errorMessage } from "../lib/api"
  import type { Suggestions } from "../lib/recipe"
  import { read, write } from "../lib/storage"
  import { toast } from "../lib/toast"

  interface Props {
    initial: Suggestions
    /** Recipes in the whole box (Shuffle and Surprise me need a few to choose from). */
    total: number
  }
  let { initial, total }: Props = $props()

  const COUNT = 4
  const KEY = "crumb:suggest"
  interface Saved {
    date: string
    seed: number
    shown: number[]
  }

  const today = new Date().toDateString()
  let data = $state(initial)
  let loading = $state(false)
  let saved: Saved = read<Saved | null>(KEY, null, "session") ?? { date: today, seed: 0, shown: [] }
  if (saved.date !== today) saved = { date: today, seed: 0, shown: [] }

  const ids = () => data.items.map((i) => i.recipe.id)

  async function load(seed: number, exclude: number[]) {
    const params = new URLSearchParams({ limit: String(COUNT), seed: String(seed) })
    if (exclude.length) params.set("exclude", exclude.join(","))
    return api<Suggestions>(`/api/suggestions?${params}`)
  }

  async function shuffle() {
    loading = true
    try {
      const shown = [...new Set([...saved.shown, ...ids()])]
      let next = await load(saved.seed + 1, shown)
      let kept = shown
      // Seen everything: start again from the whole box
      if (next.items.length < 2) {
        next = await load(saved.seed + 1, ids())
        kept = []
      }
      if (next.items.length < 2) {
        toast({ title: "That's everything for now", description: "Cook something and check back." })
        return
      }
      data = next
      saved = { date: today, seed: saved.seed + 1, shown: kept }
      write(KEY, saved, "session")
    } catch (e) {
      toast({ title: "Couldn't shuffle", description: errorMessage(e), tone: "error" })
    } finally {
      loading = false
    }
  }

  onMount(() => {
    // Back on the home page after a shuffle today: show that set again
    if (saved.seed > 0) {
      loading = true
      load(saved.seed, saved.shown)
        .then((d) => (data = d.items.length >= 2 ? d : data))
        .catch(() => {})
        .finally(() => (loading = false))
      return
    }
    // The AI is still writing blurbs: pick them up when they're ready
    if (initial.ai !== "pending") return
    let tries = 0
    const timer = setInterval(async () => {
      tries += 1
      try {
        const d = await load(0, [])
        if (d.ai === "ready" && saved.seed === 0) data = d
        if (d.ai !== "pending" || tries >= 3) clearInterval(timer)
      } catch {
        clearInterval(timer)
      }
    }, 4000)
    return () => clearInterval(timer)
  })
</script>

{#if data.items.length >= 2}
  {#if loading}
    <GridSkeleton count={COUNT} />
  {:else}
    <div class="grid grid-cols-2 gap-x-3 gap-y-6 sm:grid-cols-4 sm:gap-x-4" data-prerender-eager>
      {#each data.items as item (item.recipe.id)}
        <RecipeCard
          recipe={item.recipe}
          reason={item.reason}
          aiReason={item.ai}
          sizes="(min-width: 1200px) 17rem, (min-width: 640px) 24vw, 50vw"
        />
      {/each}
    </div>
  {/if}
  {#if total > COUNT}
    <div class="mt-4 flex justify-center">
      <button type="button" class="btn btn-soft" disabled={loading} onclick={shuffle}>
        <Shuffle /> Shuffle
      </button>
    </div>
  {/if}
{:else}
  <p class="text-ink-muted text-sm">Add a few more recipes and Crumb will start suggesting some.</p>
{/if}
