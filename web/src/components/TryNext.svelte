<script lang="ts">
  /** "Try next" on the home page: a few recipes worth cooking soon, with Shuffle. */
  import Dices from "@lucide/svelte/icons/dices"
  import Shuffle from "@lucide/svelte/icons/shuffle"
  import { onMount } from "svelte"
  import GridSkeleton from "./GridSkeleton.svelte"
  import RecipeCard from "./RecipeCard.svelte"
  import { api, errorMessage } from "../lib/api"
  import { randomHref, wireRandomLinks } from "../lib/random"
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
  let section: HTMLElement | undefined = $state()

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
    if (section) wireRandomLinks(section)
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
  <section bind:this={section}>
    <div class="mb-4 flex items-center justify-between gap-2">
      <h2 class="font-serif text-xl font-semibold">Try next</h2>
      <div class="flex items-center gap-1">
        {#if total > COUNT}
          <button type="button" class="btn btn-link" disabled={loading} onclick={shuffle}>
            <Shuffle class="size-4" /> Shuffle
          </button>
        {/if}
        <a href={randomHref()} class="btn btn-link" data-random data-no-prerender>
          <Dices class="size-4" /> Surprise me
        </a>
      </div>
    </div>
    {#if loading}
      <GridSkeleton count={COUNT} />
    {:else}
      <div
        class="grid grid-cols-2 gap-x-3 gap-y-6 transition-opacity sm:gap-x-5 md:grid-cols-3 lg:grid-cols-4"
      >
        {#each data.items as item (item.recipe.id)}
          <RecipeCard recipe={item.recipe} reason={item.reason} aiReason={item.ai} />
        {/each}
      </div>
    {/if}
  </section>
{/if}
