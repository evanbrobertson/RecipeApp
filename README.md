# Crumb

Crumb is a private recipe box that you host yourself. Paste a link or the recipe text, and Crumb keeps the ingredients and the steps. You can also connect Crumb to Claude. Then Claude can save, find and change your recipes from a chat.

## Features

### Wee Chef and the optional keys

Wee Chef is the optional helper in Crumb. It uses two optional keys that you set on the server:

- **AI key:** `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` or `DEEPSEEK_API_KEY`. With this key, Wee Chef reads pasted text, files and photos. It also writes the reasons and ideas in "Try next".
- **Check key:** `TYPESAFE_API_KEY`. With this key, Wee Chef checks each new recipe in the background.

Without the keys, Crumb uses its built-in text reader and its own "Try next" ranking. All other features work without a key. This README tells you which features need a key.

### Recipe box and search

The recipe box holds all your recipes. The Recipes page shows the recipes as cards, with the newest recipe first.

- Type in the search field to find recipes. Crumb compares each word with the title, the ingredients, the category and the cuisine.
- Select a category to show only the recipes in that category.
- Select **Select** to choose more than one recipe. Then add the recipes to a cookbook, or delete them.

The home page shows these parts:

- **Pick up where you left off:** the last three recipes that you opened on this device. If you stopped in cooking mode, the card shows the step and opens cooking mode again.
- **Can't decide?:** "Surprise me" and "What should I cook next?". Refer to [Try next, Surprise me and ideas](#try-next-surprise-me-and-ideas).
- **The shelf:** your cookbooks.
- **Fresh in the box:** the eight newest recipes.

### Add recipes

Use the Add box on the home page or on the Add page. The Add box finds what you pasted and selects the correct mode. You can also select a mode from its menu: Link, Recipe text, Photo, File, Claude, From scratch or Other apps.

**Link.** Crumb reads the recipe from the web page. It gets the title, the photo, the times, the servings, the ingredients, the steps and the nutrition when the page has them.

- If you saved the same link before, Crumb opens the recipe that is already in your recipe box.
- If a site blocks Crumb, or shows the recipe only with JavaScript, Crumb tries again in a browser on the server. The Docker image includes this browser.
- If you paste more than one link, the Import page opens and imports all the links together.
- A share link from Just the Recipe opens the original recipe page.

**Recipe text.** Paste the full recipe from an email, a note or a document. Crumb divides the text into ingredients, steps, times and sections. With the AI key, Wee Chef reads the text. Without the AI key, the built-in text reader reads it. If Crumb finds no ingredients and no steps, it tells you to add "Ingredients" and "Instructions" headings.

**Photos.** Add one to six photos of the pages of one recipe. You can paste the photos, drop them or take them with the phone camera. You can change the order of the pages before Crumb reads them.

- With the AI key, Wee Chef reads the photos and opens the new recipe. You can add a note for Wee Chef, for example "the pancakes on the left".
- Without the AI key, your browser reads the photos on your device. Crumb then opens the recipe in the editor and tells you to check the amounts. The browser reader reads English text.
- Crumb does not keep the photos. It removes the location and the other photo data from each photo.

**Files.** Drop a file in the Add box, or use the Import page. Crumb reads these files:

- PDF files
- Saved web pages (`.html`)
- Text and Markdown files. A line of `---` divides two recipes in one file.
- Paprika exports (`.paprikarecipes` and `.paprikarecipe`)
- Mealie exports and other schema.org recipe JSON
- Crumb backups and single-recipe exports
- `.zip` files that contain any of the files above

With the AI key, Wee Chef reads PDF files, text files and web pages without recipe data. Without the AI key, the built-in text reader reads them. Crumb cannot read Word files. For a Word file, copy the recipe text and paste it in the Add box. When a file has a recipe with a link that is already in your recipe box, Crumb does not add that recipe again.

**Other apps.** The Import page imports recipes from other recipe apps:

- **Paprika:** upload the export file.
- **Mealie:** upload the JSON export.
- **Just the Recipe:** paste the share links, or upload the PDF files. To make a PDF file, select **Print** and then **Save as PDF** in Just the Recipe.

