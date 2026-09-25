<script lang="ts">
  /** Hand-drawn-ish bowls, boards and jars for mise en place. Bigger quantity, bigger bowl. */
  import type { Vessel } from "../lib/ingredients"

  let { vessel, filled = false, color }: { vessel: Vessel; filled?: boolean; color?: string } =
    $props()

  const SIZES: Record<Vessel, number> = {
    pinch: 44,
    ramekin: 58,
    small: 74,
    medium: 94,
    large: 116,
    board: 112,
    jar: 46,
  }
  const size = $derived(SIZES[vessel])
  const fill = $derived(color ?? "#d8bf9a")
</script>

<svg
    width={size}
    height={vessel === 'jar' ? size * 1.3 : vessel === 'board' ? size * 0.62 : size * 0.72}
    viewBox={vessel === 'jar' ? '0 0 100 130' : vessel === 'board' ? '0 0 100 62' : '0 0 100 72'}
    class={["prep-vessel overflow-visible", filled && "filled"]}
    aria-hidden="true"
  >
    <!-- Cutting board -->
    {#if vessel === "board"}
      <ellipse cx="50" cy="56" rx="46" ry="5" fill="var(--vessel-shadow)" />
      <rect x="4" y="10" width="86" height="44" rx="10" fill="var(--board-edge)" />
      <rect x="4" y="10" width="86" height="40" rx="10" fill="var(--board)" />
      <circle cx="84" cy="20" r="3.5" fill="var(--board-hole)" />
      <path
        d="M14 22 q20 -3 40 0 M18 34 q24 3 46 -1 M12 44 q20 -2 34 1"
        stroke="var(--board-edge)"
        stroke-width="1.5"
        fill="none"
      />
      <g class="bowl-fill">
        <rect x="24" y="24" width="9" height="9" rx="2" fill={fill} transform="rotate(-8 28 28)" />
        <rect x="37" y="28" width="9" height="9" rx="2" fill={fill} transform="rotate(12 41 32)" />
        <rect x="30" y="36" width="9" height="9" rx="2" fill={fill} />
        <rect x="46" y="20" width="9" height="9" rx="2" fill={fill} transform="rotate(20 50 24)" />
        <rect
          x="50"
          y="33"
          width="9"
          height="9"
          rx="2"
          fill={fill}
          transform="rotate(-15 54 37)"
        />
        <rect x="62" y="26" width="9" height="9" rx="2" fill={fill} transform="rotate(5 66 30)" />
      </g>
    
    <!-- Jar / shaker for "to taste" things -->
    {:else if vessel === "jar"}
      <ellipse cx="50" cy="124" rx="34" ry="5" fill="var(--vessel-shadow)" />
      <rect
        x="22"
        y="30"
        width="56"
        height="92"
        rx="14"
        fill="var(--jar-glass)"
        stroke="var(--jar-edge)"
        stroke-width="3"
      />
      <rect
        x="26"
        class="bowl-fill"
        y={filled ? 56 : 120}
        width="48"
        height={filled ? 62 : 0}
        rx="10"
        fill={fill}
      />
      <rect x="26" y="12" width="48" height="22" rx="6" fill="var(--jar-cap)" />
      <circle cx="40" cy="22" r="2.5" fill="var(--jar-holes)" />
      <circle cx="50" cy="22" r="2.5" fill="var(--jar-holes)" />
      <circle cx="60" cy="22" r="2.5" fill="var(--jar-holes)" />
    
    <!-- Bowls of every size -->
    {:else}
      <ellipse cx="50" cy="68" rx="30" ry="4" fill="var(--vessel-shadow)" />
      <path
        d="M4 26 Q 6 66 50 66 Q 94 66 96 26 Z"
        fill="var(--bowl)"
        stroke="var(--bowl-edge)"
        stroke-width="2.5"
      />
      <ellipse
        cx="50"
        cy="26"
        rx="46"
        ry="12"
        fill="var(--bowl-inside)"
        stroke="var(--bowl-edge)"
        stroke-width="2.5"
      />
      <g class="bowl-fill">
        <ellipse cx="50" cy="27" rx="38" ry="8" fill={fill} />
        <ellipse cx="50" cy="24" rx="26" ry="5" fill={fill} style="filter: brightness(1.08)" />
      </g>
      <path
        d="M14 36 Q 20 54 40 58"
        stroke="var(--bowl-gloss)"
        stroke-width="3"
        fill="none"
        stroke-linecap="round"
      />
        {/if}
  </svg>

<style>
/* Sage-glazed bowls on cream, a butter-wood board, a glass shaker with a tile-green cap */
.prep-vessel {
  --bowl: #fbf8ef;
  --bowl-inside: #e4ece3;
  --bowl-edge: #c9d6ca;
  --bowl-gloss: rgb(255 255 255 / 0.8);
  --board: #e6c792;
  --board-edge: #cfa96f;
  --board-hole: #b98f58;
  --jar-glass: #eef3ec;
  --jar-edge: #b8c8bb;
  --jar-cap: #2f6b4f;
  --jar-holes: #e4ece3;
  --vessel-shadow: rgb(28 43 34 / 0.12);
}
.prep-vessel .bowl-fill {
  opacity: 0;
  transform: translateY(6px) scale(0.6);
  transform-origin: center;
  transform-box: fill-box;
  transition:
    opacity 0.25s,
    transform 0.35s cubic-bezier(0.3, 1.6, 0.5, 1);
}
.prep-vessel.filled .bowl-fill {
  opacity: 1;
  transform: none;
}
.prep-vessel rect.bowl-fill {
  opacity: 1;
  transform: none;
  transition:
    y 0.4s,
    height 0.4s;
}
:global(.dark) .prep-vessel {
  --bowl: #2c3d33;
  --bowl-inside: #1a241e;
  --bowl-edge: #43584a;
  --bowl-gloss: rgb(255 255 255 / 0.14);
  --board: #b3915f;
  --board-edge: #94744a;
  --board-hole: #7c603c;
  --jar-glass: #24322a;
  --jar-edge: #4a5e50;
  --jar-cap: #3b7a5a;
  --jar-holes: #1a241e;
  --vessel-shadow: rgb(0 0 0 / 0.35);
}
</style>
