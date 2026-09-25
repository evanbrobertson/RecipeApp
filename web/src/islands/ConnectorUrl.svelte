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
  <div class="border-line rounded-ui space-y-3 border p-4 sm:p-5">
    <label class="text-sm font-medium" for="mcp-url">Connector URL</label>
    <div class="flex gap-2">
      <input
        id="mcp-url"
        value={info?.mcpUrl ?? ""}
        readonly
        class="input flex-1 font-mono text-sm"
        onfocus={(e) => e.currentTarget.select()}
      />
      <button type="button" class="btn btn-primary h-11" onclick={copy}>
        {#if copied}<Check /> Copied{:else}<Copy /> Copy{/if}
      </button>
    </div>
    {#if info && !info.authEnabled}
      <div class="rounded-ui flex gap-3 bg-amber-50 p-3 text-sm text-amber-900 dark:bg-amber-950/30 dark:text-amber-200">
        <ShieldAlert class="mt-0.5 size-5 shrink-0" />
        <div>
          <p class="font-semibold">No password set</p>
          <p class="opacity-90">
            Anyone with this URL can read and change your recipes. Set APP_PASSWORD before you
            deploy.
          </p>
        </div>
      </div>
    {/if}
  </div>
{:else}
  <pre class="bg-raised rounded-ui overflow-x-auto p-4 text-sm"><code
      >claude mcp add --transport http recipes {info?.mcpUrl ?? "https://<your-app>/mcp"}</code
    ></pre>
{/if}
