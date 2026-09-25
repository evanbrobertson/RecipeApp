<script setup lang="ts">
useHead({ title: "Add a recipe · Just the Recipe" })

const tips = [
  {
    icon: "i-lucide-link",
    title: "A link",
    text: "Most recipe sites work. If one blocks us, a real browser has a go.",
  },
  {
    icon: "i-lucide-clipboard-type",
    title: "Any recipe text",
    text: "From an email, notes, a PDF or a photo's text. Headings help but aren't required.",
  },
  {
    icon: "i-lucide-list",
    title: "Lots of links",
    text: "Paste several at once and they're imported one after another.",
  },
]
</script>

<template>
  <div class="mx-auto max-w-2xl">
    <h1 class="font-serif text-3xl font-semibold sm:text-4xl">Add a recipe</h1>
    <p class="text-muted mt-1">Paste a link or the recipe itself.</p>
    <AddRecipeBox class="mt-5" autofocus />

    <div class="mt-8 grid gap-3 sm:grid-cols-3">
      <div v-for="tip in tips" :key="tip.title" class="bg-elevated/50 rounded-2xl p-4">
        <UIcon :name="tip.icon" class="text-primary size-5" />
        <p class="mt-2 text-sm font-semibold">{{ tip.title }}</p>
        <p class="text-muted mt-0.5 text-sm">{{ tip.text }}</p>
      </div>
    </div>

    <div class="mt-6 grid gap-3 sm:grid-cols-2">
      <NuxtLink
        to="/recipes/new"
        class="border-default hover:border-primary flex items-center gap-3 rounded-2xl border p-4 transition"
      >
        <UIcon name="i-lucide-pen-line" class="text-primary size-6" />
        <span>
          <span class="block font-semibold">Write from scratch</span>
          <span class="text-muted text-sm">Grandma's card, your own creation</span>
        </span>
      </NuxtLink>
      <NuxtLink
        to="/import"
        class="border-default hover:border-primary flex items-center gap-3 rounded-2xl border p-4 transition"
      >
        <UIcon name="i-lucide-file-down" class="text-primary size-6" />
        <span>
          <span class="block font-semibold">Import from another app</span>
          <span class="text-muted text-sm">Just the Recipe, Paprika, Mealie…</span>
        </span>
      </NuxtLink>
    </div>
  </div>
</template>
