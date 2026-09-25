<script lang="ts">
  import Check from "@lucide/svelte/icons/check"
  import Copy from "@lucide/svelte/icons/copy"
  import ShieldAlert from "@lucide/svelte/icons/shield-alert"
  import { api } from "../lib/api"
  import { pageState } from "../lib/page.svelte"
  import type { ConnectorInfo } from "../lib/recipe"

  /** `part` picks which piece of the connect page this island renders. */
  let { part = "url" }: { part?: "url" | "code" } = $props()

  const page = pageState<{ connector: ConnectorInfo }>(async () => ({
    connector: await api<ConnectorInfo>("/api/connector"),
  }))
  const info = $derived(page.data?.connector)
  let copied = $state(false)

  async function copy() {
    if (!info) return
    try {
      await navigator.clipboard.writeText(info.mcpUrl)
      copied = true
      setTimeout(() => (copied = false), 2000)
    } catch {
      // clipboard blocked: the field is selectable
    }
  }
</script>

{#if part === "url"}
  <div class="card p-4">
    <label class="label mb-2" for="mcp-url">Connector URL</label>
    <div class="flex gap-2">
      <input
        id="mcp-url"
        value={info?.mcpUrl ?? ""}
        readonly
        class="input min-w-0 flex-1 font-mono text-sm"
        onfocus={(e) => e.currentTarget.select()}
      />
      <button type="button" class="btn btn-primary" onclick={copy}>
        {#if copied}<Check /> Copied{:else}<Copy /> Copy{/if}
      </button>
    </div>
    {#if info && !info.authEnabled}
      <p class="text-ink-muted mt-3 flex gap-2 text-sm">
        <ShieldAlert class="text-error mt-px size-4 shrink-0" />
        <span>
          <strong class="text-ink">No password set.</strong> Anyone with this URL can change your recipes.
          Set APP_PASSWORD before you deploy.
        </span>
      </p>
    {/if}
  </div>
{:else}
  <pre class="bg-tint rounded-ctl overflow-x-auto p-4 text-sm"><code
      >claude mcp add --transport http recipes {info?.mcpUrl ?? "https://<your-app>/mcp"}</code
    ></pre>
{/if}
