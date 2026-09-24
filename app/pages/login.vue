<script setup lang="ts">
definePageMeta({ layout: "bare" })
useHead({ title: "Sign in · Just the Recipe" })

const route = useRoute()
const password = ref("")
const loading = ref(false)
const error = ref("")

async function submit() {
  loading.value = true
  error.value = ""
  try {
    await $fetch("/api/auth/login", { method: "POST", body: { password: password.value } })
    const target = typeof route.query.next === "string" ? route.query.next : "/"
    // Only allow same-origin paths ("//host" would be an open redirect)
    const next = target.startsWith("/") && !target.startsWith("//") ? target : "/"
    // Full reload so the server renders the protected page with the new cookie
    window.location.href = next
  } catch (e) {
    error.value = errorMessage(e, "Couldn't sign in")
    loading.value = false
  }
}
</script>

<template>
  <div class="w-full max-w-sm">
    <div class="mb-6 flex flex-col items-center text-center">
      <UIcon name="i-lucide-chef-hat" class="text-primary size-10" />
      <h1 class="mt-2 font-serif text-2xl font-semibold">Just the Recipe</h1>
      <p class="text-muted text-sm">Enter your password to open your recipe box.</p>
    </div>
    <form class="space-y-4" @submit.prevent="submit">
      <UFormField label="Password" :error="error || undefined">
        <UInput
          v-model="password"
          type="password"
          autocomplete="current-password"
          size="lg"
          class="w-full"
          autofocus
          required
        />
      </UFormField>
      <UButton type="submit" label="Sign in" block size="lg" :loading="loading" />
    </form>
  </div>
</template>
