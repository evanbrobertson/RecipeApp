# Just the Recipe

A private recipe box in the style of "Just the Recipe". Paste a link **or the recipe text** and get back
just the ingredients and steps. Add it to Claude as a connector so Claude can save, find and adapt
your recipes from any chat.

## Features

- **One box for everything:** paste a URL or raw recipe text (email, notes, PDF text). Several links at once
  are bulk-imported. Text is parsed by a built-in parser, or by Claude when `NUXT_ANTHROPIC_API_KEY` is set.
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

## Quick start

```bash
bun install
bun run dev            # http://localhost:3214 (no password in dev unless NUXT_APP_PASSWORD is set)
```

Production:

```bash
bun run build
NUXT_APP_PASSWORD=change-me bun run start
```

See [DEPLOY.md](./DEPLOY.md) for Railway.

## Configuration

| Variable                 | Required   | Description                                                                   |
| ------------------------ | ---------- | ----------------------------------------------------------------------------- |
| `NUXT_APP_PASSWORD`      | Production | Password for the web app and for approving the Claude connector               |
| `NUXT_PUBLIC_SITE_URL`   | No         | Public URL. On Railway, `RAILWAY_PUBLIC_DOMAIN` is used automatically         |
| `DATABASE_PATH`          | No         | SQLite file. Defaults to the Railway volume, or `.data/recipes.db` locally    |
| `NUXT_ANTHROPIC_API_KEY` | No         | Lets Claude parse text pasted on the Add page (falls back to built-in parser) |
| `NUXT_ANTHROPIC_MODEL`   | No         | Model used for parsing, default `claude-opus-5`                               |
| `CHROMIUM_PATH`          | No         | Chromium for the scraping fallback (set in the Docker image; auto-detected)   |
| `BROWSER_SCRAPING`       | No         | Set to `off` to disable the headless browser fallback                         |

## Connect to Claude

1. Deploy with `NUXT_APP_PASSWORD` set.
2. In Claude go to **Settings → Connectors → Add custom connector** and paste `https://<your-app>/mcp`.
3. Click **Connect** and approve with your app password.

The in-app **Claude** page shows your exact URL. Claude Code:
`claude mcp add --transport http recipes https://<your-app>/mcp`.

Tools exposed: `search_recipes`, `get_recipe`, `save_recipe`, `import_recipe_from_text`,
`import_recipe_from_url`, `update_recipe`, `delete_recipe`, `list_cookbooks`, `add_to_cookbook`.

## Stack

Nuxt 4 · Nuxt UI 4 · Tailwind 4 · SQLite (better-sqlite3 + Drizzle) · Cheerio · playwright-core ·
unpdf · fflate · MCP TypeScript SDK · Anthropic SDK (optional) · oxlint + oxfmt
