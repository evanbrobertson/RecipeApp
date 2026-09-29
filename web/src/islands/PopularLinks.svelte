<script lang="ts">
  /**
   * Popular (src/popular.rs): recipe links several other households saved that this one
   * hasn't. Links only; each opens the preview, which reads the page like any other link.
   * Nothing shows when the server has no Popular (one household, or `POPULAR=off`), or
   * nothing is popular yet.
   */
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import Flame from "@lucide/svelte/icons/flame"
  import { onMount } from "svelte"
  import { api } from "../lib/api"

  interface Popular {
    enabled: boolean
    items: { url: string; host: string; title: string; households: number }[]
  }

  let items = $state<Popular["items"]>([])

  onMount(async () => {
    try {
      const popular = await api<Popular>("/api/popular")
      if (popular.enabled) items = popular.items.slice(0, 8)
    } catch {
      // Popular is a nice-to-have: the Add page works the same without it
    }
  })
</script>

{#if items.length}
  <section aria-labelledby="popular-title">
    <h2 id="popular-title" class="section-title mb-1">Popular in Crumb</h2>
    <p class="text-ink-muted mb-3.5 text-sm">Saved by several other households.</p>
    <div class="list-card">
      {#each items as item (item.url)}
        <a href={`/preview?url=${encodeURIComponent(item.url)}`} class="list-row" data-no-prerender>
          <span class="well"><Flame /></span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[17px] leading-tight font-bold">{item.title}</span>
            <span class="text-ink-muted block truncate text-sm">
              {item.host} · in {item.households} boxes
            </span>
          </span>
          <ChevronRight class="text-ink-muted size-5 flex-none" />
        </a>
      {/each}
    </div>
  </section>
{/if}
