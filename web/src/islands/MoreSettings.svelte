<script lang="ts">
  import BookOpen from "@lucide/svelte/icons/book-open"
  import Check from "@lucide/svelte/icons/check"
  import ChevronRight from "@lucide/svelte/icons/chevron-right"
  import ClipboardCheck from "@lucide/svelte/icons/clipboard-check"
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import Copy from "@lucide/svelte/icons/copy"
  import CookingPot from "@lucide/svelte/icons/cooking-pot"
  import Globe from "@lucide/svelte/icons/globe"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import LocateFixed from "@lucide/svelte/icons/locate-fixed"
  import LinkOff from "@lucide/svelte/icons/unlink"
  import LogOut from "@lucide/svelte/icons/log-out"
  import MonitorSmartphone from "@lucide/svelte/icons/monitor-smartphone"
  import Moon from "@lucide/svelte/icons/moon"
  import Sun from "@lucide/svelte/icons/sun"
  import Sunrise from "@lucide/svelte/icons/sunrise"
  import { api, errorMessage } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { ConnectorInfo, SharedLink } from "../lib/recipe"
  import type { ThemeMode } from "../lib/theme"
  import { toast } from "../lib/toast"

  const page = pageState<{ connector: ConnectorInfo; shares?: SharedLink[] }>(async () => ({
    connector: await api<ConnectorInfo>("/api/connector"),
    shares: await api<SharedLink[]>("/api/shares").catch(() => []),
  }))
  const info = $derived(page.data?.connector)

  // ─── Shared links: every live one, with Copy and Stop sharing ───
  let shares = $state<SharedLink[] | undefined>(page.data?.shares)
  $effect(() => {
    if (!shares && page.data?.shares) shares = page.data.shares
  })
  /** The row asking "Stop sharing?", by its link. */
  let confirming = $state<string | null>(null)
  let stopping = $state(false)

  const day = (iso: string) =>
    new Date(iso).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })

  function shareMeta(s: SharedLink) {
    const parts = [s.kind === "cookbook" ? "Cookbook" : "Recipe", `shared ${day(s.createdAt)}`]
    parts.push(s.lastOpenedAt ? `last opened ${day(s.lastOpenedAt)}` : "not opened yet")
    return parts.join(" · ")
  }

  async function copyLink(s: SharedLink) {
    try {
      await navigator.clipboard.writeText(s.url)
      toast({ title: "Link copied" })
    } catch {
      toast({ title: "Couldn't copy", tone: "error" })
    }
  }

  async function stopSharing(s: SharedLink) {
    stopping = true
    try {
      // By the recipe's or cookbook's id: the token never goes into a request URL
      const path = s.kind === "cookbook" ? "cookbooks" : "recipes"
      await api(`/api/${path}/${s.id}/share`, { method: "DELETE" })
      shares = shares?.filter((x) => x.url !== s.url)
      confirming = null
      toast({ title: "Stopped sharing", description: "The link no longer works." })
    } catch (e) {
      toast({ title: "Couldn't stop sharing", description: errorMessage(e), tone: "error" })
    } finally {
      stopping = false
    }
  }

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

{#if shares?.length}
  <section>
    <h2 class="settings-heading" id="shares-title">Shared links</h2>
    <ul class="list-card" aria-labelledby="shares-title">
      {#each shares as s (s.url)}
        {@const Kind = s.kind === "cookbook" ? BookOpen : CookingPot}
        <li class="list-row settings-row flex-wrap">
          <Kind class="settings-icon" />
          <span class="min-w-0 flex-1">
            <a
              class="block truncate font-bold hover:underline"
              href={s.kind === "cookbook" ? `/cookbooks/${s.id}` : `/recipes/${s.id}`}>{s.title}</a
            >
            <span class="text-ink-muted block text-sm">{shareMeta(s)}</span>
          </span>
          {#if confirming === s.url}
            <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
              <span class="text-ink-muted mr-auto text-sm">The link stops working for everyone.</span>
              <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                Keep
              </button>
              <button
                type="button"
                class="btn btn-danger"
                disabled={stopping}
                onclick={() => stopSharing(s)}
              >
                Stop sharing
              </button>
            </span>
          {:else}
            <span class="flex flex-none gap-1">
              <button
                type="button"
                class="btn btn-ghost btn-icon"
                aria-label={`Copy the link to ${s.title}`}
                title="Copy link"
                onclick={() => copyLink(s)}
              >
                <Copy />
              </button>
              <button
                type="button"
                class="btn btn-ghost btn-icon"
                aria-label={`Stop sharing ${s.title}`}
                title="Stop sharing"
                onclick={() => (confirming = s.url)}
              >
                <LinkOff />
              </button>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

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
            {info.weeChef ? "Tidied up by Wee Chef" : "Read by the built-in parser"}
          </span>
        </span>
        <!-- Always "Wee Chef", never the AI provider behind it -->
        {#if info.weeChef}<span class="meta flex-none">Wee Chef is on</span>{/if}
      </div>
      {#if info.weeChefChecks}
        <!-- Check all lives on the Suggestions page, beside what it finds -->
        <a href="/suggestions" class="list-row settings-row">
          <ClipboardCheck class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Wee Chef checks</span>
          <span class="meta flex-none">Suggestions</span>
          <ChevronRight class="text-ink-muted size-5 flex-none" />
        </a>
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
