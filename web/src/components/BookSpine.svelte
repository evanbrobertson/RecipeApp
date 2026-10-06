<script lang="ts">
  /**
   * A cloth-bound spine: grain and weave, a rounded back caught by light from the upper left, and
   * its dressing (gilt rules, a paper label or contrasting ends). Only paint; the shelf makes it a
   * button and the open book's flight turns it into the side of a book.
   */
  import { bookColor, bookPalette } from "../lib/books"
  import type { Spine } from "../lib/shelf"

  interface Props {
    spine: Spine
    /**
     * A lying book drawn standing, to be turned a quarter left (the open book's flight): its
     * light still has to come from above once it's turned.
     */
    turned?: boolean
  }
  let { spine, turned = false }: Props = $props()

  const look = $derived(bookPalette(spine.book.color))
  const up = $derived(spine.standing || turned)
  /** Foil on dark cloth is stamped in; dark type on pale cloth catches light on its lower lip */
  const pale = $derived(["sage", "butter", "cream"].includes(bookColor(spine.book.color)))
</script>

<span
  class={["spine", up ? "up" : "flat", turned && "turned", `s-${spine.style}`, pale && "pale"]}
  style:--w={`${turned ? spine.h : spine.w}px`}
  style:--h={`${turned ? spine.w : spine.h}px`}
  style:--cloth={look.cloth}
  style:--shade={look.shade}
  style:--foil={look.foil}
  style:--orn={look.bands[0]}
  style:--size={`${spine.font}px`}
>
  {#if spine.style === "ends"}
    <span class="end a"></span><span class="end b"></span>
  {:else if spine.style === "rules"}
    <span class="rule a"></span><span class="rule b"></span>
  {/if}
  <span class="title font-serif">
    {#each spine.lines as line, i (i)}<span class="line">{line}</span>{/each}
  </span>
  <span class="shine"></span>
</span>

<style>
  .spine {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--w);
    height: var(--h);
    overflow: hidden;
    color: var(--foil);
    background-color: var(--cloth);
    /* Mottled cloth, then a fine weave */
    background-image:
      var(--cloth-grain),
      repeating-linear-gradient(0deg, rgb(255 255 255 / 0.06) 0 1px, transparent 1px 3px),
      repeating-linear-gradient(90deg, rgb(0 0 0 / 0.05) 0 1px, transparent 1px 3px);
    background-blend-mode: soft-light, normal, normal;
    border-radius: 3px 3px 2px 2px;
  }
  .flat {
    border-radius: 3px;
  }

  /* Gilt: two fine rules at each end */
  .rule {
    position: absolute;
    border: 0 solid var(--orn);
    opacity: 0.9;
  }
  .up .rule {
    left: 3px;
    right: 3px;
    height: 6px;
    border-width: 1.5px 0;
  }
  .up .rule.a {
    top: 12px;
  }
  .up .rule.b {
    bottom: 12px;
  }
  .flat .rule {
    top: 3px;
    bottom: 3px;
    width: 6px;
    border-width: 0 1.5px;
  }
  .flat .rule.a {
    left: 12px;
  }
  .flat .rule.b {
    right: 12px;
  }

  /* Contrasting cloth at head and foot, edged with a foil line */
  .end {
    position: absolute;
    background-color: var(--orn);
    background-image: var(--cloth-grain);
    background-blend-mode: soft-light;
  }
  .up .end {
    left: 0;
    right: 0;
    height: 14px;
  }
  .up .end.a {
    top: 0;
    box-shadow: 0 2px 0 -0.5px var(--foil);
  }
  .up .end.b {
    bottom: 0;
    box-shadow: 0 -2px 0 -0.5px var(--foil);
  }
  .flat .end {
    top: 0;
    bottom: 0;
    width: 14px;
  }
  .flat .end.a {
    left: 0;
    box-shadow: 2px 0 0 -0.5px var(--foil);
  }
  .flat .end.b {
    right: 0;
    box-shadow: -2px 0 0 -0.5px var(--foil);
  }

  .title {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    max-width: calc(100% - 28px);
    font-size: var(--size);
    line-height: 1.12;
    text-align: center;
    white-space: nowrap;
    /* Stamped into dark cloth */
    text-shadow: 0 -1px 0 rgb(0 0 0 / 0.28);
  }
  .pale .title {
    text-shadow: 0 1px 0 rgb(255 255 255 / 0.35);
  }
  .line {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Up the spine, top to bottom, the way an English spine reads */
  .up .title {
    writing-mode: vertical-rl;
    max-width: none;
    max-height: calc(100% - 26px);
  }
  .up .line {
    max-width: none;
    max-height: 100%;
  }

  /* A paper label, gummed on by hand */
  .s-label .title {
    padding: 3px 10px 4px;
    color: #1c2b22;
    text-shadow: none;
    background:
      linear-gradient(160deg, rgb(255 255 255 / 0.35), transparent 40%, rgb(150 110 40 / 0.1)),
      #f3ead2;
    border-radius: 2px;
    box-shadow:
      inset 0 0 0 1px rgb(28 43 34 / 0.18),
      inset 0 0 0 3px #f3ead2,
      inset 0 0 0 4px rgb(28 43 34 / 0.28),
      0 1px 1px rgb(0 0 0 / 0.18);
    rotate: -0.6deg;
  }
  .up.s-label .title {
    padding: 10px 4px;
    max-width: calc(100% - 10px);
    rotate: 0.6deg;
  }

  /* The rounded back of the spine, lit from the upper left, darker at head and foot */
  .shine {
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: inherit;
  }
  .up .shine {
    background:
      linear-gradient(
        var(--across, 90deg),
        rgb(0 0 0 / 0.3) 0,
        rgb(255 255 255 / 0.16) 13%,
        rgb(255 255 255 / 0.05) 36%,
        transparent 52%,
        rgb(0 0 0 / 0.1) 76%,
        rgb(0 0 0 / 0.36) 100%
      ),
      linear-gradient(
        var(--along, 180deg),
        rgb(255 255 255 / 0.14),
        transparent 8px,
        transparent calc(100% - 7px),
        rgb(0 0 0 / 0.26)
      );
  }
  /* Drawn standing, turned to lie: the light keeps coming from above */
  .turned .shine {
    --across: 270deg;
    --along: 90deg;
  }
  .flat .shine {
    background:
      linear-gradient(
        180deg,
        rgb(255 255 255 / 0.2) 0,
        rgb(255 255 255 / 0.07) 26%,
        transparent 48%,
        rgb(0 0 0 / 0.1) 74%,
        rgb(0 0 0 / 0.34) 100%
      ),
      linear-gradient(
        90deg,
        rgb(0 0 0 / 0.2),
        transparent 6px,
        transparent calc(100% - 6px),
        rgb(0 0 0 / 0.22)
      );
  }
</style>
