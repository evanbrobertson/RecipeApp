# Crumb

Private recipe box: paste a recipe URL or text, scrape ingredients/instructions, store in SQLite, show it
in a clean UI. Also a remote MCP connector for Claude.

## Tech Stack

- **Server:** Rust (axum 0.8, tokio, rusqlite bundled, scraper, reqwest/rustls), crate `crumb` at the repo root
- **Frontend:** Astro 7 static build + Svelte 5 islands, in `web/`
- **Styling:** Tailwind CSS 4 with semantic tokens (`web/src/styles/app.css`), no component library
- **Icons:** `lucide` via `web/src/components/Icon.astro` in `.astro` files, `@lucide/svelte` in `.svelte`
- **Database:** SQLite (WAL). Schema is raw SQL in `src/db.rs`, created/upgraded on start
- **Scraping:** JSON-LD first, HTML/microdata fallback, headless Chromium over CDP for blocked sites
- **AI (optional):** Anthropic, OpenAI or DeepSeek (`src/llm.rs`, structured JSON output) parses pasted text and writes "Try next" blurbs; without a key, the heuristic parser and the plain algorithm are used
- **Deploy:** Railway, `Dockerfile` (Astro build → Rust build → debian-slim runtime with Chromium)

## Commands

```bash
cargo run                        # Server on :3000, serves web/dist
cargo test                       # Unit + integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt

cd web
bun run build                    # Static build into web/dist (+ .br/.gz siblings)
bun run dev                      # Astro dev server on :4321, proxies /api to :3000
bun run check                    # astro check (TS + Svelte)
bun run lint                     # oxlint
bun run format                   # oxfmt
```

## Project Structure

```
src/
  main.rs         # Boot: config, DB, browser, listen
  lib.rs          # AppState, router, layers
  config.rs       # Env vars (APP_PASSWORD, ANTHROPIC_/OPENAI_/DEEPSEEK_*, SITE_URL, WEB_DIST, ...; NUXT_* fallbacks)
  db.rs           # Path resolution, pragmas, bootstrap + legacy upgrades
  model.rs        # Recipe/cookbook types, validation, normalize_sections
  recipes.rs      # Service layer shared by REST API and MCP
  api.rs          # /api/** handlers
  web.rs          # Static files + page-data injection for dynamic pages
  auth.rs         # Password login, signed session cookie, auth middleware
  oauth.rs        # OAuth 2.1 (DCR, PKCE) for the Claude connector, /.well-known/*
  mcp.rs          # MCP Streamable HTTP (stateless JSON-RPC) at /mcp
  suggest.rs      # Try next ranking + Surprise me (pure, unit-tested)
  suggestions.rs  # Their service: DB inputs, time zone cookies, cached background AI re-rank
  scraper.rs, text_parser.rs, importers.rs, llm.rs, browser.rs, markdown.rs
tests/api.rs      # Router integration tests against a temp DB
web/src/
  layouts/Layout.astro   # Head, fonts, header, tab bar, timer dock, theme
  pages/                 # Static pages; pages/shell/* are templates for dynamic routes
  islands/               # Svelte islands (one per interactive page/section)
  components/            # Shared Svelte + Astro components
  lib/                   # api, storage, toast, timers, ingredients (scaling/mise), books
```

## Key Patterns

- **Page data:** templates contain `<script type="application/json" id="page-data">null</script>`; the
  server replaces `null` with JSON for that route (table in `src/web.rs`). Islands read it with
  `pageState()` (`web/src/lib/page.svelte.ts`) and only fetch when it's missing (e.g. `astro dev`).
- **Dynamic routes:** `/recipes/:id{,/cook,/prep,/edit}` and `/cookbooks/:id` are served from
  `web/dist/shell/*/index.html`. Direct `/shell/*` requests 404.
- **Templates are cached in memory at startup** — restart the server after rebuilding `web/`.
- **Every navigation is a full page load.** Cross-page state lives in storage: scale, cook step and prep
  progress in `sessionStorage`; timers, recently viewed and theme in `localStorage`. Use `flash()` for a
  toast that should appear after navigating.
- **Islands:** `client:load` for data-independent UI (SSR'd at build), `client:only="svelte"` for anything
  reading page data, with a skeleton `slot="fallback"`.
- **One corner radius everywhere:** `--radius` / `rounded-ui` (1rem). Circles use `rounded-full`.
- **DB compatibility:** timestamps are unix seconds, JSON columns are text; the production DB on the
  Railway volume must keep working.
- **API parity:** JSON is camelCase; errors are `{statusCode, statusMessage, message}`.
- **Deduplication:** saving a URL that already exists returns the existing recipe (`isNew: false`).
- **Cook/view log:** `recipe_events` (`viewed`/`cooked`, deduped within 30 min / 6 h). Views are pruned after 400
  days; cooks are kept and go into backups as `cookedAt`. Anything that logs a view must run inside `whenActive`.
- **Anything with a side effect on GET** (like `/random`) must be excluded from `speculation-rules.json` and marked
  `data-no-prerender`, or hovering the link runs it.

## Code Style

- Rust: `cargo fmt`, clippy clean with `-D warnings`
- Web: oxfmt (no semicolons, double quotes, 2-space indent, 100 char width); oxlint correctness=error
