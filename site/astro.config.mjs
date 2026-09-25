import { defineConfig } from "astro/config"

/**
 * A fully static marketing page. On GitHub Pages it lives under the project path,
 * so the base defaults to /RecipeApp/; set SITE_BASE=/ for a root or custom domain.
 */
const base = process.env.SITE_BASE ?? "/RecipeApp/"

export default defineConfig({
  output: "static",
  site: process.env.SITE_URL || undefined,
  base,
  trailingSlash: "ignore",
  build: {
    // One HTML file, no render-blocking stylesheet request
    inlineStylesheets: "always",
    assets: "assets",
  },
  compressHTML: true,
  devToolbar: { enabled: false },
})
