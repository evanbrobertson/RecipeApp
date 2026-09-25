<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import ClipboardCheck from "@lucide/svelte/icons/clipboard-check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import Globe from "@lucide/svelte/icons/globe"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import LocateFixed from "@lucide/svelte/icons/locate-fixed"
  import LogOut from "@lucide/svelte/icons/log-out"
  import MonitorSmartphone from "@lucide/svelte/icons/monitor-smartphone"
  import Moon from "@lucide/svelte/icons/moon"
  import Sun from "@lucide/svelte/icons/sun"
  import Sunrise from "@lucide/svelte/icons/sunrise"
  import { api, errorMessage } from "../lib/api"
  import { plural } from "../lib/checks"
  import { pageState } from "../lib/page.svelte"
  import type { ChecksStatus, ConnectorInfo } from "../lib/recipe"
  import type { ThemeMode } from "../lib/theme"
  import { toast } from "../lib/toast"

  const page = pageState<{ connector: ConnectorInfo; checks?: ChecksStatus }>(async () => {
    const connector = await api<ConnectorInfo>("/api/connector")
    const checks = connector.weeChefChecks ? await api<ChecksStatus>("/api/checks") : undefined
    return { connector, checks }
  })
  const info = $derived(page.data?.connector)

  // ─── Wee Chef checks every imported recipe (only when the server has a key) ───
  let checks = $state<ChecksStatus | undefined>(page.data?.checks)
  $effect(() => {
    if (!checks && page.data?.checks) checks = page.data.checks
  })
  let starting = $state(false)
  const running = $derived(!!checks && checks.pending > 0)
  const checksText = $derived.by(() => {
    const c = checks
    if (!c) return ""
    if (c.pending > 0) return `Checking… ${c.checked} of ${c.eligible} done`
    const parts = [
      c.checked < c.eligible
        ? `${c.eligible - c.checked} not checked yet`
        : `All ${plural(c.checked, "recipe")} checked`,
    ]
    // On recipes already in the box it only suggests (plus an undoable symbol clean-up)
    if (c.checked < c.eligible) parts.push("suggests fixes, tidies only stray symbols")
    if (c.tidied) parts.push(`${plural(c.tidied, "thing")} tidied`)
    if (c.toCheck) parts.push(`${plural(c.toCheck, "recipe")} to look at`)
    return parts.join(" · ")
  })

  // Reads `checks` itself (not just `running`) so every fresh status schedules the next poll,
  // until nothing is pending
  $effect(() => {
    const current = checks
    if (!current || current.pending <= 0) return
    let cancelled = false
    const timer = setTimeout(async () => {
      const next = await api<ChecksStatus>("/api/checks").catch(() => ({ ...current }))
      if (!cancelled) checks = next
    }, 1500)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })

  async function checkAll() {
    if (starting || running) return
    starting = true
    try {
      checks = await api<ChecksStatus>("/api/checks", { method: "POST" })
      if (!checks.queued) toast({ title: "Every recipe has been checked" })
    } catch (e) {
      toast({ title: "Couldn't start the checks", description: errorMessage(e), tone: "error" })
    } finally {
      starting = false
    }
  }
  // Wee Chef is the AI; the provider is only a quiet hint for whoever set up the key
  const keyName = $derived(info?.aiProvider === "Claude" ? "Anthropic" : info?.aiProvider)

  // ─── Theme (the logic lives in lib/theme-boot.js, inlined in the head) ───
  const theme = window.crumbTheme
  const modes: { value: ThemeMode; icon: typeof Sun; label: string; text: string }[] = [
    { value: "light", icon: Sun, label: "Light", text: "Always light" },
    { value: "dark", icon: Moon, label: "Dark", text: "Always dark" },
    { value: "system", icon: MonitorSmartphone, label: "System", text: "Follows your device" },
    { value: "sun", icon: Sunrise, label: "Sunrise & sunset", text: "Dark from sunset to sunrise" },
  ]

  let mode = $state<ThemeMode>(theme.mode())
  let saved = $state(savedLocation())
  let locating = $state(false)
  // Hidden when a Permissions-Policy header blocks geolocation (Chromium can tell us)
  const policy = (document as { featurePolicy?: { allowsFeature(f: string): boolean } })
    .featurePolicy
  const canLocate = "geolocation" in navigator && (policy?.allowsFeature("geolocation") ?? true)

  function savedLocation(): boolean {
    try {
      const loc = JSON.parse(localStorage.getItem(theme.LOC) || "null")
      return !!loc && isFinite(loc.lat) && isFinite(loc.lng)
    } catch {
      return false
    }
  }

  // Re-read after any change (here or in another tab) so the times follow the location
  const sun = $derived.by(() => {
    void saved
    return mode === "sun" ? theme.sun() : null
  })

  // One decimal (about 10 km) is plenty for sunrise and sunset
  const round = (n: number) => Math.round(n * 10) / 10

  const time = (ms: number) =>
    new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }).toLowerCase()

  function setMode(value: ThemeMode) {
    mode = value
    try {
      localStorage.setItem(theme.KEY, value)
    } catch {
      // storage blocked: applies to this page only
      document.documentElement.classList.toggle(
        "dark",
        value === "dark" ||
          (value === "system" && matchMedia("(prefers-color-scheme: dark)").matches) ||
          (value === "sun" && theme.sun().dark),
      )
      return
    }
    theme.apply()
  }

  // Radio group keys: arrows move and select, Home/End jump
  let group: HTMLElement | undefined = $state()
  function onkey(e: KeyboardEvent) {
    const i = modes.findIndex((m) => m.value === mode)
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key]
    let next = step === undefined ? -1 : (i + step + modes.length) % modes.length
    if (e.key === "Home") next = 0
    if (e.key === "End") next = modes.length - 1
    if (next < 0) return
    e.preventDefault()
    setMode(modes[next]!.value)
    group?.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus()
  }

  function locate() {
    if (locating) return
    locating = true
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        locating = false
        try {
          localStorage.setItem(
            theme.LOC,
            JSON.stringify({ lat: round(pos.coords.latitude), lng: round(pos.coords.longitude) }),
          )
        } catch {
          toast({ title: "Couldn't save your location", tone: "error" })
          return
        }
        saved = true
        theme.apply()
        toast({ title: "Using your location", description: "Sunrise and sunset are exact now." })
      },
      (err) => {
        locating = false
        toast(
          err.code === err.PERMISSION_DENIED
            ? {
                title: "Location is blocked",
                description:
                  "Allow it in your browser's site settings, or keep the time-zone estimate.",
                tone: "error",
              }
            : {
                title: "Couldn't find your location",
                description: "Still using the estimate from your time zone.",
                tone: "error",
              },
        )
      },
      { timeout: 15000, maximumAge: 3600000 },
    )
  }

  function forget() {
    try {
      localStorage.removeItem(theme.LOC)
    } catch {
      // storage blocked: nothing was saved
    }
    saved = false
    theme.apply()
  }

  // Changed in another tab
  function sync(e: StorageEvent) {
    if (e.key === theme.KEY) mode = theme.mode()
    if (e.key === theme.LOC) saved = savedLocation()
  }

  async function signOut() {
    await api("/api/auth/logout", { method: "POST" }).catch(() => {})
    location.href = "/login"
  }
