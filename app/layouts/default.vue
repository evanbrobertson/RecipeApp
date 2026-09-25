<script setup lang="ts">
const route = useRoute()

const links = [
  { to: "/", label: "Kitchen", icon: "i-lucide-house" },
  { to: "/recipes", label: "Recipes", icon: "i-lucide-book-open-text" },
  { to: "/cookbooks", label: "Shelf", icon: "i-lucide-library-big" },
  { to: "/more", label: "More", icon: "i-lucide-ellipsis" },
]

function isActive(to: string) {
  if (to === "/") return route.path === "/"
  if (to === "/more") return ["/more", "/connect", "/import"].includes(route.path)
  return route.path.startsWith(to)
}

const colorMode = useColorMode()
const isDark = computed({
  get: () => colorMode.value === "dark",
  set: (v) => (colorMode.preference = v ? "dark" : "light"),
})
</script>

<template>
  <div class="bg-default min-h-dvh">
    <header class="bg-default/85 border-default sticky top-0 z-40 border-b backdrop-blur-md">
      <div class="mx-auto flex h-14 max-w-6xl items-center justify-between gap-3 px-4">
        <NuxtLink to="/" class="group flex items-center gap-2">
          <span
            class="bg-primary grid size-8 place-items-center rounded-xl text-white shadow-sm transition group-hover:-rotate-6"
          >
            <UIcon name="i-lucide-chef-hat" class="size-5" />
          </span>
          <span class="font-serif text-lg font-semibold tracking-tight">Just the Recipe</span>
        </NuxtLink>

        <nav class="hidden items-center gap-1 md:flex">
          <UButton
            v-for="link in links"
            :key="link.to"
            :to="link.to"
            :label="link.label"
            :icon="link.icon"
            variant="ghost"
            :color="isActive(link.to) ? 'primary' : 'neutral'"
            :class="isActive(link.to) ? 'bg-primary/10' : ''"
          />
          <UButton to="/add" label="Add recipe" icon="i-lucide-plus" class="ml-2 rounded-full" />
        </nav>

        <ClientOnly>
          <UButton
            :icon="isDark ? 'i-lucide-sun' : 'i-lucide-moon'"
            variant="ghost"
            color="neutral"
            aria-label="Toggle dark mode"
            class="md:hidden"
            @click="
              () => {
                isDark = !isDark
              }
            "
          />
        </ClientOnly>
      </div>
    </header>

    <main class="mx-auto max-w-6xl px-4 pt-5 pb-32 sm:pt-8 md:pb-16">
      <slot />
    </main>

    <!-- Kitchen timers keep ticking on every page -->
    <div class="pointer-events-none fixed inset-x-0 top-16 z-50 px-4">
      <TimerDock />
    </div>

    <!-- Mobile tab bar with a big thumb-reachable add button -->
    <nav
      class="bg-default/95 border-default fixed inset-x-0 bottom-0 z-40 border-t pb-[env(safe-area-inset-bottom)] backdrop-blur-md md:hidden"
    >
      <div class="relative grid grid-cols-5 items-end">
        <template v-for="(link, i) in links" :key="link.to">
          <NuxtLink
            :to="link.to"
            class="flex flex-col items-center gap-0.5 pt-2 pb-2 text-[11px] font-medium transition"
            :class="isActive(link.to) ? 'text-primary' : 'text-muted'"
            :style="i >= 2 ? { gridColumnStart: i + 2 } : undefined"
          >
            <UIcon :name="link.icon" class="size-6" />
            {{ link.label }}
          </NuxtLink>
        </template>
        <NuxtLink
          to="/add"
          aria-label="Add recipe"
          class="bg-primary shadow-primary/30 absolute -top-5 left-1/2 grid size-14 -translate-x-1/2 place-items-center rounded-2xl text-white shadow-lg ring-4 ring-(--ui-bg) transition active:scale-95"
        >
          <UIcon name="i-lucide-plus" class="size-7" />
        </NuxtLink>
      </div>
    </nav>
  </div>
</template>
