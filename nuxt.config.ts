// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: "2025-07-15",

  devServer: { port: 3214 },

  devtools: { enabled: false },

  modules: ["@nuxt/ui", "@vueuse/nuxt"],

  css: ["~/assets/css/main.css"],

  // System fonts only: no web font downloads at build or run time
  ui: { fonts: false },

  icon: {
    // Bundle only the icons we use so nothing is fetched from the Iconify API at runtime
    serverBundle: "local",
    clientBundle: { scan: true },
  },

  app: {
    head: {
      htmlAttrs: { lang: "en" },
      title: "Just the Recipe",
      meta: [
        { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" },
        { name: "description", content: "Your recipes, without the life story." },
        { name: "theme-color", content: "#16a34a" },
        { name: "robots", content: "noindex, nofollow" },
      ],
      link: [
        { rel: "icon", href: "/favicon.ico" },
        { rel: "manifest", href: "/manifest.webmanifest" },
      ],
    },
  },

  runtimeConfig: {
    // Password for the web UI and the Claude connector. Empty = no auth (local dev only).
    appPassword: "",
    // Optional: enables Claude-powered parsing of pasted text (falls back to the built-in parser)
    anthropicApiKey: "",
    anthropicModel: "claude-opus-5",
    public: {
      siteUrl: "",
    },
  },

  // Nitro doesn't scan dot-directories, so OAuth discovery routes are registered here
  serverHandlers: [
    {
      route: "/.well-known/oauth-authorization-server",
      handler: "~~/server/handlers/oauth-authorization-server.ts",
    },
    {
      route: "/.well-known/oauth-protected-resource",
      handler: "~~/server/handlers/oauth-protected-resource.ts",
    },
    {
      route: "/.well-known/oauth-protected-resource/mcp",
      handler: "~~/server/handlers/oauth-protected-resource.ts",
    },
  ],

  nitro: {
    compressPublicAssets: true,
    routeRules: {
      "/_nuxt/**": { headers: { "cache-control": "public, max-age=31536000, immutable" } },
    },
  },

  experimental: {
    payloadExtraction: false,
  },
})
