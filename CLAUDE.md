# Recipe App

"Just the Recipe" clone: paste a recipe URL or recipe text, store it in SQLite, show it in a clean UI.
Also a remote MCP server so the app can be added to Claude as a custom connector. Deploys to Railway.

## Tech Stack

- **Framework:** Nuxt 4 (Vue 3, Nitro)
- **UI:** Nuxt UI v4 (Tailwind CSS 4), system fonts only
- **Database:** SQLite via better-sqlite3 + Drizzle ORM
- **Scraping:** Cheerio with JSON-LD first, microdata/HTML fallback
- **Text import:** heuristic parser (`server/lib/text-parser.ts`), optional Claude extraction via the Anthropic SDK
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
    index.vue           # Add: one box for a URL or pasted text, plus recent recipes
    login.vue           # Password screen (layout: bare)
    connect.vue         # Claude connector instructions
    recipes/index.vue   # Library: server-side search, category filter, bulk actions
    recipes/[id].vue    # Recipe view (cook mode, checklists) + edit
    recipes/new.vue     # Manual entry
    cookbooks/          # Cookbook list + detail
  components/           # RecipeCard, RecipeGrid, RecipeEditor, CookbookCard, EmptyState
shared/utils/
  recipe.ts             # Zod schemas, RecipeSection type, normalizeSections
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
    scraper.ts          # URL scraper
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
- **Deduplication:** a URL that already exists returns the existing recipe. `url` is nullable for pasted recipes.
- **Auth:** `NUXT_APP_PASSWORD` gates the UI (sealed cookie) and the OAuth consent screen. No password = open (dev).
- **OAuth tokens** are stored as SHA-256 hashes in `oauth_tokens`; codes are single-use with PKCE S256.

## Code Style

- oxfmt: no semicolons, double quotes, 2-space indent, 100 char width
- oxlint: correctness=error, suspicious=warn, perf=warn
