<script lang="ts">
  /**
   * A preview's video (/preview?url=…, src/preview.rs), from its page data: the recipe
   * page's player, so nothing loads from the video's site until Play.
   */
  import RecipeVideo from "../components/RecipeVideo.svelte"
  import { inlineData } from "../lib/api"
  import type { VideoEmbed } from "../lib/recipe"

  interface Data {
    preview?: { title?: string; video?: string | null; videoEmbed?: VideoEmbed | null }
  }

  const preview = inlineData<Data>()?.preview
</script>

{#if preview?.video}
  <div class="share-video">
    <RecipeVideo video={preview.video} embed={preview.videoEmbed} title={preview.title ?? ""} />
  </div>
{/if}
