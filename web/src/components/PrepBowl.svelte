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
  const fill = $derived(color ?? "#d9b99b")
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
      <ellipse cx="50" cy="56" rx="46" ry="5" fill="rgb(0 0 0 / 0.12)" />
      <rect x="4" y="10" width="86" height="44" rx="10" fill="#c99662" />
      <rect x="4" y="10" width="86" height="40" rx="10" fill="#dcae78" />
      <circle cx="84" cy="20" r="3.5" fill="#b07f4d" />
      <path
        d="M14 22 q20 -3 40 0 M18 34 q24 3 46 -1 M12 44 q20 -2 34 1"
        stroke="#c99662"
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
      <ellipse cx="50" cy="124" rx="34" ry="5" fill="rgb(0 0 0 / 0.12)" />
      <rect
        x="22"
        y="30"
        width="56"
        height="92"
        rx="14"
        fill="var(--jar-glass, #e9eef0)"
        stroke="#b9c6cc"
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
      <rect x="26" y="12" width="48" height="22" rx="6" fill="#8a9aa3" />
      <circle cx="40" cy="22" r="2.5" fill="#dfe6ea" />
      <circle cx="50" cy="22" r="2.5" fill="#dfe6ea" />
      <circle cx="60" cy="22" r="2.5" fill="#dfe6ea" />
    
    <!-- Bowls of every size -->
    {:else}
      <ellipse cx="50" cy="68" rx="30" ry="4" fill="rgb(0 0 0 / 0.14)" />
      <path
        d="M4 26 Q 6 66 50 66 Q 94 66 96 26 Z"
        fill="var(--bowl, #ffffff)"
        stroke="var(--bowl-edge, #d8cfc2)"
        stroke-width="2.5"
      />
      <ellipse
        cx="50"
        cy="26"
        rx="46"
        ry="12"
        fill="var(--bowl-inside, #efe9df)"
        stroke="var(--bowl-edge, #d8cfc2)"
        stroke-width="2.5"
      />
      <g class="bowl-fill">
        <ellipse cx="50" cy="27" rx="38" ry="8" fill={fill} />
        <ellipse cx="50" cy="24" rx="26" ry="5" fill={fill} style="filter: brightness(1.08)" />
      </g>
      <path
        d="M14 36 Q 20 54 40 58"
        stroke="rgb(255 255 255 / 0.7)"
        stroke-width="3"
        fill="none"
        stroke-linecap="round"
      />
        {/if}
  </svg>

<style>
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
  --bowl: #3a332c;
  --bowl-inside: #2b251f;
  --bowl-edge: #574c40;
  --jar-glass: #2c3337;
}
</style>
