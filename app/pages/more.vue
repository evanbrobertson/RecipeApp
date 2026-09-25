<script setup lang="ts">
useHead({ title: "More · Just the Recipe" })

const { data: info } = useFetch("/api/connector")
const colorMode = useColorMode()

const modes = [
  { value: "light", icon: "i-lucide-sun", label: "Light" },
  { value: "dark", icon: "i-lucide-moon", label: "Dark" },
  { value: "system", icon: "i-lucide-monitor", label: "Auto" },
]

async function signOut() {
  await $fetch("/api/auth/logout", { method: "POST" })
  window.location.href = "/login"
}

const links = [
  {
    to: "/connect",
    icon: "i-lucide-plug",
    title: "Connect to Claude",
    text: "Save, search and tweak recipes from any Claude chat",
  },
  {
    to: "/import",
    icon: "i-lucide-file-down",
    title: "Import recipes",
    text: "From Just the Recipe, Paprika, Mealie, files or links",
  },
  {
    to: "/recipes/new",
    icon: "i-lucide-pen-line",
    title: "Write a recipe",
    text: "Type one in from scratch",
  },
]
</script>

<template>
  <div class="mx-auto max-w-2xl space-y-8">
    <h1 class="font-serif text-3xl font-semibold sm:text-4xl">More</h1>

    <div class="space-y-2">
      <NuxtLink
        v-for="l in links"
        :key="l.to"
        :to="l.to"
        class="bg-elevated/50 hover:bg-elevated flex items-center gap-4 rounded-2xl p-4 transition"
      >
        <span class="bg-primary/10 text-primary grid size-11 place-items-center rounded-xl">
          <UIcon :name="l.icon" class="size-5" />
        </span>
        <span class="min-w-0 flex-1">
          <span class="block font-semibold">{{ l.title }}</span>
          <span class="text-muted text-sm">{{ l.text }}</span>
        </span>
        <UIcon name="i-lucide-chevron-right" class="text-dimmed size-5" />
      </NuxtLink>
      <a
        href="/api/export"
        download
        class="bg-elevated/50 hover:bg-elevated flex items-center gap-4 rounded-2xl p-4 transition"
      >
        <span class="bg-primary/10 text-primary grid size-11 place-items-center rounded-xl">
          <UIcon name="i-lucide-archive" class="size-5" />
        </span>
        <span class="min-w-0 flex-1">
          <span class="block font-semibold">Download a backup</span>
          <span class="text-muted text-sm"
            >All recipes and cookbooks as one JSON file (re-import it any time)</span
          >
        </span>
        <UIcon name="i-lucide-download" class="text-dimmed size-5" />
      </a>
    </div>

    <section>
      <h2 class="mb-2 font-semibold">Appearance</h2>
      <div class="bg-elevated inline-flex rounded-full p-1">
        <ClientOnly>
          <button
            v-for="m in modes"
            :key="m.value"
            type="button"
            class="flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm font-semibold transition"
            :class="
              colorMode.preference === m.value ? 'bg-default text-primary shadow-sm' : 'text-muted'
            "
            @click="
              () => {
                colorMode.preference = m.value
              }
            "
          >
            <UIcon :name="m.icon" class="size-4" />{{ m.label }}
          </button>
        </ClientOnly>
      </div>
    </section>

    <section class="text-muted space-y-1 text-sm">
      <p v-if="info">
        <UIcon name="i-lucide-globe" class="mr-1 size-4 align-[-3px]" />
        Tricky sites:
        {{
          info.browserScraping
            ? "a real browser steps in when a site blocks us"
            : "browser fallback not installed"
        }}
      </p>
      <p v-if="info">
        <UIcon name="i-lucide-sparkles" class="mr-1 size-4 align-[-3px]" />
        Pasted text: {{ info.claudeParsing ? "cleaned up by Claude" : "built-in parser" }}
      </p>
    </section>

    <UButton
      v-if="info?.authEnabled"
      label="Sign out"
      icon="i-lucide-log-out"
      variant="ghost"
      color="neutral"
      @click="signOut"
    />
  </div>
</template>
