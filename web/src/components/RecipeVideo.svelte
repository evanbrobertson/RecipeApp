<script lang="ts">
  /**
   * The recipe's video, played in place. Nothing loads from the video's site until Play:
   * a still (YouTube's) or a tile stands in. Once it's playing, scrolling past it floats
   * the player in a window over the page so the steps can be read along with it: drag its
   * bar to move it, its corners to resize it (remembered on this device). The iframe
   * itself never moves in the page (moving it would reload it); only its styles change.
   */
  import ExternalLink from "@lucide/svelte/icons/external-link"
  import Play from "@lucide/svelte/icons/play"
  import ArrowUp from "@lucide/svelte/icons/arrow-up"
  import GripHorizontal from "@lucide/svelte/icons/grip-horizontal"
  import MoveDiagonal from "@lucide/svelte/icons/move-diagonal"
  import MoveDiagonal2 from "@lucide/svelte/icons/move-diagonal-2"
  import X from "@lucide/svelte/icons/x"
  import { hostOf, type VideoEmbed } from "../lib/recipe"
  import { read, write } from "../lib/storage"

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

  // ─── The floating window: where it is and how big, in CSS pixels ───
  interface Win {
    x: number
    y: number
    w: number
  }
  /** The window's bar, above the picture. */
  const BAR = 40
  /** Kept clear of the screen's edges. */
  const EDGE = 8
  const STEP = 32
  let win = $state<Win | null>(null)
  const tall = $derived(!!embed?.vertical)
  /** Height over width of the picture. */
  const ratio = $derived(tall ? 16 / 9 : 9 / 16)
  const height = (w: number) => Math.round(w * ratio) + BAR
  const storeKey = $derived(tall ? "crumb:video-window:tall" : "crumb:video-window")

  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi))
  function fitWidth(w: number) {
    const min = tall ? 150 : 260
    const most = Math.min(innerWidth - 2 * EDGE, (innerHeight - 2 * EDGE - BAR) / ratio)
    return Math.round(clamp(w, Math.min(min, most), most))
  }
  /** Whole on screen, at a size the screen can hold. */
  function fit(r: Win): Win {
    const w = fitWidth(r.w)
    return {
      w,
      x: Math.round(clamp(r.x, EDGE, innerWidth - w - EDGE)),
      y: Math.round(clamp(r.y, EDGE, innerHeight - height(w) - EDGE)),
    }
  }
  /** Big enough to use: bottom right above the timers on wider screens, top on phones. */
  function firstPlace(): Win {
    const wide = innerWidth >= 768
    const w = fitWidth(
      tall ? (wide ? 240 : innerWidth * 0.45) : wide ? Math.max(448, innerWidth * 0.36) : innerWidth,
    )
    return fit({ w, x: innerWidth, y: wide ? innerHeight - height(w) - 80 : EDGE })
  }

  $effect(() => {
    if (!docked || win) return
    const saved = read<Win | null>(storeKey, null)
    win = fit(saved && Number.isFinite(saved.w) ? saved : firstPlace())
  })
  // Keep it on screen when the window changes size or the phone turns
  function refit() {
    if (win) win = fit(win)
  }
  $effect(() => {
    if (!docked) return
    addEventListener("resize", refit)
    return () => removeEventListener("resize", refit)
  })

  type Kind = "move" | "left" | "right"
  let drag = $state<{ kind: Kind; x: number; y: number; from: Win } | null>(null)

  /** Moves or resizes by (dx, dy). A corner keeps the opposite bottom corner where it is. */
  function apply(kind: Kind, from: Win, dx: number, dy: number): Win {
    if (kind === "move") return fit({ ...from, x: from.x + dx, y: from.y + dy })
    const w = fitWidth(kind === "left" ? from.w - dx : from.w + dx)
    const bottom = from.y + height(from.w)
    return fit({ w, x: kind === "left" ? from.x + from.w - w : from.x, y: bottom - height(w) })
  }

  function start(kind: Kind, e: PointerEvent) {
    if (!win || e.button !== 0) return
    // The bar's own buttons still click
    if (kind === "move" && (e.target as Element).closest("button:not([data-grip])")) return
    e.preventDefault()
    ;(e.currentTarget as Element).setPointerCapture(e.pointerId)
    drag = { kind, x: e.clientX, y: e.clientY, from: win }
  }
  function moving(e: PointerEvent) {
    if (drag) win = apply(drag.kind, drag.from, e.clientX - drag.x, e.clientY - drag.y)
  }
  function done() {
    if (!drag) return
    drag = null
    write(storeKey, win)
  }
  /** Arrow keys do the same, a step at a time. */
  function key(kind: Kind, e: KeyboardEvent) {
    const d = {
      ArrowLeft: [-STEP, 0],
      ArrowRight: [STEP, 0],
      ArrowUp: [0, -STEP],
      ArrowDown: [0, STEP],
    }[e.key]
    if (!d || !win) return
    e.preventDefault()
    // Resizing: right and down grow it, whichever corner
    const [dx, dy] =
      kind === "move" ? d : [(kind === "left" ? -1 : 1) * (d[0] || d[1]), 0]
    win = apply(kind, win, dx, dy)
    write(storeKey, win)
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
      <div
        class={["frame", docked && "docked", drag && "dragging"]}
        style:left={docked && win ? `${win.x}px` : undefined}
        style:top={docked && win ? `${win.y}px` : undefined}
        style:width={docked && win ? `${win.w}px` : undefined}
        style:height={docked && win ? `${height(win.w)}px` : undefined}
      >
        {#if docked && win}
          <!-- The window's bar: drag it to move, its corners to resize -->
          <div
            class="bar"
            role="presentation"
            onpointerdown={(e) => start("move", e)}
            onpointermove={moving}
            onpointerup={done}
            onpointercancel={done}
          >
            <button
              type="button"
              class="bar-btn handle cursor-nwse-resize"
              aria-label="Resize the video (arrow keys)"
              title="Drag to resize"
              onpointerdown={(e) => (e.stopPropagation(), start("left", e))}
              onpointermove={moving}
              onpointerup={done}
              onpointercancel={done}
              onkeydown={(e) => key("left", e)}
            >
              <MoveDiagonal2 class="size-4" />
            </button>
            <button
              type="button"
              data-grip
              class="bar-btn grip min-w-0 flex-1 cursor-move justify-center"
              aria-label="Move the video (arrow keys)"
              title="Drag to move"
              onkeydown={(e) => key("move", e)}
            >
              <GripHorizontal class="size-5 opacity-70" />
            </button>
            <button type="button" class="bar-btn" onclick={backToIt} title="Back to video">
              <ArrowUp class="size-4" />{#if win.w >= 300}<span>Back to video</span>{/if}
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
            <button
              type="button"
              class="bar-btn handle cursor-nesw-resize"
              aria-label="Resize the video (arrow keys)"
              title="Drag to resize"
              onpointerdown={(e) => (e.stopPropagation(), start("right", e))}
              onpointermove={moving}
              onpointerup={done}
              onpointercancel={done}
              onkeydown={(e) => key("right", e)}
            >
              <MoveDiagonal class="size-4" />
            </button>
          </div>
        {/if}
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
  /* Floating over the page: the bar, then the picture. Where it sits and its size are set
     inline (see the script); until then, top right. */
  .frame.docked {
    position: fixed;
    inset: auto;
    top: calc(env(safe-area-inset-top) + 0.5rem);
    right: 0.5rem;
    z-index: 45;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-ctl);
    overflow: hidden;
    background: #000;
    box-shadow: var(--menu-shadow);
  }
  .frame.docked :global(iframe),
  .frame.docked :global(video) {
    min-height: 0;
    flex: 1;
  }
  /* The iframe would swallow the pointer mid-drag */
  .frame.dragging :global(iframe),
  .frame.dragging :global(video) {
    pointer-events: none;
  }
  .frame.dragging {
    user-select: none;
  }
  .bar {
    display: flex;
    height: 2.5rem;
    flex: none;
    align-items: stretch;
    background: var(--tile);
    color: var(--on-tile);
    touch-action: none;
  }
  .bar-btn {
    display: inline-flex;
    min-width: 2.5rem;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    padding-inline: 0.625rem;
    font-size: 13px;
    font-weight: 700;
    white-space: nowrap;
    touch-action: none;
  }
  .bar-btn:hover,
  .bar-btn:focus-visible {
    background: rgb(255 255 255 / 0.12);
  }
  .bar-btn:focus-visible {
    outline: 2px solid var(--on-tile);
    outline-offset: -3px;
  }
</style>
