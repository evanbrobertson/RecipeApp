# Recipe App

"Just the Recipe" clone: paste a recipe URL or recipe text, store it in SQLite, show it in a clean UI.
Also a remote MCP server so the app can be added to Claude as a custom connector. Deploys to Railway.

## Tech Stack

- **Framework:** Nuxt 4 (Vue 3, Nitro)
- **UI:** Nuxt UI v4 (Tailwind CSS 4), warm "tomato" theme in `app/assets/css/main.css`
- **Database:** SQLite via better-sqlite3 + Drizzle ORM
- **Scraping:** Cheerio with JSON-LD first, microdata/HTML fallback
- **Text import:** heuristic parser (`server/lib/text-parser.ts`), optional Claude extraction via the Anthropic SDK
- **Scraping fallback:** headless Chromium via `playwright-core` (`server/lib/browser.ts`), Chromium from apt in Docker
- **File import:** `unpdf` (PDF text), `fflate` (zip/gzip for Paprika) in `server/lib/importers.ts`
- **Fonts:** Fraunces (headings) + Figtree (body), self-hosted via `@fontsource-variable`
- **Claude connector:** `@modelcontextprotocol/sdk` Streamable HTTP (stateless) + a minimal OAuth 2.1 server
- **Linting/Formatting:** oxlint + oxfmt (not ESLint/Prettier)

## Commands

```bash
bun run dev          # Start dev server (port 3214)
bun run build        # Production build
bun run start        # Run the production build
bun run lint         # Lint with oxlint
bun run format       # Format with oxfmt
bun run checks       # Format check + lint + typecheck
```

## Project Structure

```
app/
  pages/
    index.vue             # Kitchen: greeting, add box, recently viewed, shelf, recent recipes
    add.vue               # Add a recipe (URL / text / many links)
    import.vue            # Bulk links + file uploads (JTR PDFs, Paprika, JSON, HTML, text, zip)
    more.vue              # Hub: connect, import, backup, appearance, sign out
    login.vue             # Password screen (layout: bare)
    connect.vue           # Claude connector instructions
    recipes/index.vue     # Library: server-side search, category chips, bulk actions
    recipes/[id]/index.vue  # Recipe view (scaling, checklists) + edit
    recipes/[id]/cook.vue   # Cook mode (layout: false): wake lock, steps, timers
    recipes/[id]/prep.vue   # Mise en place: bowls sized by quantity
    recipes/new.vue       # Manual entry
    cookbooks/index.vue   # The 3D shelf
    cookbooks/[id].vue    # Cookbook detail (colour, rename, remove)
  components/
    Bookshelf.vue, CookbookSpine.vue, OpenBook.vue   # shelf + opening book (CSS 3D)
    PrepBowl.vue          # SVG bowls/boards/jars
    AddRecipeBox.vue, RecipeCard.vue, RecipeGrid.vue, RecipeEditor.vue, ScaleControl.vue, TimerDock.vue
  composables/            # useRecipe, useKitchenTimers, useScreenAwake, useRecentlyViewed
  utils/                  # books (palette/sizes), ingredientColor, errors
shared/utils/
  recipe.ts             # Zod schemas, RecipeSection type, normalizeSections, BOOK_COLORS
  ingredients.ts        # Ingredient parser, scaling, mise en place vessels, timer detection
  format.ts             # recipeToMarkdown (copy/share + MCP)
server/
  api/                  # REST routes (thin wrappers over server/lib/recipes.ts)
  routes/mcp.ts         # POST /mcp — Claude connector endpoint (bearer token required)
  routes/oauth/         # authorize (consent page), token, register (DCR), revoke
  handlers/             # /.well-known OAuth metadata (registered via serverHandlers in nuxt.config)
  middleware/auth.ts    # Password gate for pages and /api
  utils/                # auth (sessions), oauth (tokens, PKCE), params
  lib/
    recipes.ts          # Service layer shared by REST + MCP
    scraper.ts          # URL scraper (fetch → headless browser fallback), parseRecipeHtml, recipeFromJsonLd
    browser.ts          # Headless Chromium page loader (one at a time)
    importers.ts        # File importers
    text-parser.ts      # Pasted-text parser
    claude-extract.ts   # Optional Claude structured extraction
    mcp.ts              # MCP tool definitions
  database/
    schema.ts           # Drizzle schema (recipes, cookbooks, cookbook_recipes, oauth_*)
    index.ts            # useDB() singleton, bootstrap SQL, legacy schema upgrade
```

## Key Patterns

- **Service layer:** REST routes and MCP tools both call `server/lib/recipes.ts`; keep logic there.
- **Zod validation** on API inputs via `readValidatedBody`. Use `recipePatchSchema` for updates (no defaults).
- **DB singleton:** `useDB()` creates tables on first use; no migration step at deploy time.
- **DB path:** `DATABASE_PATH` → `$RAILWAY_VOLUME_MOUNT_PATH/recipes.db` → `.data/recipes.db`.
- **JSON columns:** `ingredients` and `instructions` are `RecipeSection[]` (`{ name, items }`) as JSON text.
- **Timers/scale/prep state** live in `useState` so they survive moving between view, cook and prep pages.
- **Hydration:** anything reading localStorage, the clock or browser APIs (wake lock) must be client-only.
- **3D shelf:** only `.spine` takes pointer events; side faces are decorative.
- **Deduplication:** a URL that already exists returns the existing recipe. `url` is nullable for pasted recipes.
- **Auth:** `NUXT_APP_PASSWORD` gates the UI (sealed cookie) and the OAuth consent screen. No password = open (dev).
- **OAuth tokens** are stored as SHA-256 hashes in `oauth_tokens`; codes are single-use with PKCE S256.

## Code Style

- oxfmt: no semicolons, double quotes, 2-space indent, 100 char width
- oxlint: correctness=error, suspicious=warn, perf=warn
