<script lang="ts">
  import ArrowLeftRight from "@lucide/svelte/icons/arrow-left-right"
  import Copy from "@lucide/svelte/icons/copy"
  import DoorOpen from "@lucide/svelte/icons/door-open"
  import House from "@lucide/svelte/icons/house"
  import KeyRound from "@lucide/svelte/icons/key-round"
  import Link from "@lucide/svelte/icons/link"
  import LogOut from "@lucide/svelte/icons/log-out"
  import Mail from "@lucide/svelte/icons/mail"
  import MonitorSmartphone from "@lucide/svelte/icons/monitor-smartphone"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import UserMinus from "@lucide/svelte/icons/user-minus"
  import UserPlus from "@lucide/svelte/icons/user-plus"
  import UserRound from "@lucide/svelte/icons/user-round"
  import { type Snippet, onMount } from "svelte"
  import {
    type Accounts,
    type Device,
    type Household,
    type Member,
    type Passkey,
    type PendingInvite,
    type Status,
    accounts,
    authStatus,
    deviceName,
  } from "../lib/account"
  import { errorMessage } from "../lib/api"
  import { flash, toast } from "../lib/toast"

  /**
   * With accounts (self-hosted or hosted): who's signed in and their devices, and their
   * household: its members, invites, leaving, and the other households they're in.
   * `children` goes between the two (the account page's sign-in methods).
   */
  let { children }: { children?: Snippet } = $props()
  let status = $state<Status | null>(null)
  let client = $state<Accounts | null>(null)
  let devices = $state<Device[]>([])
  let passkeys = $state<Passkey[]>([])
  let household = $state<Household | null>(null)
  let busy = $state(false)
  /** The row asking to confirm: `remove:<member id>`, `passkey:<id>` or `leave`. */
  let confirming = $state<string | null>(null)
  let inviting = $state(false)
  let inviteEmail = $state("")
  /** The last link made, to copy (self-hosted invites are only ever links). */
  let madeLink = $state<string | null>(null)
  let renaming = $state(false)
  let newName = $state("")

  const signedIn = $derived(status?.mode !== "password" && !!status?.user)
  const hosted = $derived(status?.mode === "hosted")
  /** Hosted, in a browser that can make one. */
  const canPasskey = $derived(!!client?.passkeys?.supported())
  const owner = $derived(household?.role === "owner")
  const others = $derived(household?.households.filter((h) => h.id !== household?.id) ?? [])

  onMount(async () => {
    status = await authStatus()
    if (status.mode === "password" || !status.user) return
    client = accounts(status.mode)
    await Promise.all([loadDevices(), loadPasskeys(), loadHousehold()])
  })

  async function loadPasskeys() {
    passkeys = (await client?.passkeys?.list().catch(() => [])) ?? []
  }

  async function addPasskey() {
    busy = true
    try {
      if (await client!.passkeys!.add(device(navigator.userAgent))) {
        await loadPasskeys()
        toast({ title: "Passkey added" })
      }
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't add the passkey"), tone: "error" })
    }
    busy = false
  }

  async function removePasskey(p: Passkey) {
    busy = true
    try {
      await client!.passkeys!.remove(p)
      confirming = null
      await loadPasskeys()
      toast({ title: "Passkey removed" })
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't remove the passkey"), tone: "error" })
    }
    busy = false
  }

  async function loadDevices() {
    devices = (await client?.devices().catch(() => [])) ?? []
  }

  async function loadHousehold() {
    household = (await client?.household().catch(() => null)) ?? null
  }

  /** Runs a change, then reloads the household; toasts what went wrong. */
  async function change(work: () => Promise<unknown>, failed: string, done?: string) {
    busy = true
    try {
      await work()
      confirming = null
      await loadHousehold()
      if (done) toast({ title: done })
    } catch (err) {
      toast({ title: errorMessage(err, failed), tone: "error" })
    }
    busy = false
  }

  const device = deviceName

  const day = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })

  async function signOutDevice(d: Device) {
    busy = true
    try {
      await client!.signOutDevice(d)
      await loadDevices()
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't sign that out"), tone: "error" })
    }
    busy = false
  }

  async function signOutOthers() {
    busy = true
    try {
      await client!.signOutOthers()
      await loadDevices()
      toast({ title: "Signed out everywhere else" })
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't sign out"), tone: "error" })
    }
    busy = false
  }

  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url)
      toast({ title: "Invite link copied" })
    } catch {
      toast({ title: "Couldn't copy", tone: "error" })
    }
  }

  async function invite(e?: SubmitEvent) {
    e?.preventDefault()
    busy = true
    try {
      const made = await client!.invite(hosted ? inviteEmail.trim() : undefined)
      madeLink = made.url
      await loadHousehold()
      if (hosted) {
        toast({ title: `Invite sent to ${inviteEmail.trim()}` })
        inviteEmail = ""
        inviting = false
      } else {
        await copy(made.url)
      }
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't make an invite"), tone: "error" })
    }
    busy = false
  }

  function cancelInvite(i: PendingInvite) {
    return change(() => client!.cancelInvite(i), "Couldn't cancel the invite", "Invite cancelled")
  }

  function removeMember(m: Member) {
    return change(() => client!.removeMember(m), "Couldn't remove them", `${m.name} was removed`)
  }

  async function rename(e: SubmitEvent) {
    e.preventDefault()
    const name = newName.trim()
    if (!name || name === household?.name) {
      renaming = false
      return
    }
    await change(() => client!.rename(name), "Couldn't rename the household")
    renaming = false
  }

  /** Leaving or switching changes whose recipes every page shows: start over from home. */
  async function leave() {
    busy = true
    try {
      await client!.leave(household!)
      flash({ title: `You left ${household!.name}` })
      location.href = "/"
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't leave"), tone: "error" })
      busy = false
    }
  }

  async function switchTo(h: { id: string; name: string }) {
    busy = true
    try {
      await client!.switchTo(h.id)
      flash({ title: `Now in ${h.name}` })
      location.href = "/"
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't switch"), tone: "error" })
      busy = false
    }
  }

  const inviteMeta = (i: PendingInvite) =>
    [i.email ?? (i.createdBy ? `Link made by ${i.createdBy}` : "Invite link"), `until ${day(i.expiresAt)}`].join(
      " · ",
    )
