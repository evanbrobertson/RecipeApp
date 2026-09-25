<script setup lang="ts">
useHead({ title: "Kitchen · Just the Recipe" })

// Greeting uses the viewer's clock, so it's set after hydration (the server runs in UTC)
const greeting = ref("Hello")
const prompt = ref("What's cooking?")
onMounted(() => {
  const hour = new Date().getHours()
  if (hour < 5) [greeting.value, prompt.value] = ["Still up?", "Midnight snack?"]
  else if (hour < 11) [greeting.value, prompt.value] = ["Good morning", "What's for breakfast?"]
  else if (hour < 15) [greeting.value, prompt.value] = ["Good afternoon", "What's for lunch?"]
  else if (hour < 18) [greeting.value, prompt.value] = ["Good afternoon", "What's cooking tonight?"]
  else [greeting.value, prompt.value] = ["Good evening", "What's cooking tonight?"]
})

const { list: recentlyViewed } = useRecentlyViewed()
const { data: recent, status: recentStatus } = useFetch("/api/recipes", {
  query: { limit: 8 },
  key: "recent-recipes",
})
const { data: books } = useFetch("/api/cookbooks", { key: "cookbooks", lazy: true })

const opened = ref<ShelfBook | null>(null)
</script>

<template>
  <div class="space-y-12">
    <section class="mx-auto max-w-2xl pt-2 text-center sm:pt-6">
      <p class="text-primary font-semibold">{{ greeting }}</p>
      <h1 class="mt-1 font-serif text-4xl font-semibold tracking-tight text-balance sm:text-5xl">
        {{ prompt }}
      </h1>
      <p class="text-muted mt-3 sm:text-lg">
        Paste a link or the recipe itself. You get the ingredients and the steps, and none of the
        life story.
      </p>
      <AddRecipeBox class="mt-6 text-left" compact />
      <div class="text-muted mt-3 flex flex-wrap justify-center gap-x-4 gap-y-1 text-sm">
        <NuxtLink to="/recipes/new" class="hover:text-primary">Write one from scratch</NuxtLink>
        <NuxtLink to="/import" class="hover:text-primary">Import from another app</NuxtLink>
        <NuxtLink to="/connect" class="hover:text-primary">Save from Claude</NuxtLink>
      </div>
    </section>

    <ClientOnly>
      <section v-if="recentlyViewed.length">
        <h2 class="mb-3 font-serif text-xl font-semibold">Pick up where you left off</h2>
        <div class="no-scrollbar -mx-4 flex gap-3 overflow-x-auto px-4 pb-1">
          <NuxtLink
            v-for="r in recentlyViewed"
            :key="r.id"
            :to="`/recipes/${r.id}`"
            class="bg-elevated/60 hover:bg-elevated flex w-64 flex-none items-center gap-3 rounded-2xl p-2 pr-4 transition"
          >
            <img
              v-if="r.image"
              :src="r.image"
              alt=""
              referrerpolicy="no-referrer"
              loading="lazy"
              class="size-14 rounded-xl object-cover"
            />
            <span
              v-else
              class="bg-primary/10 text-primary grid size-14 place-items-center rounded-xl"
            >
              <UIcon name="i-lucide-cooking-pot" class="size-6" />
            </span>
            <span class="line-clamp-2 text-sm font-semibold">{{ r.title }}</span>
          </NuxtLink>
        </div>
      </section>
    </ClientOnly>

    <section v-if="books?.length">
      <div class="mb-2 flex items-center justify-between">
        <h2 class="font-serif text-xl font-semibold">Your shelf</h2>
        <UButton
          to="/cookbooks"
          label="All cookbooks"
          variant="link"
          trailing-icon="i-lucide-arrow-right"
        />
      </div>
      <div class="no-scrollbar -mx-4 overflow-x-auto px-4">
        <div class="min-w-max pt-6">
          <Bookshelf :books="books.slice(0, 14)" :pulled-id="opened?.id" @open="opened = $event" />
        </div>
      </div>
      <OpenBook :book="opened" @close="opened = null" />
    </section>

    <section v-if="recentStatus === 'pending' || recent?.length">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="font-serif text-xl font-semibold">Fresh in the box</h2>
        <UButton
          to="/recipes"
          label="All recipes"
          variant="link"
          trailing-icon="i-lucide-arrow-right"
        />
      </div>
      <RecipeGrid :loading="recentStatus === 'pending'" :count="4">
        <RecipeCard v-for="recipe in recent" :key="recipe.id" :recipe="recipe" />
      </RecipeGrid>
    </section>
  </div>
</template>
