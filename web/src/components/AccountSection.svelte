<script lang="ts">
  import LogOut from "@lucide/svelte/icons/log-out"
  import MonitorSmartphone from "@lucide/svelte/icons/monitor-smartphone"
  import UserRound from "@lucide/svelte/icons/user-round"
  import { onMount } from "svelte"
  import { api, errorMessage } from "../lib/api"
  import { toast } from "../lib/toast"

  /** With accounts (AUTH_MODE=accounts): who's signed in, and their sessions to sign out. */
  type Status = {
    mode: "password" | "accounts"
    user?: { name: string; email: string } | null
    household?: { name: string; role: string } | null
  }
  type Session = {
    id: number
    userAgent: string | null
    createdAt: number
    lastSeenAt: number
    current: boolean
  }

  let status = $state<Status | null>(null)
  let sessions = $state<Session[]>([])
  let busy = $state(false)

  onMount(async () => {
    status = await api<Status>("/api/auth/status").catch(() => null)
    if (status?.mode === "accounts" && status.user) await load()
  })

  async function load() {
    sessions = await api<Session[]>("/api/auth/sessions").catch(() => [])
  }

  /** "Firefox on Linux", roughly: enough to tell one's devices apart. */
  function device(agent: string | null) {
    if (!agent) return "Unknown device"
    const browser = /Edg\//.test(agent)
      ? "Edge"
        : /Firefox\//.test(agent)
          ? "Firefox"
          : /Chrome\//.test(agent)
            ? "Chrome"
            : /Safari\//.test(agent)
              ? "Safari"
              : /okhttp|Android/.test(agent)
                ? "Crumb app"
                : "Browser"
    const system = /iPhone|iPad/.test(agent)
      ? "iOS"
      : /Android/.test(agent)
        ? "Android"
        : /Mac OS X/.test(agent)
          ? "macOS"
          : /Windows/.test(agent)
            ? "Windows"
            : /Linux/.test(agent)
              ? "Linux"
              : ""
    return system ? `${browser} on ${system}` : browser
  }

  const day = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })

  async function revoke(s: Session) {
    busy = true
    try {
      await api(`/api/auth/sessions/${s.id}`, { method: "DELETE" })
      await load()
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't sign that out"), tone: "error" })
    }
    busy = false
  }

  async function revokeOthers() {
    busy = true
    try {
      await api("/api/auth/sessions/revoke-others", { method: "POST" })
      await load()
      toast({ title: "Signed out everywhere else" })
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't sign out"), tone: "error" })
    }
    busy = false
  }
</script>

{#if status?.mode === "accounts" && status.user}
  <section>
    <h2 class="settings-heading" id="account-title">Account</h2>
    <div class="list-card" role="group" aria-labelledby="account-title">
      <div class="list-row settings-row">
        <UserRound class="settings-icon" />
        <span class="min-w-0 flex-1">
          <span class="block truncate font-bold">{status.user.name}</span>
          <span class="text-ink-muted block truncate text-sm">{status.user.email}</span>
        </span>
        {#if status.household}
          <span class="meta flex-none">{status.household.name}</span>
        {/if}
      </div>
      {#each sessions as s (s.id)}
        <div class="list-row settings-row">
          <MonitorSmartphone class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block truncate font-bold">{device(s.userAgent)}</span>
            <span class="text-ink-muted block text-sm">
              {s.current ? "This device" : `Last used ${day(s.lastSeenAt)}`}
            </span>
          </span>
          {#if !s.current}
            <button
              type="button"
              class="btn btn-ghost btn-icon flex-none"
              aria-label={`Sign out ${device(s.userAgent)}`}
              title="Sign out"
              disabled={busy}
              onclick={() => revoke(s)}
            >
              <LogOut />
            </button>
          {/if}
        </div>
      {/each}
      {#if sessions.length > 1}
        <button
          type="button"
          class="list-row settings-row w-full text-left"
          disabled={busy}
          onclick={revokeOthers}
        >
          <LogOut class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Sign out everywhere else</span>
        </button>
      {/if}
    </div>
  </section>
{/if}
