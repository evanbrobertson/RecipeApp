<script lang="ts">
  import Globe from "@lucide/svelte/icons/globe"
  import LogOut from "@lucide/svelte/icons/log-out"
  import Monitor from "@lucide/svelte/icons/monitor"
  import Moon from "@lucide/svelte/icons/moon"
  import Sparkles from "@lucide/svelte/icons/sparkles"
  import Sun from "@lucide/svelte/icons/sun"
  import { api } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { ConnectorInfo } from "../lib/recipe"

  const page = pageState<{ connector: ConnectorInfo }>(async () => ({
    connector: await api<ConnectorInfo>("/api/connector"),
  }))
  const info = $derived(page.data?.connector)

  const modes = [
    { value: "light", icon: Sun, label: "Light" },
    { value: "dark", icon: Moon, label: "Dark" },
    { value: "system", icon: Monitor, label: "Auto" },
  ]
  // A bare string (not JSON), shared with the theme script in the page head
  let mode = $state("system")
  try {
    mode = localStorage.getItem("crumb:theme") || "system"
  } catch {
    // storage blocked
  }

  function setMode(value: string) {
    mode = value
    try {
      localStorage.setItem("crumb:theme", value)
    } catch {
      // storage blocked: applies to this page only
    }
    const dark =
      value === "dark" || (value === "system" && matchMedia("(prefers-color-scheme: dark)").matches)
    document.documentElement.classList.toggle("dark", dark)
  }

  async function signOut() {
    await api("/api/auth/logout", { method: "POST" }).catch(() => {})
    location.href = "/login"
  }
</script>

<section>
  <h2 class="mb-2 font-semibold">Appearance</h2>
  <div class="bg-raised rounded-ui inline-flex p-1" role="radiogroup" aria-label="Theme">
    {#each modes as m (m.value)}
      <button
        type="button"
        role="radio"
        aria-checked={mode === m.value}
        class={[
          "rounded-ui flex h-9 items-center gap-1.5 px-4 text-sm font-semibold transition",
          mode === m.value ? "bg-canvas text-primary shadow-sm" : "text-ink-muted",
        ]}
        onclick={() => setMode(m.value)}
      >
        <m.icon class="size-4" />{m.label}
      </button>
    {/each}
  </div>
</section>

{#if info}
  <section class="text-ink-muted space-y-1.5 text-sm">
    <p class="flex items-center gap-2">
      <Globe class="size-4" />
      Tricky sites: {info.browserScraping
        ? "a real browser steps in when a site blocks us"
        : "browser fallback not installed"}
    </p>
    <p class="flex items-center gap-2">
      <Sparkles class="size-4" />
      Pasted text: {info.aiProvider ? `cleaned up by ${info.aiProvider}` : "built-in parser"}
    </p>
  </section>

  {#if info.authEnabled}
    <button type="button" class="btn btn-ghost -ml-3" onclick={signOut}>
      <LogOut /> Sign out
    </button>
  {/if}
{/if}