**From scratch.** Type a short title and select **From scratch**. Crumb opens an empty editor with that title.

**Claude.** Save recipes from a Claude chat. Refer to [Claude connector](#claude-connector-mcp).

**Share from your phone.** Install Crumb as an app from your browser. Then share a link or text from another app to Crumb. The Add page opens with the link or the text.

### Clean-up and checks

Crumb cleans up each new recipe that comes from a link, pasted text, a file or photos. Recipes from a Crumb backup are the exception. Crumb saves these recipes as they are.

**Clean-up.** The clean-up always runs, with or without a key. It does these changes before Crumb saves the recipe:

- It removes checkbox and bullet characters at the start of a line.
- It changes web codes, for example `&amp;`, into the correct characters.
- It writes a decimal amount that is clearly a fraction as a fraction. For example, "0.33333 cup" becomes "⅓ cup". It does not change metric amounts, for example "12.5 g".
- It writes a raw time, for example "PT1H10M", as "1h 10m".
- It removes a step that is the same as the step before it.
- From a web page, it removes a photo credit or an advertisement line that repeats three or more times. It keeps the first one.
- It calculates the total time from the prep time and the cook time when the total time is missing.

**Background check.** This check needs the check key. After Crumb saves a new recipe, Wee Chef examines each ingredient line and each step. On a new recipe that you did not edit, Wee Chef makes the changes that it is sure about:

- It changes a heading in the steps or in the ingredients into a section name.
- It joins a step that the source divided into two parts.
- It moves a tip from the steps to the notes.
- It removes a line that is not part of the recipe.

Wee Chef does not change a line that it is not sure about. It flags the line for you to examine. If you edit the recipe before the check ends, Wee Chef only flags lines and changes nothing.

**On the recipe page.** A small card shows what Wee Chef did:

- "Wee Chef tidied 3 things": select **See** to show the list of changes. Select **Undo** to put back the ingredients, the steps, the notes and the times from the import.
- "2 lines might need a look": select **Edit** to open the editor.

Undo is available only until you change the recipe. After you use Undo on a recipe, no later check cleans up that recipe again.

**In the editor.** A flagged line shows a reason, for example "looks like a section heading". Each flag gives one or more actions:

- **Make it a heading**, **Remove it**, **Move to notes** or **Join with the step above**, as applicable to the flag.
- **Keep as is:** Crumb keeps the line. Later checks do not flag the same line for the same reason again.

A flag also goes away when you change or remove the line.

**Suggestions page.** When recipes have flagged lines, the navigation shows **Suggestions** with a count. On a phone, the tab bar shows **Review**. The Suggestions page lists each recipe and the number of flagged lines in it. Wee Chef changes nothing on this page.

**Check with Wee Chef.** This menu item on the recipe page needs the check key. It checks the recipe again now. Wee Chef only flags lines on a recipe that is already in your recipe box. The one exception is a new import that you did not edit and that Wee Chef did not check yet. Wee Chef treats that recipe as a new import. The check can also do the small clean-up again: checkbox characters, web codes, fractions and raw times. You can undo these changes.

**Check all recipes with Wee Chef.** This item on the More page needs the check key. It checks these recipes:

- Recipes from a link, text, a file or photos that Wee Chef never checked
- Recipes that you restored from a backup
- Recipes that you edited after their last check
- Recipes whose check failed, up to three tries

"Check all" only flags lines. It also does the small clean-up, which you can undo. It does not check recipes that you wrote yourself or that Claude saved. The More page shows the progress.

### Recipe page

The recipe page shows the photo, the title, the author, the source link and the description. It also shows the times (Prep, Cook, Extra and Total), the servings and your cook history.

- **Scale:** select ½×, 1×, 2× or 3×. Crumb changes the amounts and the number of servings. Mise en place mode and cooking mode use the same scale. Crumb keeps the scale while the browser tab stays open.
- **Ingredient list:** tap an ingredient to mark it. Tap it again to remove the mark.
- **Split panels:** on a wide screen, the ingredients and the method show side by side. The panel that you click or move into with the keyboard gets more width.
- **Lock:** select the button between the panels to stop the resize. The panels then keep a fixed width. Crumb remembers this setting on your device.
- **Notes:** your notes show below the method in handwriting.
- **Nutrition:** the nutrition values show when the recipe has them.
- **Cookbooks:** in "On the shelf in", tap a cookbook to add the recipe to it. Tap it again to remove the recipe.

The menu on the recipe page has these items:

- **Mark as cooked**, **Edit**, **Copy as text**, **Share** and **Print**
- **Surprise me**
- **Check with Wee Chef**, only with the check key
- **Export as JSON:** one file in the backup format. The Import page can read it back into a recipe box.
- **Export as Markdown:** one text file that you can read in any text editor.
- **Delete**

### Mise en place

Mise en place mode helps you prepare all the ingredients before you cook. Open it from the recipe page. Crumb gives each ingredient a container that agrees with its quantity. The ingredients are in three groups:

1. **Chop & prep:** items for the cutting board.
2. **Measure into bowls:** from the large bowl to the medium bowl, the small bowl, the ramekin and the pinch bowl.
3. **Keep within reach:** seasoning and other items "to taste".

The "Get out" line lists the bowls and the board that you need. Tap each ingredient when it is ready, and its bowl fills. When all the ingredients are ready, select **Start cooking**. The screen stays on, and the scale control is available. Crumb keeps your progress while the browser tab stays open.

### Cooking mode

Cooking mode shows one step at a time in large text. Open it from the recipe page.

- Swipe, or tap the arrows, to go to the next or the previous step.
- On a keyboard, use the Right arrow, Space or Page Down for the next step. Use the Left arrow or Page Up for the previous step.
- Tap a dot at the top to go directly to that step.
- Tap **Ingredients** to show the full list. You can mark each ingredient and change the scale there.

The screen stays on in cooking mode. If the browser needs a tap first, Crumb shows "Tap to keep the screen on". When a step includes a time, for example "bake for 25 minutes", Crumb shows a button to start a timer. The "You'll need" card shows the ingredients that the step names, with the scaled amounts.

Crumb remembers your step while the browser tab stays open. After the last step, Crumb shows "Bon appétit!" and adds a cook to the cook history.

### Timers

Start a timer from a step in cooking mode. The timers show in a dock on every page, and they continue when you go to a different page. Timers also stay the same in all your open tabs. When a timer ends, Crumb plays a chime and shows a message. On a phone that can vibrate, the phone also vibrates. Tap a timer to remove it.

### Shelf and cookbooks

A cookbook is a named group of recipes. The Shelf page shows your cookbooks as books on a shelf.

- Select a book to take it off the shelf. The book opens to a table of contents.
- Select **New book** to make a cookbook. Give it a name, a description and a cover colour.
- On the cookbook page, change the name, the description or the colour. You can also remove recipes from the cookbook.
- When you delete a cookbook, Crumb keeps the recipes in it.

To add recipes to a cookbook, use "On the shelf in" on the recipe page. You can also use **Select** on the Recipes page.

### Try next, Surprise me and ideas

**Try next.** On the home page, select **What should I cook next?**. Crumb shows four recipes from your recipe box. Each card can show a reason, for example:

- "Because you made Chicken Tinga"
- "You've opened this 5 times"
- "Last made 3 months ago"
- "Weeknight-quick: 30 min" or "A weekend project: 2h 30m"
- "New in the box, not tried yet"
- "Good for the season"

The season comes from your time zone, so the seasons are correct south of the equator. Crumb does not show recipes that you cooked in the last 14 days. Select **Shuffle** to show four different recipes. Shuffle shows when you have more than four recipes.

With the AI key, Wee Chef puts the four recipes in a new order and writes a short reason for each one. On about one day in three, the last card is an idea for a dish that is not in your recipe box. The idea card opens a web search for a recipe. Without the AI key, Crumb uses its own ranking and shows no ideas. To use only the Crumb ranking with an AI key, set `SUGGESTIONS_AI=off`.

**Surprise me.** Crumb opens one recipe at random. It does not show recipes that you saw in this session, that you viewed recently or that you cooked in the last 14 days. Surprise me is on the home page, on the More page and in the recipe page menu. On the recipe that opens, select **Shuffle again** for a different recipe.

### Cook history

Crumb adds a cook to the history in two ways:

- Select **Mark as cooked** in the recipe page menu. The message has an **Undo** button.
- Go past the last step in cooking mode.

Crumb adds only one cook for the same recipe within a few hours. The recipe page shows a line, for example "Cooked 3 times · last 2 weeks ago". Try next and Surprise me use the cook history. Backups include the cook history. Crumb also records when you open a recipe, for Try next. It deletes these records after 400 days.

### Theme

Select a theme on the More page:

- **Light:** always light.
- **Dark:** always dark.
- **System:** the same as your device.
- **Sunrise & sunset:** dark from sunset to sunrise.

For "Sunrise & sunset", Crumb calculates the times from your time zone. For exact times, select **Use my location**. Crumb keeps the location on your device only. Select **Forget location** to remove it.

### Claude connector (MCP)

Connect Crumb to Claude as a custom connector. Then ask Claude to work with your recipe box in a chat. Claude can do these tasks:

- Save a recipe from the chat, or import a recipe from a link or text.
- Search your recipes and read a recipe.
- Change a recipe, for example to scale it or to replace an ingredient.
- Fill empty fields of a recipe from its source page.
- Suggest what to cook, pick a random recipe and mark a recipe as cooked.
- Show, change and organize your cookbooks.
- Delete a recipe or a cookbook. Claude first shows a preview and asks you to confirm.

You approve the connection with the app password. Refer to [Connect to Claude](#connect-to-claude) for the setup.

### Backups

On the More page, select **Download a backup**. Crumb downloads one JSON file with all your recipes, your cookbooks and your cook history. To restore a backup, drop the file in the Add box or on the Import page.

- Crumb saves the restored recipes as they are in the backup. No clean-up or check runs on them.
- When a restored recipe has a link that is already in your recipe box, Crumb does not add it again.
- When you restore the same backup two times, Crumb does not add the same cooks two times.

### Password

Set `APP_PASSWORD` to protect Crumb with a password. The password protects the web app and the approval of the Claude connector. Without `APP_PASSWORD`, Crumb has no password. Use this only on your own computer.

### Self-hosting

Crumb is one server program and one SQLite database file. To start Crumb, refer to [Quick start](#quick-start). For the settings, refer to [Configuration](#configuration). For Railway, refer to [DEPLOY.md](./DEPLOY.md).

## How it's built

- **Server:** one Rust binary (axum, rusqlite, scraper, wreq, reqwest). It serves the API, the MCP connector and
  OAuth, and the static frontend.
- **Frontend:** Astro, built to static HTML, with small Svelte 5 islands for the interactive parts. Every
  navigation is a plain page load, made instant with speculation-rules prerendering and cross-document view
  transitions.
- **No loading round trip:** for pages that show your recipes, the server inlines the page's data as JSON
  (`#page-data`) into the HTML, so the page renders from a single response.
- **Tricky sites:** pages are fetched with a real browser's TLS and HTTP/2 fingerprint (Firefox, then Safari if that's refused). If both are blocked or the recipe is rendered by JavaScript, the server retries in headless Chromium (installed in the Docker image).
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

See [DEPLOY.md](./DEPLOY.md) for Railway, and [docs/RELEASING.md](./docs/RELEASING.md) for how builds are released (CI, image tags, dev and stable).

## Configuration

| Variable            | Required   | Description                                                                   |
| ------------------- | ---------- | ----------------------------------------------------------------------------- |
| `APP_PASSWORD`      | Production | Password for the web app and for approving the Claude connector               |
| `SITE_URL`          | No         | Public URL. On Railway, `RAILWAY_PUBLIC_DOMAIN` is used automatically         |
| `DATABASE_PATH`     | No         | SQLite file. Defaults to the Railway volume, or `.data/recipes.db` locally    |
| `ANTHROPIC_API_KEY` | No         | Turns on Wee Chef (reads pasted text, files and photos, writes "Try next" blurbs and ideas) |
| `OPENAI_API_KEY`    | No         | The same with OpenAI instead                                                  |
| `DEEPSEEK_API_KEY`  | No         | The same with DeepSeek instead                                                |
| `ANTHROPIC_BASE_URL` | No        | A different API address (also `OPENAI_BASE_URL` and `DEEPSEEK_BASE_URL`)    |
| `LLM_PROVIDER`      | No         | `anthropic`, `openai` or `deepseek`; default: the first one with a key       |
| `ANTHROPIC_MODEL`   | No         | Default `claude-sonnet-5` (also `OPENAI_MODEL`, default `gpt-5.6-luna`, and `DEEPSEEK_MODEL`, default `deepseek-flash`) |
| `SUGGEST_MODEL`     | No         | A different (e.g. cheaper) model for Wee Chef's "Try next" blurbs and ideas   |
| `VISION_MODEL`      | No         | The model Wee Chef reads recipe photos with; defaults to the main model (`deepseek-flash` on DeepSeek) |
| `SUGGESTIONS_AI`    | No         | Wee Chef is on whenever a key is set; `off` keeps "Try next" algorithm-only   |
| `TYPESAFE_API_KEY`  | No         | Turns on Wee Chef's background check of new recipes, "Check with Wee Chef" and "Check all" |
| `TYPESAFE_MODEL`    | No         | Default `jev-1.13.0` (also `TYPESAFE_BASE_URL`, default `https://api.typesafe.ai`) |
| `CHECKS_AI`         | No         | `off` turns the background check off, even with `TYPESAFE_API_KEY` set     |
| `WEB_DIST`          | No         | Built frontend directory, default `web/dist`                                  |
| `HOST` / `PORT`     | No         | Listen address, default `0.0.0.0:3000`                                        |
| `CHROMIUM_PATH`     | No         | Chromium for the scraping fallback (set in the Docker image; auto-detected)   |
| `BROWSER_SCRAPING`  | No         | Set to `off` to disable the headless browser fallback                         |
| `SENTRY_DSN`        | No         | Report errors and traces to Sentry (server and browser). Unset: nothing is sent |
| `SENTRY_ENVIRONMENT` | No        | Environment name in Sentry, default `production` (the hosted app uses `dev` and `stable`) |
| `SENTRY_RELEASE`    | No         | Release name; set in the Docker image by CI, default `crumb@<version>`        |
| `SENTRY_TRACES_SAMPLE_RATE` | No | Share of server requests traced, default `0.1` (the browser traces 0.2 in `stable`, all in `dev`) |
| `SENTRY_BROWSER`    | No         | `off` keeps the browser SDK from loading while the server still reports       |

The older `NUXT_*` variable names are still read as fallbacks.

Sentry is strictly opt-in: without `SENTRY_DSN` the server sends nothing and pages load no Sentry code.
With it, no cookies, auth headers, query strings, request bodies or user details are sent, and Session
Replay masks all text and blocks all media.

## Connect to Claude

1. Deploy with `APP_PASSWORD` set.
2. In Claude go to **Settings → Connectors → Add custom connector** and paste `https://<your-app>/mcp`.
3. Click **Connect** and approve with your app password.

The in-app **Connect** page shows your exact URL. Claude Code:
`claude mcp add --transport http recipes https://<your-app>/mcp`.

Tools exposed: `search_recipes`, `get_recipe`, `save_recipe`, `import_recipe_from_text`, `import_recipe_from_url`, `update_recipe`, `refresh_recipe_from_source`, `suggest_recipes`, `random_recipe`, `mark_recipe_cooked`, `delete_recipe`, `list_cookbooks`, `get_cookbook`, `add_to_cookbook`, `remove_from_cookbook`, `update_cookbook`, `delete_cookbook`.

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd web && bun run check && bun run lint && bun run format:check
```