</script>

<svelte:window onstorage={sync} />

<div class="space-y-8">
<section>
  <h2 class="settings-heading" id="theme-title">Theme</h2>
  <div class="list-card">
    <div role="radiogroup" aria-labelledby="theme-title" bind:this={group} onkeydown={onkey}>
      {#each modes as m (m.value)}
        {@const on = mode === m.value}
        <button
          type="button"
          role="radio"
          aria-checked={on}
          tabindex={on ? 0 : -1}
          class="list-row settings-row w-full text-left"
          onclick={() => setMode(m.value)}
        >
          <m.icon class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block font-bold">{m.label}</span>
            <span class="text-ink-muted block text-sm">{m.text}</span>
          </span>
          <Check class={["text-primary size-5 flex-none", !on && "invisible"]} />
        </button>
      {/each}
    </div>

    {#if sun}
      <div class="border-line border-t py-3 pr-4 pl-[3.125rem]">
        <p class="text-[15px] font-bold" aria-live="polite">
          {#if sun.rise !== undefined && sun.set !== undefined}
            Dark from {time(sun.set)} until {time(sun.rise)}
          {:else}
            No sunset here today, so it stays {sun.dark ? "dark" : "light"}
          {/if}
        </p>
        <p class="meta mt-0.5">
          {saved ? "Using your saved location" : "Estimated from your time zone"}
        </p>
        <div class="mt-2.5 flex flex-wrap gap-2">
          {#if canLocate}
            <button type="button" class="btn btn-soft" onclick={locate}>
              {#if locating}<LoaderCircle class="animate-spin" />{:else}<LocateFixed />{/if}
              {saved ? "Update my location" : "Use my location"}
            </button>
          {/if}
          {#if saved}
            <button type="button" class="btn btn-ghost" onclick={forget}>Forget location</button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</section>

{#if info}
  <section>
    <h2 class="settings-heading">Settings</h2>
    <div class="list-card">
      <div class="list-row settings-row">
        <Globe class="settings-icon" />
        <span class="min-w-0 flex-1">
          <span class="block font-bold">Tricky sites</span>
          <span class="text-ink-muted block text-sm">
            {info.browserScraping
              ? "A real browser steps in when a site blocks us"
              : "Browser fallback not installed"}
          </span>
        </span>
      </div>
      <div class="list-row settings-row">
        <ChefHat class="settings-icon" />
        <span class="min-w-0 flex-1">
          <span class="block font-bold">Pasted text</span>
          <span class="text-ink-muted block text-sm">
            {info.aiProvider ? "Tidied up by Wee Chef" : "Read by the built-in parser"}
          </span>
        </span>
        {#if keyName}<span class="meta flex-none">{keyName} key</span>{/if}
      </div>
      {#if info.weeChefChecks && checks}
        <button
          type="button"
          class="list-row settings-row w-full text-left"
          disabled={running}
          aria-busy={running}
          onclick={checkAll}
        >
          <ClipboardCheck class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block font-bold">Check all recipes with Wee Chef</span>
            <span class="text-ink-muted block text-sm" aria-live="polite">{checksText}</span>
          </span>
          {#if running || starting}<LoaderCircle
              class="text-primary size-5 flex-none animate-spin"
            />{/if}
        </button>
      {/if}
      {#if info.authEnabled}
        <button type="button" class="list-row settings-row w-full text-left" onclick={signOut}>
          <LogOut class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Sign out</span>
        </button>
      {/if}
    </div>
  </section>
{/if}
</div>