</script>

{#if signedIn && status?.user}
  <section>
    <h2 class="settings-heading" id="account-title">Account</h2>
    <div class="list-card" role="group" aria-labelledby="account-title">
      <div class="list-row settings-row">
        <UserRound class="settings-icon" />
        <span class="min-w-0 flex-1">
          <span class="block truncate font-bold">{status.user.name}</span>
          <span class="text-ink-muted block truncate text-sm">{status.user.email}</span>
        </span>
      </div>
      {#each devices as d (d.id)}
        <div class="list-row settings-row">
          <MonitorSmartphone class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block truncate font-bold">{device(d.userAgent)}</span>
            <span class="text-ink-muted block text-sm">
              {d.current ? "This device" : `Last used ${day(d.lastSeenAt)}`}
            </span>
          </span>
          {#if !d.current}
            <button
              type="button"
              class="btn btn-ghost btn-icon flex-none"
              aria-label={`Sign out ${device(d.userAgent)}`}
              title="Sign out"
              disabled={busy}
              onclick={() => signOutDevice(d)}
            >
              <LogOut />
            </button>
          {/if}
        </div>
      {/each}
      {#each passkeys as p (p.id)}
        <div class="list-row settings-row flex-wrap">
          <KeyRound class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block truncate font-bold">{p.name}</span>
            <span class="text-ink-muted block text-sm">Passkey added {day(p.createdAt)}</span>
          </span>
          {#if confirming === `passkey:${p.id}`}
            <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
              <span class="text-ink-muted mr-auto text-sm">
                It stops signing you in. Also delete it from the device.
              </span>
              <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                Keep
              </button>
              <button
                type="button"
                class="btn btn-danger"
                disabled={busy}
                onclick={() => removePasskey(p)}
              >
                Remove
              </button>
            </span>
          {:else}
            <button
              type="button"
              class="btn btn-ghost btn-icon flex-none"
              aria-label={`Remove the passkey ${p.name}`}
              title="Remove"
              onclick={() => (confirming = `passkey:${p.id}`)}
            >
              <Trash2 />
            </button>
          {/if}
        </div>
      {/each}
      {#if canPasskey}
        <button
          type="button"
          class="list-row settings-row w-full text-left"
          disabled={busy}
          onclick={addPasskey}
        >
          <KeyRound class="settings-icon" />
          <span class="min-w-0 flex-1">
            <span class="block font-bold">Add a passkey</span>
            <span class="text-ink-muted block text-sm">
              Sign in with your fingerprint, face or screen lock
            </span>
          </span>
        </button>
      {/if}
      {#if devices.length > 1}
        <button
          type="button"
          class="list-row settings-row w-full text-left"
          disabled={busy}
          onclick={signOutOthers}
        >
          <LogOut class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Sign out everywhere else</span>
        </button>
      {/if}
    </div>
  </section>

  {@render children?.()}

  {#if household}
    <section>
      <h2 class="settings-heading" id="household-title">Household</h2>
      <div class="list-card" role="group" aria-labelledby="household-title">
        <div class="list-row settings-row">
          <House class="settings-icon" />
          {#if renaming}
            <form class="flex min-w-0 flex-1 gap-2" onsubmit={rename}>
              <label class="sr-only" for="household-name">Household name</label>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                id="household-name"
                class="input min-w-0 flex-1"
                maxlength="80"
                bind:value={newName}
                autofocus
                required
              />
              <button type="submit" class="btn btn-ghost" disabled={busy}>Save</button>
            </form>
          {:else}
            <span class="min-w-0 flex-1">
              <span class="block truncate font-bold">{household.name}</span>
              <span class="text-ink-muted block text-sm">
                {household.members.length === 1
                  ? "Just you"
                  : `${household.members.length} people share these recipes`}
              </span>
            </span>
            {#if owner}
              <button
                type="button"
                class="btn btn-ghost btn-icon flex-none"
                aria-label="Rename the household"
                title="Rename"
                onclick={() => {
                  newName = household!.name
                  renaming = true
                }}
              >
                <Pencil />
              </button>
            {/if}
          {/if}
        </div>
        {#each household.members as m (m.id)}
          <div class="list-row settings-row flex-wrap">
            <UserRound class="settings-icon" />
            <span class="min-w-0 flex-1">
              <span class="block truncate font-bold">{m.you ? `${m.name} (you)` : m.name}</span>
              <span class="text-ink-muted block truncate text-sm">
                {m.role === "owner" ? "Owner" : "Member"} · {m.email}
              </span>
            </span>
            {#if owner && !m.you && m.role !== "owner"}
              {#if confirming === `remove:${m.id}`}
                <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
                  <span class="text-ink-muted mr-auto text-sm">
                    They lose these recipes. Their account stays.
                  </span>
                  <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                    Keep
                  </button>
                  <button
                    type="button"
                    class="btn btn-danger"
                    disabled={busy}
                    onclick={() => removeMember(m)}
                  >
                    Remove
                  </button>
                </span>
              {:else}
                <button
                  type="button"
                  class="btn btn-ghost btn-icon flex-none"
                  aria-label={`Remove ${m.name} from the household`}
                  title="Remove"
                  onclick={() => (confirming = `remove:${m.id}`)}
                >
                  <UserMinus />
                </button>
              {/if}
            {/if}
          </div>
        {/each}
        {#if owner}
          {#each household.invites as i (i.id)}
            <div class="list-row settings-row">
              {#if i.email}<Mail class="settings-icon" />{:else}<Link class="settings-icon" />{/if}
              <span class="min-w-0 flex-1">
                <span class="block truncate font-bold">Invited</span>
                <span class="text-ink-muted block truncate text-sm">{inviteMeta(i)}</span>
              </span>
              <button
                type="button"
                class="btn btn-ghost flex-none"
                disabled={busy}
                onclick={() => cancelInvite(i)}
              >
                Cancel
              </button>
            </div>
          {/each}
          {#if madeLink}
            <div class="list-row settings-row">
              <Link class="settings-icon" />
              <span class="min-w-0 flex-1">
                <span class="block truncate font-bold">New invite link</span>
                <span class="text-ink-muted block text-sm">
                  {hosted
                    ? "Only works for the address it went to."
                    : "Works once, for a week. Send it to who you're inviting."}
                </span>
              </span>
              <button
                type="button"
                class="btn btn-ghost btn-icon flex-none"
                aria-label="Copy the invite link"
                title="Copy link"
                onclick={() => copy(madeLink!)}
              >
                <Copy />
              </button>
            </div>
          {/if}
          {#if hosted && inviting}
            <div class="list-row settings-row">
              <UserPlus class="settings-icon" />
              <form class="flex min-w-0 flex-1 gap-2" onsubmit={invite}>
                <label class="sr-only" for="invite-email">Their email</label>
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  id="invite-email"
                  type="email"
                  class="input min-w-0 flex-1"
                  placeholder="Their email"
                  autocomplete="off"
                  bind:value={inviteEmail}
                  autofocus
                  required
                />
                <button type="submit" class="btn btn-ghost" disabled={busy}>Send</button>
              </form>
            </div>
          {:else}
              <button
                type="button"
                class="list-row settings-row w-full text-left"
                disabled={busy}
                onclick={() => (hosted ? (inviting = true) : invite())}
              >
                <UserPlus class="settings-icon" />
                <span class="min-w-0 flex-1">
                  <span class="block font-bold">Invite someone</span>
                  <span class="text-ink-muted block text-sm">
                    {hosted ? "By email" : "Makes a link to copy and send"}
                  </span>
                </span>
              </button>
          {/if}
        {/if}
        {#each others as h (h.id)}
            <button
              type="button"
              class="list-row settings-row w-full text-left"
              disabled={busy}
              onclick={() => switchTo(h)}
            >
              <ArrowLeftRight class="settings-icon" />
              <span class="min-w-0 flex-1">
                <span class="block truncate font-bold">Switch to {h.name}</span>
                <span class="text-ink-muted block text-sm">Another household you're in</span>
              </span>
            </button>
        {/each}
        {#if !owner}
          {#if confirming === "leave"}
            <div class="list-row settings-row flex-wrap">
              <DoorOpen class="settings-icon" />
              <span class="min-w-0 flex-1 text-sm">
                You'll stop seeing {household.name}'s recipes.
              </span>
              <span class="flex flex-none gap-2">
                <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                  Stay
                </button>
                <button type="button" class="btn btn-danger" disabled={busy} onclick={leave}>
                  Leave
                </button>
              </span>
            </div>
          {:else}
            <button
              type="button"
              class="list-row settings-row w-full text-left"
              onclick={() => (confirming = "leave")}
            >
              <DoorOpen class="settings-icon" />
              <span class="min-w-0 flex-1 font-bold">Leave household</span>
            </button>
          {/if}
        {/if}
      </div>
    </section>
  {/if}
{/if}
