# Crumb

Private recipe box: paste a recipe URL or text, scrape ingredients/instructions, store in SQLite, show it
in a clean UI. Also a remote MCP connector for Claude.

## Tech Stack

- **Server:** Rust (axum 0.8, tokio, rusqlite bundled, scraper, reqwest/rustls), crate `crumb` at the repo root
- **Shared crates (`crates/`):** `crumb-core` (pure logic: model, validation, fractions, categories, text parser,
  Markdown, Try next, ingredient scaling, mise en place and prep groups, timers, text export, sun clock, check
  wording, shelf geometry, the Add box, Home's wording, editor drafts and one-tap fixes),
  `crumb-client` (typed Rust client for the API), `crumb-ffi` (UniFFI bindings to crumb-core for Kotlin and Swift).
  `crumb-fetch` (the Firefox/Safari `wreq` profiles, shared by the server and the relay; plus the public-address guard the relay uses),
  `crumb-relay` (a small service on other networks that fetches pages the server's IP was blocked on; `docs/RELAY.md`;
  with Chromium or the video tools installed it also renders pages and watches videos for the server),
  `crumb-work` (the Chromium driver and the video tools' steps, shared by the server and the relay).
  Thin clients, fat server: the native apps call these instead of re-implementing logic
- **Frontend:** Astro 7 static build + Svelte 5 islands, in `web/`
- **Styling:** Tailwind CSS 4, "Green Tile" design language: semantic tokens and shared classes in
  `web/src/styles/app.css` (cream/paper/tint/line, tile green, one butter accent), no component library
- **Fonts:** self-hosted woff2 in `web/src/assets/fonts`: Nunito Sans (body), DM Serif Display (titles), Caveat
  (greetings and tips only), Kalam (the cook's notes), each with metric-matched fallbacks
- **Icons:** `lucide` via `web/src/components/Icon.astro` in `.astro` files, `@lucide/svelte` in `.svelte`
- **Database:** SQLite (WAL). Schema is raw SQL in `src/db.rs`, created/upgraded on start
- **Scraping:** `wreq` with Firefox then Safari browser fingerprints (reqwest for APIs and the image fallback), JSON-LD first,
  HTML/microdata fallback. Order per link (`scraper::scrape_plan`): site memory (`src/sites.rs`, `sites.db`: what worked on
  the host) → a recognised platform's API (WordPress: `src/scraper/platforms.rs`, `fallbacks.rs`) hedged with the page
  (Firefox, Safari if blocked; the API starts if the page hasn't answered in 300 ms, first recipe wins) → if both profiles
  were blocked: the API, then `crumb-relay`s over Tailscale (`src/relay.rs`, `SCRAPE_RELAYS`), with the Internet Archive's
  copy joining only if no relay answered within `RELAY_HEAD_START` (2 s; at once when there are no relays), since an
  archived copy can be stale → headless Chromium over CDP last, for sites that still block or need JavaScript (a relay's
  Chromium first when one says it has it in `/health`, `scraper::render_page`). Never several requests
  at one site at once bar that hedge; Chromium and videos never race. Every fetch of a link a cook supplies refuses private
  addresses and listed sites (`crumb_fetch::guard`, redirects and DNS too, and the terms-check and image `reqwest` clients; Chromium goes through `crumb_fetch::proxy`); the relay client is
  the one exception (Tailscale)
- **Videos:** TikTok / Instagram Reels / YouTube (videos and Shorts) links go to `src/video.rs`: the caption first, else `yt-dlp`
  download → local whisper.cpp transcript + `ffmpeg` stills → one Wee Chef vision call. A relay with the tools does the
  details and the watching (`/video/meta`, `/video/watch`); Wee Chef always runs on the server. Tools are in the Docker image
  (`video` stage; bump `YT_DLP_VERSION` when imports break) and optional everywhere else. Videos run as jobs in
  `src/video_jobs.rs`: `VIDEO_WORKERS` at once (a heavy-work budget Chromium shares), `VIDEO_QUEUE_MAX` waiting,
  429 past that; the web gets a job id and polls `/api/import/jobs/{id}`, MCP awaits the job. YouTube turns servers away, so
  the extension (`extension/src/youtube.ts`) reads a video's description and captions in the browser and the Add page
  posts them with the link as `video` (`video::FromBrowser`); never ask for or store anyone's YouTube cookies
- **AI ("Wee Chef"):** on whenever an Anthropic, OpenAI or DeepSeek key is set (`src/llm.rs`, structured JSON output); parses pasted text, writes "Try next" blurbs and, about one day in three, one recipe idea not in the box. User-facing text always says "Wee Chef", never the provider (Claude is only named for the MCP connector). `SUGGESTIONS_AI=off` is the only opt-out (Try next only). Without a key, the heuristic parser and the plain algorithm are used
- **Deploy:** Railway, `Dockerfile` (Astro build → Rust build → debian-slim runtime with Chromium). GitHub
  Actions build one GHCR image per master commit and deploy it to Railway `dev`; the Promote workflow retags it
  for `stable` (Railway `production`). See `docs/RELEASING.md`; versions come from conventional commit messages
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
crates/
  crumb-core/     # Pure logic shared by the server and every client; the server re-exports it (crumb::model, ...)
  crumb-client/   # Typed API client, tested against the real router
  crumb-ffi/      # UniFFI bindings (uniffi.toml: Kotlin package app.crumb.core, Swift module CrumbCore)
  crumb-fetch/    # Browser-profile fetching (wreq) + the SSRF guard (guard.rs) and Chromium's guarded proxy (proxy.rs); the server re-exports it in scraper.rs
  crumb-relay/    # Relay binary (axum): POST /fetch behind a bearer token, rate limits, and /render, /video/* as a worker; Dockerfile, systemd unit
  crumb-work/     # Headless Chromium (browser.rs) and the video tools (video.rs) for the server and the relay; wire.rs
src/
  main.rs         # Boot: config, DB, browser, listen
  lib.rs          # AppState, router, layers
  config.rs       # Env vars (APP_PASSWORD, ANTHROPIC_/OPENAI_/DEEPSEEK_*, TYPESAFE_*, SITE_URL, WEB_DIST, ...; NUXT_* fallbacks)
  db.rs           # Path resolution, pragmas, bootstrap + legacy upgrades
  households.rs   # One SQLite file per household; Scoped extractor gives a handler its household's state
  accounts.rs     # AUTH_MODE=accounts: accounts.db (users, households, invites, sessions, identities, share index); account_api.rs
  social.rs       # Sign in with Google / Apple for AUTH_MODE=accounts (OIDC code flow; hosted uses Better Auth's)
  hosted.rs       # AUTH_MODE=hosted: asks the Better Auth service (auth/) who's signed in, proxies /api/auth/*
  recipes.rs      # Service layer shared by REST API and MCP
  api.rs          # /api/** handlers
  web.rs          # Static files + page-data injection for dynamic pages
  auth.rs         # Password login, signed session cookie, auth middleware
  oauth.rs        # OAuth 2.1 (DCR, PKCE) for the Claude connector, /.well-known/*
  mcp.rs          # MCP Streamable HTTP (stateless JSON-RPC) at /mcp
  preview.rs      # /preview?url=: a page read and shown in the share layout, not saved until "Add to my Crumb"
  suggestions.rs  # Their service: DB inputs, time zone cookies, cached background AI re-rank
  checks.rs       # Import clean-up (tidy) + Wee Chef's background Jev check: fixes, flags, Undo
  sites.rs        # sites.db, the one server-wide file of facts about hosts: site memory (`site_facts`: platform, API root, winning method, blocks_server) and Wee Chef's terms checks (`terms_checks`); `Config.sites_db`, `AppState.sites`
  site_terms.rs   # Sites whose terms forbid automated fetching: the list (`data/site-terms.toml`), `check`/`guard`/`photo_is_listed`, and Wee Chef's Flagger
  scraper/platforms.rs # Platform detection (WordPress Link header / head links), WPRM/Mediavine/Tasty ids and card markup
  scraper/page.rs # A recipe the extension read in the cook's browser (`page`): capped, parsed, never fetched
  images.rs       # /img resizer (WebP, disk cache), hero preload Link header
  telemetry.rs    # Sentry: init, scrubbing, request transactions, browser Server-Timing hint
  video.rs        # Cooking videos: caption, else download + whisper transcript + frames for Wee Chef
  video_jobs.rs   # Their queue: worker pool, bounded wait, job status for the Add box, heavy-work budget
  relay.rs        # Client for crumb-relay: rotation, 5-minute skip of a relay that's down, time budget; worker calls
  trash.rs        # Deleted recipes, restorable for 30 days
  popular.rs      # Popular: links several households saved, counted in memory
  scraper.rs, importers.rs, llm.rs
tests/api.rs      # Router integration tests against a temp DB
auth/             # Hosted edition's Better Auth service (Bun, bun:sqlite, organization plugin); bun test
extension/        # Browser extension (MV3, Chrome + Firefox builds, Bun): asks "Read this recipe in Crumb?", reads the recipe in the page, opens /preview
web/src/
  layouts/Layout.astro   # Head, fonts, theme + transition boot scripts, nav rail / tab bar, timer dock
  pages/                 # Static pages; pages/shell/* are templates for dynamic routes
  islands/               # Svelte islands (one per interactive page/section)
  components/            # Shared Svelte + Astro components
  lib/                   # api, account (both account modes), storage, toast, timers, ingredients, books
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
- **Households:** handlers that touch recipes take `Scoped`, never `State<AppState>`: its `db` is the signed-in
  household's file (`households/{id}/recipes.db`, or the original database for household 1). Anything cached in
  memory or on disk per box must be keyed by `state.household`.
- **Account modes:** `password` (one `APP_PASSWORD`), `accounts` (Rust-owned users, households, invite links) and
  `hosted` (Better Auth in `auth/`, households = organizations, mapped to local ids in `accounts.db`). The web
  talks to either through `web/src/lib/account.ts`; handlers get the signed-in user via `auth::session`/`SignedIn`.
  Account, household, connected apps and data settings live on `/more/account` (`AccountPage.svelte`), not More.
- **API parity:** JSON is camelCase; errors are `{statusCode, statusMessage, message}`,
  plus a `code` only when a client can act on it (`site_blocked`: a recipe site's bot check turned the server away, so the web
  nudges toward the extension instead of showing an error; `site_terms`: the site's terms forbid automated fetching, and
  the error also carries `site`, the listed site's display name, e.g. "Allrecipes", which BlockedNudge and the preview's
  failed state show and `crumb-client` exposes as `Error::site()`).
- **`sites.db`:** one file beside the home database, opened once by `sites::Sites` (never a second opener or a second
  config path). Only facts about public hosts: how a host was read (`site_facts`, `SCRAPE_SITE_MEMORY=off` stops
  it) and when Wee Chef last read its terms (`terms_checks`). Never a recipe, a household or a link's path.
- **Sites whose terms forbid automated fetching:** `data/site-terms.toml` (reviewed by a person, compiled into the
  server and the extension) lists hosts the server never fetches. `scraper::scrape_page` calls `site_terms::guard` first,
  before any network use or site memory, and is the one guarantee that no scrape of a listed host (page, API, relay,
  archive, browser) goes out: import, refresh, preview and MCP all come through it, and they answer 422 `site_terms`.
  Callers add checks only for a different answer (the preview fails before it asks; a video is queued, not scraped). A
  `page` from the extension never reaches `scrape_page` and is still taken. A redirect from another site, or a page
  Chromium follows, can't reach a listed host either: `AppState::new` installs `site_terms::host_is_listed` as
  `crumb_fetch::guard::set_veto`, which every guard check and Chromium's proxy ask. Photos on a listed host, or on an entry's
  `image_hosts` (CDNs), are never downloaded either: `/img` redirects the browser to the original (307) and
  `photo_is_dead` / `fetch_to_embed` skip them (`site_terms::photo_is_listed`). Wee Chef
  suggests more in the background (`site_terms::Flagger`, `tos-suggestion` GitHub issues, `data/site-terms-ignore.toml`
  for hosts a person cleared); a person edits the list, Wee Chef never does. Robots.txt is ignored on purpose.
- **Deduplication:** saving a URL that already exists returns the existing recipe (`isNew: false`).
- **Trash:** `recipes::delete_recipes` moves recipes to `recipe_trash` (`src/trash.rs`): the whole row as JSON, its cookbooks
  and cooks; restored under its own id within 30 days, then purged. Nothing else needs to skip deleted recipes. The
  connector can list and restore, never empty it (the safeguard against a recipe's text talking Claude into a delete).
- **Popular** (`src/popular.rs`): public recipe links at least `POPULAR_MIN_HOUSEHOLDS` (≥2) households saved, counted from
  the boxes every 6 hours and kept only in memory; links only (host, a title only when that many agree), never a photo,
  a recipe's contents or who saved it. A household opts out in its own box (`box_settings`); `POPULAR=off` for all.
- **Cook/view log:** `recipe_events` (`viewed`/`cooked`, deduped within 30 min / 6 h). Views are pruned after 400
  days; cooks are kept and go into backups as `cookedAt`. Anything that logs a view must run inside `whenActive`.
- **Sentry:** the browser SDK gets its DSN, environment and release from a `Server-Timing` header the server adds
  to HTML responses, so there is no build-time DSN and one image serves dev and stable. The SDK loads after the
  page is idle; keep it off the critical path. Never attach recipe contents, bodies, query strings or cookies.
  Every AI call (Wee Chef's providers and Typesafe) is a `gen_ai.chat` span via `telemetry::AiSpan` (provider, model,
  feature, tokens), kept whatever the sample rate so Sentry's AI dashboards show whole usage and cost; never prompts or replies.
- **Env vars:** a new or changed variable goes in `.env.schema` (`auth/.env.schema` for the auth service) with its
  type, default and `@sensitive`/`@required`, and in the README's Configuration table.
- **Recipe videos:** `video` is a link (the scraper keeps only ones it can play: JSON-LD `VideoObject`, then the
  recipe card's player, `og:video`, the post's first embed); `crumb_core::embed` works out the player, sent as the
  read-only `videoEmbed`. The page's `RecipeVideo.svelte` loads nothing from the video's site until Play.
- **Previews** (`/preview?url=`, `src/preview.rs`) scrape on GET, so they only scrape for `Sec-Fetch-Site` `none` or
  `same-origin`; from another site they ask first. The Add button posts to `/api/recipes/import`, which takes the
  preview's kept scrape. A site that turns the server away (`site_blocked`) can still be read: the extension reads
  only the recipe (JSON-LD, else the recipe card, plus `og:image`/`og:title`, 512 KB max) in the cook's browser and the
  preview page (`via=extension`) posts it as `page` to `/api/preview` (or `/api/recipes/import`); the server parses it
  (`scraper::page::FromPage`), saves it under the link, never a URL from the payload, and scrapes only if it held no
  recipe. The native apps use `POST /api/recipes/preview` `{url}` (`preview::api`, `Client::preview`) instead: `{status: "saved", id, title}`, `{status: "import"}` (video or shared cookbook) or `{status: "ready", recipe}`, keeping the scrape the same way. The extension is its own product: versions `extension-vX.Y.Z` from commits touching
  `extension/`, released by `.github/workflows/extension.yml`.
- **Anything with a side effect on GET** (like `/random`) must be excluded from `speculation-rules.json` and marked
  `data-no-prerender`, or hovering the link runs it.

- **Core changes land on master first.** Changes to `crates/`, `src/` or the API go in their own PR against
  master, never on a platform branch (`android/`, `ios/`, `desktop/`). Platform branches then merge master in, so
  they only ever differ from master in their own directory and CI workflow.

## Code Style

- Rust: `cargo fmt`, clippy clean with `-D warnings`
- Web: oxfmt (no semicolons, double quotes, 2-space indent, 100 char width); oxlint correctness=error
