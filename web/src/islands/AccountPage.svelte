<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy"
  import Download from "@lucide/svelte/icons/download"
  import KeyRound from "@lucide/svelte/icons/key-round"
  import Link2 from "@lucide/svelte/icons/link-2"
  import LoaderCircle from "@lucide/svelte/icons/loader-circle"
  import LogOut from "@lucide/svelte/icons/log-out"
  import Mail from "@lucide/svelte/icons/mail"
  import Pencil from "@lucide/svelte/icons/pencil"
  import Plug from "@lucide/svelte/icons/plug"
  import Trash2 from "@lucide/svelte/icons/trash-2"
  import LinkOff from "@lucide/svelte/icons/unlink"
  import { onMount } from "svelte"
  import AccountSection from "../components/AccountSection.svelte"
  import PasswordInput from "../components/PasswordInput.svelte"
  import SocialButtons from "../components/SocialButtons.svelte"
  import {
    type ApiToken,
    type ConnectedApp,
    apiTokens,
    createApiToken,
    revokeApiToken,
    type Provider,
    type SignInMethods,
    type Status,
    accounts,
    authStatus,
    changeEmail,
    connectedApps,
    deleteAccount,
    disconnectApp,
    providerNames,
    socialError,
  } from "../lib/account"
  import { errorMessage } from "../lib/api"
  import { flash, toast } from "../lib/toast"

  /**
   * More → Account: who's signed in and how, their devices and household (AccountSection),
   * the apps connected to Crumb, their data (download, delete) and signing out. With one
   * password it's just the connected apps and signing out.
   */
  let status = $state<Status | null>(null)
  let methods = $state<SignInMethods | null>(null)
  let apps = $state<ConnectedApp[] | null>(null)
  let tokens = $state<ApiToken[] | null>(null)
  let tokenName = $state("")
  let tokenScope = $state<"read" | "write">("read")
  let tokenDays = $state("")
  let tokenError = $state("")
  /** A token just made: shown once, here, and nowhere after. */
  let fresh = $state<{ name: string; token: string } | null>(null)
  let busy = $state(false)
  /** The row asking to confirm: `app:<id>`, `token:<id>`, `unlink:<provider>`, `email` or `delete`. */
  let confirming = $state<string | null>(null)
  let password = $state("")
  let typedEmail = $state("")
  let deleteError = $state("")
  let newEmail = $state("")
  let newEmailAgain = $state("")
  let emailPassword = $state("")
  let emailError = $state("")

  const signedIn = $derived(!!status && status.mode !== "password" && !!status.user)
  const linkable = $derived(
    (status?.providers ?? []).filter((p) => !methods?.linked.some((l) => l.provider === p)),
  )
  /** Unlinking the last way in isn't allowed. */
  const ways = $derived((methods?.password ? 1 : 0) + (methods?.linked.length ?? 0))

  onMount(async () => {
    // Back from linking Google or Apple
    const params = new URLSearchParams(location.search)
    const linked = params.get("linked") as Provider | null
    const failed = params.get("error")
    if (linked && linked in providerNames) toast({ title: `${providerNames[linked]} linked` })
    if (failed) toast({ title: socialError(failed)!, tone: "error" })
    if (linked || failed) history.replaceState(null, "", location.pathname)

    status = await authStatus()
    await Promise.all([loadApps(), loadTokens(), signedIn ? loadMethods() : null])
  })

  async function loadMethods() {
    methods = await accounts(status!.mode)
      .signInMethods()
      .catch(() => null)
  }

  async function loadApps() {
    apps = await connectedApps().catch(() => [])
  }

  async function loadTokens() {
    tokens = await apiTokens().catch(() => [])
  }

  async function makeToken(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    tokenError = ""
    try {
      const made = await createApiToken(
        tokenName.trim(),
        tokenScope,
        tokenDays ? Number(tokenDays) : undefined,
      )
      fresh = { name: tokenName.trim(), token: made.token }
      tokenName = ""
      await loadTokens()
    } catch (err) {
      tokenError = errorMessage(err, "Couldn't make the token")
    }
    busy = false
  }

  async function revoke(token: ApiToken) {
    busy = true
    try {
      await revokeApiToken(token)
      confirming = null
      await loadTokens()
      toast({ title: `${token.name} revoked`, description: "It stopped working." })
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't revoke it"), tone: "error" })
    }
    busy = false
  }

  async function copyToken() {
    try {
      await navigator.clipboard.writeText(fresh!.token)
      toast({ title: "Token copied" })
    } catch {
      toast({ title: "Couldn't copy. Select it and copy by hand", tone: "error" })
    }
  }

  const day = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })

  async function unlink(p: Provider) {
    busy = true
    try {
      await accounts(status!.mode).unlink(p)
      confirming = null
      await loadMethods()
      toast({ title: `${providerNames[p]} unlinked` })
    } catch (err) {
      toast({ title: errorMessage(err, `Couldn't unlink ${providerNames[p]}`), tone: "error" })
    }
    busy = false
  }

  async function disconnect(app: ConnectedApp) {
    busy = true
    try {
      await disconnectApp(app)
      confirming = null
      await loadApps()
      toast({ title: `${app.name} disconnected`, description: "It has to be approved again." })
    } catch (err) {
      toast({ title: errorMessage(err, "Couldn't disconnect it"), tone: "error" })
    }
    busy = false
  }

  async function remove(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    deleteError = ""
    try {
      await deleteAccount(methods?.password ? { password } : { confirm: typedEmail })
      flash({ title: "Your account was deleted" })
      location.href = "/login"
    } catch (err) {
      deleteError = errorMessage(err, "Couldn't delete your account")
      busy = false
    }
  }

  function closeEmail() {
    confirming = null
    newEmail = newEmailAgain = emailPassword = emailError = ""
  }

  async function saveEmail(e: SubmitEvent) {
    e.preventDefault()
    const email = newEmail.trim()
    // Typed twice, because a typo here is the very thing this is for
    if (email.toLowerCase() !== newEmailAgain.trim().toLowerCase()) {
      emailError = "The two addresses don't match"
      return
    }
    busy = true
    emailError = ""
    try {
      const made = await changeEmail(email, emailPassword)
      if (made.pending) {
        closeEmail()
        toast({
          title: `Check ${made.email}`,
          description: `Open the link sent there to switch. Until then, sign in with ${status!.user!.email}.`,
        })
      } else {
        flash({ title: "Email changed", description: `Sign in with ${made.email} from now on.` })
        location.reload()
        return
      }
    } catch (err) {
      emailError = errorMessage(err, "Couldn't change your email")
    }
    busy = false
  }

  async function signOut() {
    await accounts(status!.mode)
      .signOut()
      .catch(() => {})
    location.href = "/login"
  }
