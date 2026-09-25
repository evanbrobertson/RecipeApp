# Crumb

A private recipe box. Paste a link **or the recipe text** and keep just the crumbs that matter: the
ingredients and the steps. Add it to Claude as a connector so Claude can save, find and adapt your recipes
from any chat.

## Features

- **One box for everything:** paste a URL or raw recipe text (email, notes, PDF text). Several links at once
  are bulk-imported. Text is parsed by a built-in parser, or by an AI model when an API key is set (Anthropic, OpenAI or DeepSeek).
- **Sites that block scrapers:** if a plain fetch is blocked (403, bot filters) or the recipe is rendered by
  JavaScript, the server retries in headless Chromium (installed in the Docker image).
- **Cook mode:** full-screen, big type, one step at a time, swipe or tap, screen kept awake, one-tap timers
  for any time mentioned in a step, and "you'll need" ingredient hints per step.
- **Mise en place:** every ingredient gets a vessel sized to its quantity (pinch bowl → large bowl, board for
  chopping, jar for "to taste"). Tap each one when it's prepped and watch the bowls fill up.
- **The shelf:** cookbooks are 3D books on a wooden shelf. Pull one out and it opens to a table of contents.
- **Scaling** (½× to 3×), ingredient checklists, print/share/copy, dark mode, installable PWA with share target.
- **Import** from Just the Recipe (links or its PDFs), Paprika, Mealie/schema.org JSON, saved web pages,
  text files and zips. **Backup** everything to one JSON file and restore it.
- **Claude connector (remote MCP):** `https://<your-app>/mcp` with built-in OAuth.
- **Single container + SQLite:** deploys to Railway with a volume. Password-protected.

## How it's built

- **Server:** one Rust binary (axum, rusqlite, scraper, reqwest). It serves the API, the MCP connector and
  OAuth, and the static frontend.
- **Frontend:** Astro, built to static HTML, with small Svelte 5 islands for the interactive parts. Every
  navigation is a plain page load, made instant with speculation-rules prerendering and cross-document view
  transitions.
- **No loading round trip:** for pages that show your recipes, the server inlines the page's data as JSON
  (`#page-data`) into the HTML, so the page renders from a single response.
- **Caching:** hashed assets under `/_astro/` are cached for a year and served precompressed (brotli/gzip).
  HTML is never cached.

```
src/        Rust server (API, MCP, OAuth, scraper, importers, page serving)
tests/      Rust integration tests
web/        Astro frontend (pages, Svelte islands, styles, icons)
```

## Quick start

```bash
cd web && bun install && bun run build && cd ..   # build the frontend into web/dist
cargo run                                         # http://localhost:3000 (no password unless APP_PASSWORD is set)
```

For frontend work, run the server with `cargo run` and then `bun run dev` in `web/`. Astro's dev server
proxies `/api` to Rust, and pages fall back to fetching their data when it wasn't inlined.

Production:

```bash
cargo build --release
APP_PASSWORD=change-me ./target/release/crumb
```

See [DEPLOY.md](./DEPLOY.md) for Railway.

## Configuration

| Variable            | Required   | Description                                                                   |
| ------------------- | ---------- | ----------------------------------------------------------------------------- |
| `APP_PASSWORD`      | Production | Password for the web app and for approving the Claude connector               |
| `SITE_URL`          | No         | Public URL. On Railway, `RAILWAY_PUBLIC_DOMAIN` is used automatically         |
| `DATABASE_PATH`     | No         | SQLite file. Defaults to the Railway volume, or `.data/recipes.db` locally    |
| `ANTHROPIC_API_KEY` | No         | Lets Claude parse pasted text and write "Try next" blurbs                     |
| `OPENAI_API_KEY`    | No         | The same with OpenAI instead                                                  |
| `DEEPSEEK_API_KEY`  | No         | The same with DeepSeek instead                                                |
| `LLM_PROVIDER`      | No         | `anthropic`, `openai` or `deepseek`; default: the first one with a key       |
| `ANTHROPIC_MODEL`   | No         | Default `claude-sonnet-5` (also `OPENAI_MODEL`, default `gpt-5-mini`, and `DEEPSEEK_MODEL`, default `deepseek-chat`) |
| `SUGGEST_MODEL`     | No         | A different (e.g. cheaper) model for "Try next" blurbs                        |
| `SUGGESTIONS_AI`    | No         | Set to `off` to keep "Try next" algorithm-only even with a key                |
| `WEB_DIST`          | No         | Built frontend directory, default `web/dist`                                  |
| `HOST` / `PORT`     | No         | Listen address, default `0.0.0.0:3000`                                        |
| `CHROMIUM_PATH`     | No         | Chromium for the scraping fallback (set in the Docker image; auto-detected)   |
| `BROWSER_SCRAPING`  | No         | Set to `off` to disable the headless browser fallback                         |

The older `NUXT_*` variable names are still read as fallbacks.

## Connect to Claude

1. Deploy with `APP_PASSWORD` set.
2. In Claude go to **Settings → Connectors → Add custom connector** and paste `https://<your-app>/mcp`.
3. Click **Connect** and approve with your app password.

The in-app **Connect** page shows your exact URL. Claude Code:
`claude mcp add --transport http recipes https://<your-app>/mcp`.

Tools exposed: `search_recipes`, `get_recipe`, `save_recipe`, `import_recipe_from_text`,
`import_recipe_from_url`, `update_recipe`, `delete_recipe`, `list_cookbooks`, `add_to_cookbook`.

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd web && bun run check && bun run lint && bun run format:check
```
