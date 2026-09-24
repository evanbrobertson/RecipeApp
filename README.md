# Just the Recipe

A private recipe box in the style of "Just the Recipe". Paste a link **or the recipe text** and get back
just the ingredients and steps. Add it to Claude as a connector so Claude can save, find and adapt
your recipes from any chat.

## Features

- **One box for everything:** paste a URL (JSON-LD / microdata scraper) or raw recipe text from an email,
  notes app, PDF or a page that blocks scrapers. Text is parsed by a built-in heuristic parser, or by
  Claude when `NUXT_ANTHROPIC_API_KEY` is set.
- **Claude connector (remote MCP):** `https://<your-app>/mcp` with built-in OAuth. From Claude you can
  save a pasted recipe, import from a URL, search by ingredient, read, edit, delete and organise into cookbooks.
- **Made for cooking:** tick off ingredients, highlight the current step, cook mode keeps the screen awake,
  print and share views, dark mode, mobile tab bar, installable PWA with share target.
- **Fast editor:** one textarea per section (one ingredient or step per line) instead of dozens of inputs.
- **Cookbooks, search** across title, ingredients, category and cuisine, and bulk actions.
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

## Connect to Claude

1. Deploy with `NUXT_APP_PASSWORD` set.
2. In Claude go to **Settings → Connectors → Add custom connector** and paste `https://<your-app>/mcp`.
3. Click **Connect** and approve with your app password.

The in-app **Claude** page shows your exact URL. Claude Code:
`claude mcp add --transport http recipes https://<your-app>/mcp`.

Tools exposed: `search_recipes`, `get_recipe`, `save_recipe`, `import_recipe_from_text`,
`import_recipe_from_url`, `update_recipe`, `delete_recipe`, `list_cookbooks`, `add_to_cookbook`.

## Stack

Nuxt 4 · Nuxt UI 4 · Tailwind 4 · SQLite (better-sqlite3 + Drizzle) · Cheerio · MCP TypeScript SDK ·
Anthropic SDK (optional) · oxlint + oxfmt