</script>

{#if !status}
  <div class="space-y-8">
    <div class="skeleton rounded-ui h-40"></div>
    <div class="skeleton rounded-ui h-28"></div>
  </div>
{:else}
  <div class="space-y-8">
    {#if signedIn}
      <AccountSection>
        {#if methods}
          <section>
            <h2 class="settings-heading" id="methods-title">Sign-in methods</h2>
            <div class="list-card" role="group" aria-labelledby="methods-title">
              {#if confirming === "email"}
                <form class="list-row settings-row flex-wrap items-start" onsubmit={saveEmail}>
                  <Mail class="settings-icon mt-0.5" />
                  <div class="min-w-0 flex-1 space-y-3">
                    <p class="font-bold">Change your email</p>
                    <p class="text-ink-muted text-sm">
                      {status.mode === "hosted"
                        ? "We'll send the new address a link, and switch once it's opened."
                        : "You'll sign in with the new address, and be signed out on other devices."}
                    </p>
                    <div>
                      <label class="label" for="new-email">New email</label>
                      <!-- svelte-ignore a11y_autofocus -->
                      <input
                        id="new-email"
                        type="email"
                        class="input"
                        autocomplete="email"
                        bind:value={newEmail}
                        autofocus
                        required
                      />
                    </div>
                    <div>
                      <label class="label" for="new-email-again">New email again</label>
                      <input
                        id="new-email-again"
                        type="email"
                        class="input"
                        autocomplete="off"
                        bind:value={newEmailAgain}
                        required
                      />
                    </div>
                    {#if methods.password}
                      <div>
                        <label class="label" for="email-password">Your password</label>
                        <PasswordInput
                          id="email-password"
                          large={false}
                          autocomplete="current-password"
                          bind:value={emailPassword}
                        />
                      </div>
                    {/if}
                    {#if emailError}<p class="text-error text-sm" role="alert">{emailError}</p>{/if}
                    <div class="flex flex-wrap justify-end gap-2">
                      <button type="button" class="btn btn-ghost" onclick={closeEmail}>Cancel</button>
                      <button type="submit" class="btn btn-primary" disabled={busy}>
                        {#if busy}<LoaderCircle class="animate-spin" />{/if}
                        Change email
                      </button>
                    </div>
                  </div>
                </form>
              {:else}
                <div class="list-row settings-row">
                  <Mail class="settings-icon" />
                  <span class="min-w-0 flex-1">
                    <span class="block font-bold">Email</span>
                    <span class="text-ink-muted block truncate text-sm">{status.user?.email}</span>
                  </span>
                  <button
                    type="button"
                    class="btn btn-ghost btn-icon flex-none"
                    aria-label="Change your email"
                    title="Change"
                    onclick={() => (confirming = "email")}
                  >
                    <Pencil />
                  </button>
                </div>
              {/if}
              {#if methods.password}
                <div class="list-row settings-row">
                  <KeyRound class="settings-icon" />
                  <span class="min-w-0 flex-1">
                    <span class="block font-bold">Password</span>
                    <span class="text-ink-muted block truncate text-sm">With your email</span>
                  </span>
                </div>
              {/if}
              {#each methods.linked as l (l.provider)}
                <div class="list-row settings-row flex-wrap">
                  <Link2 class="settings-icon" />
                  <span class="min-w-0 flex-1">
                    <span class="block font-bold">{providerNames[l.provider] ?? l.provider}</span>
                    <span class="text-ink-muted block truncate text-sm">
                      {l.email ?? `Linked ${day(l.createdAt)}`}
                    </span>
                  </span>
                  {#if ways > 1}
                    {#if confirming === `unlink:${l.provider}`}
                      <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
                        <span class="text-ink-muted mr-auto text-sm">
                          You won't be able to sign in with it.
                        </span>
                        <button
                          type="button"
                          class="btn btn-ghost"
                          onclick={() => (confirming = null)}
                        >
                          Keep
                        </button>
                        <button
                          type="button"
                          class="btn btn-danger"
                          disabled={busy}
                          onclick={() => unlink(l.provider)}
                        >
                          Unlink
                        </button>
                      </span>
                    {:else}
                      <button
                        type="button"
                        class="btn btn-ghost btn-icon flex-none"
                        aria-label={`Unlink ${providerNames[l.provider] ?? l.provider}`}
                        title="Unlink"
                        onclick={() => (confirming = `unlink:${l.provider}`)}
                      >
                        <LinkOff />
                      </button>
                    {/if}
                  {/if}
                </div>
              {/each}
            </div>
            {#if linkable.length}
              <div class="mt-3">
                <SocialButtons
                  mode={status.mode}
                  providers={linkable}
                  to={{ intent: "link" }}
                  divider={false}
                />
              </div>
            {/if}
          </section>
        {/if}
      </AccountSection>
    {/if}

    <section>
      <h2 class="settings-heading" id="apps-title">Connected apps</h2>
      <ul class="list-card" aria-labelledby="apps-title">
        {#if apps === null}
          <li class="list-row settings-row">
            <LoaderCircle class="settings-icon animate-spin" />
            <span class="text-ink-muted flex-1">Loading…</span>
          </li>
        {:else if apps.length === 0}
          <li class="list-row settings-row">
            <Plug class="settings-icon" />
            <span class="min-w-0 flex-1">
              <span class="block font-bold">No apps connected</span>
              <span class="text-ink-muted block text-sm">
                <a class="text-primary font-bold hover:underline" href="/connect">Connect Claude</a>
                to save and find recipes from a chat.
              </span>
            </span>
          </li>
        {:else}
          {#each apps as app (`${app.id}:${app.household?.id}`)}
            <li class="list-row settings-row flex-wrap">
              <Plug class="settings-icon" />
              <span class="min-w-0 flex-1">
                <span class="block truncate font-bold">{app.name}</span>
                <span class="text-ink-muted block text-sm">
                  {[app.household?.name, `connected ${day(app.connectedAt)}`]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>
              {#if confirming === `app:${app.id}`}
                <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
                  <span class="text-ink-muted mr-auto text-sm">
                    It stops working until it's approved again.
                  </span>
                  <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                    Keep
                  </button>
                  <button
                    type="button"
                    class="btn btn-danger"
                    disabled={busy}
                    onclick={() => disconnect(app)}
                  >
                    Disconnect
                  </button>
                </span>
              {:else}
                <button
                  type="button"
                  class="btn btn-ghost btn-icon flex-none"
                  aria-label={`Disconnect ${app.name}`}
                  title="Disconnect"
                  onclick={() => (confirming = `app:${app.id}`)}
                >
                  <LinkOff />
                </button>
              {/if}
            </li>
          {/each}
        {/if}
      </ul>
    </section>

    <section>
      <h2 class="settings-heading" id="tokens-title">API tokens</h2>
      <p class="text-ink-muted mb-2 text-sm">
        For scripts and agents that can't sign in through a browser, like the Crumb command line. A
        token works on this household only, and can never empty the Trash or touch your account.
      </p>
      {#if fresh}
        <div class="list-card mb-3 p-4" role="status">
          <p class="font-bold">Copy “{fresh.name}” now</p>
          <p class="text-ink-muted mb-2 text-sm">It won't be shown again.</p>
          <div class="flex items-center gap-2">
            <code class="input min-w-0 flex-1 truncate select-all">{fresh.token}</code>
            <button
              type="button"
              class="btn btn-ghost btn-icon flex-none"
              aria-label="Copy the token"
              onclick={copyToken}
            >
              <Copy />
            </button>
          </div>
          <button type="button" class="btn btn-ghost mt-2" onclick={() => (fresh = null)}>
            Done
          </button>
        </div>
      {/if}
      <ul class="list-card" aria-labelledby="tokens-title">
        {#if tokens === null}
          <li class="list-row settings-row">
            <LoaderCircle class="settings-icon animate-spin" />
            <span class="text-ink-muted flex-1">Loading…</span>
          </li>
        {:else if tokens.length === 0}
          <li class="list-row settings-row">
            <KeyRound class="settings-icon" />
            <span class="text-ink-muted flex-1">No tokens yet</span>
          </li>
        {:else}
          {#each tokens as token (token.id)}
            <li class="list-row settings-row flex-wrap">
              <KeyRound class="settings-icon" />
              <span class="min-w-0 flex-1">
                <span class="block truncate font-bold">{token.name}</span>
                <span class="text-ink-muted block text-sm">
                  {[
                    token.scope === "write" ? "Can read and change" : "Read only",
                    token.household?.name,
                    token.expired
                      ? "expired"
                      : token.expiresAt
                        ? `expires ${day(token.expiresAt)}`
                        : null,
                    token.lastUsedAt ? `used ${day(token.lastUsedAt)}` : "never used",
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>
              {#if confirming === `token:${token.id}`}
                <span class="flex w-full flex-wrap items-center justify-end gap-2 pl-[2.125rem]">
                  <span class="text-ink-muted mr-auto text-sm">Anything using it stops working.</span>
                  <button type="button" class="btn btn-ghost" onclick={() => (confirming = null)}>
                    Keep
                  </button>
                  <button
                    type="button"
                    class="btn btn-danger"
                    disabled={busy}
                    onclick={() => revoke(token)}
                  >
                    Revoke
                  </button>
                </span>
              {:else}
                <button
                  type="button"
                  class="btn btn-ghost btn-icon flex-none"
                  aria-label={`Revoke ${token.name}`}
                  title="Revoke"
                  onclick={() => (confirming = `token:${token.id}`)}
                >
                  <Trash2 />
                </button>
              {/if}
            </li>
          {/each}
        {/if}
      </ul>
      <form class="list-card mt-3 grid gap-3 p-4" onsubmit={makeToken}>
        <div>
          <label class="label" for="token-name">Name</label>
          <input
            id="token-name"
            class="input"
            bind:value={tokenName}
            maxlength="80"
            placeholder="Laptop, my agent…"
            required
          />
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="label" for="token-scope">Can</label>
            <select id="token-scope" class="input" bind:value={tokenScope}>
              <option value="read">Only read</option>
              <option value="write">Read and change</option>
            </select>
          </div>
          <div>
            <label class="label" for="token-days">Lasts</label>
            <select id="token-days" class="input" bind:value={tokenDays}>
              <option value="">Until revoked</option>
              <option value="30">30 days</option>
              <option value="90">90 days</option>
              <option value="365">1 year</option>
            </select>
          </div>
        </div>
        {#if tokenError}
          <p class="text-error text-sm" role="alert">{tokenError}</p>
        {/if}
        <button class="btn btn-primary justify-self-start" disabled={busy || !tokenName.trim()}>
          Make a token
        </button>
      </form>
    </section>

    {#if signedIn}
      <section>
        <h2 class="settings-heading" id="data-title">Your data</h2>
        <div class="list-card" role="group" aria-labelledby="data-title">
          <a href="/api/account/export" class="list-row settings-row" download>
            <Download class="settings-icon" />
            <span class="min-w-0 flex-1">
              <span class="block font-bold">Download my data</span>
              <span class="text-ink-muted block text-sm">
                Your account and every household's recipes, as one JSON file
              </span>
            </span>
          </a>
          {#if confirming === "delete"}
            <form class="list-row settings-row flex-wrap items-start" onsubmit={remove}>
              <Trash2 class="settings-icon mt-0.5" />
              <div class="min-w-0 flex-1 space-y-3">
                <p class="font-bold">Delete your account?</p>
                <p class="text-ink-muted text-sm">
                  This can't be undone. Households you share pass to the next person in them;
                  ones that are only yours are deleted with their recipes. Download your data
                  first if you want a copy.
                </p>
                {#if methods?.password}
                  <div>
                    <label class="label" for="delete-password">Your password</label>
                    <PasswordInput
                      id="delete-password"
                      large={false}
                      autocomplete="current-password"
                      bind:value={password}
                      autofocus
                    />
                  </div>
                {:else}
                  <div>
                    <label class="label" for="delete-email">Type {status.user?.email} to confirm</label>
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      id="delete-email"
                      type="email"
                      class="input"
                      autocomplete="off"
                      bind:value={typedEmail}
                      autofocus
                      required
                    />
                  </div>
                {/if}
                {#if deleteError}<p class="text-error text-sm" role="alert">{deleteError}</p>{/if}
                <div class="flex flex-wrap justify-end gap-2">
                  <button
                    type="button"
                    class="btn btn-ghost"
                    onclick={() => {
                      confirming = null
                      deleteError = ""
                    }}
                  >
                    Keep my account
                  </button>
                  <button type="submit" class="btn btn-danger" disabled={busy}>
                    {#if busy}<LoaderCircle class="animate-spin" />{/if}
                    Delete account
                  </button>
                </div>
              </div>
            </form>
          {:else}
            <button
              type="button"
              class="list-row settings-row w-full text-left"
              onclick={() => (confirming = "delete")}
            >
              <Trash2 class="settings-icon" />
              <span class="min-w-0 flex-1">
                <span class="text-error block font-bold">Delete account</span>
                <span class="text-ink-muted block text-sm">And the households only you are in</span>
              </span>
            </button>
          {/if}
        </div>
      </section>
    {/if}

    {#if signedIn || status.passwordRequired}
      <div class="list-card">
        <button type="button" class="list-row settings-row w-full text-left" onclick={signOut}>
          <LogOut class="settings-icon" />
          <span class="min-w-0 flex-1 font-bold">Sign out</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
