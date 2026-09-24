<script setup lang="ts">
useHead({ title: "Connect Claude · Just the Recipe" })

const { data } = useFetch("/api/connector")
const { copy, copied } = useClipboard({ copiedDuring: 2000 })

const examples = [
  "Save this recipe to my recipe box: [paste recipe]",
  "Import the recipe from https://…",
  "What can I make with chicken thighs and lemons from my saved recipes?",
  "Double my banana bread recipe and save the changes.",
  "Add my lasagne and tiramisu to a cookbook called Sunday dinner.",
]
</script>

<template>
  <div class="mx-auto max-w-2xl space-y-8">
    <div>
      <h1 class="font-serif text-2xl font-semibold sm:text-3xl">Connect to Claude</h1>
      <p class="text-muted mt-2">
        Add Just the Recipe as a connector in Claude. Then you can paste recipes into any chat, ask
        Claude to save them, search your collection, or have Claude adapt a saved recipe.
      </p>
    </div>

    <div class="border-default space-y-3 rounded-2xl border p-4 sm:p-5">
      <p class="text-sm font-medium">Connector URL</p>
      <div class="flex gap-2">
        <UInput
          :model-value="data?.mcpUrl ?? ''"
          readonly
          size="lg"
          class="flex-1 font-mono"
          aria-label="Connector URL"
        />
        <UButton
          :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'"
          :label="copied ? 'Copied' : 'Copy'"
          size="lg"
          @click="data && copy(data.mcpUrl)"
        />
      </div>
      <UAlert
        v-if="data && !data.authEnabled"
        color="warning"
        variant="subtle"
        icon="i-lucide-shield-alert"
        title="No password set"
        description="Anyone with this URL can read and change your recipes. Set NUXT_APP_PASSWORD before you deploy."
      />
    </div>

    <section>
      <h2 class="mb-3 font-serif text-xl font-semibold">Claude on the web, desktop and mobile</h2>
      <ol class="text-muted list-decimal space-y-2 pl-5">
        <li>In Claude, open <strong class="text-default">Settings → Connectors</strong>.</li>
        <li>
          Choose <strong class="text-default">Add custom connector</strong>, name it “Recipes” and
          paste the URL above.
        </li>
        <li>
          Click <strong class="text-default">Connect</strong>. You'll be sent here to approve it
          with your app password.
        </li>
        <li>In a chat, make sure the connector is switched on in the tools menu, then ask away.</li>
      </ol>
      <p class="text-muted mt-3 text-sm">
        Connectors you add on claude.ai are also available in the Claude mobile apps. On Team and
        Enterprise plans an owner may need to add the connector for the organisation first.
      </p>
    </section>

    <section>
      <h2 class="mb-3 font-serif text-xl font-semibold">Claude Code</h2>
      <pre
        class="bg-elevated overflow-x-auto rounded-xl p-4 text-sm"
      ><code>claude mcp add --transport http recipes {{ data?.mcpUrl }}</code></pre>
      <p class="text-muted mt-2 text-sm">Then run <code>/mcp</code> in Claude Code to sign in.</p>
    </section>

    <section>
      <h2 class="mb-3 font-serif text-xl font-semibold">Things to try</h2>
      <ul class="space-y-2">
        <li v-for="ex in examples" :key="ex" class="bg-elevated/50 rounded-lg px-3 py-2 text-sm">
          “{{ ex }}”
        </li>
      </ul>
    </section>

    <p v-if="data?.claudeParsing" class="text-muted flex items-center gap-2 text-sm">
      <UIcon name="i-lucide-sparkles" class="text-primary size-4" />
      Claude also cleans up text you paste on the Add page.
    </p>
  </div>
</template>
