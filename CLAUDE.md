# Crumb

Private recipe box: paste a recipe URL or text, scrape ingredients/instructions, store in SQLite, show it
in a clean UI. Also a remote MCP connector for Claude.

## Tech Stack

- **Server:** Rust (axum 0.8, tokio, rusqlite bundled, scraper, reqwest/rustls), crate `crumb` at the repo root
- **Frontend:** Astro 7 static build + Svelte 5 islands, in `web/`
- **Styling:** Tailwind CSS 4, "Green Tile" design language: semantic tokens and shared classes in
  `web/src/styles/app.css` (cream/paper/tint/line, tile green, one butter accent), no component library
- **Fonts:** self-hosted woff2 in `web/src/assets/fonts`: Nunito Sans (body), DM Serif Display (titles), Caveat
  (greetings and the cook's notes only), each with metric-matched fallbacks
- **Icons:** `lucide` via `web/src/components/Icon.astro` in `.astro` files, `@lucide/svelte` in `.svelte`
- **Database:** SQLite (WAL). Schema is raw SQL in `src/db.rs`, created/upgraded on start
- **Scraping:** `wreq` with Firefox then Safari browser fingerprints (reqwest for APIs and the image fallback), JSON-LD first,
  HTML/microdata fallback, headless Chromium over CDP for sites that still block or need JavaScript
- **AI ("Wee Chef"):** on whenever an Anthropic, OpenAI or DeepSeek key is set (`src/llm.rs`, structured JSON output); parses pasted text, writes "Try next" blurbs and, about one day in three, one recipe idea not in the box. User-facing text always says "Wee Chef", never the provider (Claude is only named for the MCP connector). `SUGGESTIONS_AI=off` is the only opt-out (Try next only). Without a key, the heuristic parser and the plain algorithm are used
- **Deploy:** Railway, `Dockerfile` (Astro build → Rust build → debian-slim runtime with Chromium). GitHub
  Actions build one GHCR image per master commit and deploy it to Railway `dev`; the Promote workflow retags it
  for `stable` (Railway `production`). The Linux desktop app (`desktop/linux`, Qt6/QML via cxx-qt, built in an
  `archlinux:latest` container) rides the same version: each master push uploads a `crumb-desktop-linux-<version>`
  tarball artifact, and Promote attaches the tarball, its `.sha256` and an AUR `PKGBUILD` to the GitHub Release.
  See `docs/RELEASING.md`; versions come from conventional commit messages
- **Errors/tracing:** Sentry, opt-in via `SENTRY_DSN` (`src/telemetry.rs`, `web/src/lib/sentry*.ts`)

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
crates/crumb-core/  # Pure logic shared by the server and the desktop app (model, fractions, categories, text_parser, markdown, suggest)
src/
  main.rs         # Boot: config, DB, browser, listen
  lib.rs          # AppState, router, layers
  config.rs       # Env vars (APP_PASSWORD, ANTHROPIC_/OPENAI_/DEEPSEEK_*, TYPESAFE_*, SITE_URL, WEB_DIST, ...; NUXT_* fallbacks)
  db.rs           # Path resolution, pragmas, bootstrap + legacy upgrades
  recipes.rs      # Service layer shared by REST API and MCP
  api.rs          # /api/** handlers
  web.rs          # Static files + page-data injection for dynamic pages
  auth.rs         # Password login, signed session cookie, auth middleware
  oauth.rs        # OAuth 2.1 (DCR, PKCE) for the Claude connector, /.well-known/*
  mcp.rs          # MCP Streamable HTTP (stateless JSON-RPC) at /mcp
  suggestions.rs  # Their service: DB inputs, time zone cookies, cached background AI re-rank
  checks.rs       # Import clean-up (tidy) + Wee Chef's background Jev check: fixes, flags, Undo
  images.rs       # /img resizer (WebP, disk cache), hero preload Link header
  telemetry.rs    # Sentry: init, scrubbing, request transactions, browser Server-Timing hint
  scraper.rs, importers.rs, llm.rs, browser.rs
tests/api.rs      # Router integration tests against a temp DB
web/src/
  layouts/Layout.astro   # Head, fonts, theme + transition boot scripts, nav rail / tab bar, timer dock
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
- **Four radii only:** `rounded-ui` (16px) cards/containers/menus, `rounded-ctl` (12px) controls and photos,
  `rounded-full` pills and circles, `3px 3px 0 0` book spines. Flat: 1px `border-line`, shadows only on menus.
- **Colour roles:** `text-primary` is text-weight green; fills behind text use `bg-tile text-on-tile`. Butter
  (`.btn-primary`) is only the one main action and the active nav item. Nothing under 13px.
- **Theme:** `web/src/lib/theme-boot.js` is inlined in every head: light, dark, system, or sun (dark from sunset to
  sunrise, from a saved location or a time-zone estimate). `window.crumbTheme` drives the More page setting.
- **Recipe photos:** always via `Photo.svelte` → `/img/{id}/{width}?v={fnv1a(image)}` (`src/images.rs` resizes to
  WebP and caches in `img-cache/` beside the DB). A card photo morphs into the recipe hero
  (`web/src/lib/transitions-boot.js`, `data-photo` / `data-photo-hero`).
- **Parallel builds:** `CRUMB_OUT_DIR=./dist-name bun run build` writes to its own folder (git-ignored).
- **DB compatibility:** timestamps are unix seconds, JSON columns are text; the production DB on the
  Railway volume must keep working.
- **API parity:** JSON is camelCase; errors are `{statusCode, statusMessage, message}`.
- **Deduplication:** saving a URL that already exists returns the existing recipe (`isNew: false`).
- **Cook/view log:** `recipe_events` (`viewed`/`cooked`, deduped within 30 min / 6 h). Views are pruned after 400
  days; cooks are kept and go into backups as `cookedAt`. Anything that logs a view must run inside `whenActive`.
- **Sentry:** the browser SDK gets its DSN, environment and release from a `Server-Timing` header the server adds
  to HTML responses, so there is no build-time DSN and one image serves dev and stable. The SDK loads after the
  page is idle; keep it off the critical path. Never attach recipe contents, bodies, query strings or cookies.
- **Anything with a side effect on GET** (like `/random`) must be excluded from `speculation-rules.json` and marked
  `data-no-prerender`, or hovering the link runs it.

## Code Style

- Rust: `cargo fmt`, clippy clean with `-D warnings`
- Web: oxfmt (no semicolons, double quotes, 2-space indent, 100 char width); oxlint correctness=error
