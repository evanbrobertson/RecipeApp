<script lang="ts">
  /**
   * The recipe's video, played in place. Nothing loads from the video's site until Play:
   * a still (YouTube's) or a tile stands in. Once it's playing, scrolling past it floats
   * the player in a corner so the steps can be read along with it. The iframe itself
   * never moves in the page (moving it would reload it); only its styles change.
   */
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import Play from "@lucide/svelte/icons/play"
  import ArrowUp from "@lucide/svelte/icons/arrow-up"
  import X from "@lucide/svelte/icons/x"
  import { hostOf, type VideoEmbed } from "../lib/recipe"

  interface Props {
    /** The recipe's video link. */
    video: string
    embed: VideoEmbed | null | undefined
    title: string
  }

  let { video, embed, title }: Props = $props()

  let playing = $state(false)
  /** The still didn't load (blocked, or taken down): the plain tile instead. */
  let noStill = $state(false)
  /** The player's place in the page is out of view (scrolled past it). */
  let away = $state(false)
  let slot = $state<HTMLElement>()
  const docked = $derived(playing && away)

  // Sites that start playing when asked; the rest wait for a second tap
  const autoplay = (e: VideoEmbed) => {
    if (e.provider === "file" || e.provider === "tiktok" || e.provider === "instagram")
      return e.embedUrl
    return `${e.embedUrl}${e.embedUrl.includes("?") ? "&" : "?"}autoplay=1`
  }

  $effect(() => {
    if (!slot || !playing) return
    const io = new IntersectionObserver(
      ([entry]) => {
        // Only above the viewport: scrolled on to the steps, not yet scrolled down to it
        away = !entry.isIntersecting && entry.boundingClientRect.top < 0
      },
      { threshold: 0.25 },
    )
    io.observe(slot)
    return () => io.disconnect()
  })

  function stop() {
    playing = false
    away = false
  }

  function backToIt() {
    slot?.scrollIntoView({ behavior: "smooth", block: "center" })
  }
</script>

<section aria-label="Video" class="no-print">
  <div class="mb-4 flex min-h-11 flex-wrap items-center justify-between gap-x-3 gap-y-1">
    <h2 class="section-title">Video</h2>
    <a
      href={embed?.watchUrl ?? video}
      target="_blank"
      rel="noopener noreferrer"
      class="link inline-flex min-h-11 items-center gap-1 text-sm"
    >
      {embed && embed.provider !== "file" && embed.provider !== "jwplayer"
        ? `Watch on ${embed.label}`
        : (hostOf(video) ?? "Watch")}<ExternalLink class="size-3.5" />
    </a>
  </div>

  {#if embed}
    <div
      bind:this={slot}
      class={["slot rounded-ctl bg-tint relative", embed.vertical && "vertical"]}
    >
      <div class={["frame", docked && "docked", embed.vertical && "vertical"]}>
        {#if playing}
          {#if embed.provider === "file"}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video src={embed.embedUrl} controls autoplay playsinline class="size-full bg-black"
            ></video>
          {:else}
            <iframe
              src={autoplay(embed)}
              title={`${title}: video`}
              class="size-full"
              allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
              allowfullscreen
              referrerpolicy="strict-origin-when-cross-origin"
            ></iframe>
          {/if}
          {#if docked}
            <div class="bar">
              <button type="button" class="bar-btn" onclick={backToIt}>
                <ArrowUp class="size-4" /> Back to video
              </button>
              <button
                type="button"
                class="bar-btn"
                aria-label="Stop the video"
                title="Stop the video"
                onclick={stop}
              >
                <X class="size-4" />
              </button>
            </div>
          {/if}
        {:else}
          <button
            type="button"
            class="group relative grid size-full place-items-center overflow-hidden"
            aria-label={`Play the video: ${title}`}
            onclick={() => (playing = true)}
          >
            {#if embed.thumbnail && !noStill}
              <img
                src={embed.thumbnail}
                alt=""
                loading="lazy"
                referrerpolicy="no-referrer"
                onerror={() => (noStill = true)}
                class="absolute inset-0 size-full object-cover"
              />
            {/if}
            <span
              class="bg-tile text-on-tile relative grid size-16 place-items-center rounded-full transition-transform group-hover:scale-105"
            >
              <Play class="ml-1 size-7" fill="currentColor" />
            </span>
          </button>
        {/if}
      </div>
    </div>
  {:else}
    <a
      href={video}
      target="_blank"
      rel="noopener noreferrer"
      class="card hover:bg-tint/55 flex min-h-12 items-center gap-3 px-4 py-3 transition-colors"
    >
      <span class="bg-tile text-on-tile grid size-9 shrink-0 place-items-center rounded-full">
        <Play class="ml-0.5 size-4" fill="currentColor" />
      </span>
      <span class="min-w-0 font-bold break-words">Watch the video on {hostOf(video) ?? "its site"}</span>
    </a>
  {/if}
</section>

<style>
  .slot {
    aspect-ratio: 16 / 9;
    overflow: hidden;
  }
  /* Portrait videos (Shorts, TikTok, Reels) stay phone-sized */
  .slot.vertical {
    aspect-ratio: 9 / 16;
    max-width: 22rem;
    margin-inline: auto;
  }
  .frame {
    position: absolute;
    inset: 0;
  }
  /* Floating in a corner: the picture plus a bar under it. Phones: top right, clear of
     the tab bar and the timers above it. */
  .frame.docked {
    --w: min(18rem, 60vw);
    --ratio: 9 / 16;
    position: fixed;
    inset: auto;
    z-index: 45;
    right: 1rem;
    top: calc(env(safe-area-inset-top) + 0.75rem);
    width: var(--w);
    height: calc(var(--w) * var(--ratio) + 2.25rem);
    border-radius: var(--radius-ctl);
    overflow: hidden;
    background: #000;
    box-shadow: var(--menu-shadow);
  }
  .frame.docked.vertical {
    --w: min(9.5rem, 38vw);
    --ratio: 16 / 9;
  }
  /* Wider screens: bottom right, above the timer pills' row */
  @media (min-width: 768px) {
    .frame.docked {
      --w: 22rem;
      top: auto;
      bottom: 5rem;
      right: 1.5rem;
    }
    .frame.docked.vertical {
      --w: 12rem;
    }
  }
  .frame.docked :global(iframe),
  .frame.docked :global(video) {
    height: calc(100% - 2.25rem);
  }
  .bar {
    display: flex;
    height: 2.25rem;
    align-items: stretch;
    justify-content: space-between;
    background: var(--tile);
    color: var(--on-tile);
  }
  .bar-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding-inline: 0.75rem;
    font-size: 13px;
    font-weight: 700;
  }
  .bar-btn:hover {
    background: rgb(255 255 255 / 0.12);
  }
</style>
