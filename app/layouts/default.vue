<script setup lang="ts">
const route = useRoute()

const links = [
  { to: "/", label: "Add", icon: "i-lucide-plus" },
  { to: "/recipes", label: "Recipes", icon: "i-lucide-book-open" },
  { to: "/cookbooks", label: "Cookbooks", icon: "i-lucide-library" },
  { to: "/connect", label: "Claude", icon: "i-lucide-plug" },
]

function isActive(to: string) {
  return to === "/" ? route.path === "/" : route.path.startsWith(to)
}
</script>

<template>
  <div class="bg-default min-h-dvh">
    <header
      class="border-default bg-default/80 supports-[backdrop-filter]:bg-default/70 sticky top-0 z-40 border-b backdrop-blur"
    >
      <div class="mx-auto flex h-14 max-w-5xl items-center justify-between gap-2 px-4">
        <NuxtLink to="/" class="flex items-center gap-2 font-serif text-lg font-semibold">
          <UIcon name="i-lucide-chef-hat" class="text-primary size-6" />
          <span>Just the Recipe</span>
        </NuxtLink>
        <nav class="hidden items-center gap-1 sm:flex">
          <UButton
            v-for="link in links"
            :key="link.to"
            :to="link.to"
            :label="link.label"
            :icon="link.icon"
            :variant="isActive(link.to) ? 'soft' : 'ghost'"
            :color="isActive(link.to) ? 'primary' : 'neutral'"
            size="sm"
          />
        </nav>
      </div>
    </header>

    <main class="mx-auto max-w-5xl px-4 pt-6 pb-28 sm:pt-10 sm:pb-16">
      <slot />
    </main>

    <!-- Mobile tab bar -->
    <nav
      class="border-default bg-default/95 fixed inset-x-0 bottom-0 z-40 grid grid-cols-4 border-t pb-[env(safe-area-inset-bottom)] backdrop-blur sm:hidden"
    >
      <NuxtLink
        v-for="link in links"
        :key="link.to"
        :to="link.to"
        class="flex flex-col items-center gap-0.5 py-2 text-xs"
        :class="isActive(link.to) ? 'text-primary' : 'text-muted'"
      >
        <UIcon :name="link.icon" class="size-5" />
        {{ link.label }}
      </NuxtLink>
    </nav>
  </div>
</template>
