# Desktop parity with the web

The Linux app aims to do everything the web app does, screen for screen. The one thing that
differs is the look: colours come from the Omarchy theme (`Palette`), not Crumb's Green Tile.
Layout, wording, order, behaviour and the four radii (16 cards, 12 controls and photos, full
pills, 3px spines) follow the web.

## How the app is put together

- **`Api`** (`src/api.rs`) is every server call. From QML, use a `Requests { id: requests }`
  on the page: `requests.call("recipe", {id: 7}, recipe => …, error => …)`. Answers are the
  API's own JSON (camelCase, as the web sees it). The list of operations and their arguments
  is `dispatch` in `src/api.rs`. Files for import and export are paths (a `FileDialog`'s
  `file://` URL is fine), and exports are saved into a folder (`dir`).
- **`Core`** (`src/core_bridge.rs`) is `crumb-core`: every piece of wording and arithmetic
  the web works out in the browser. That includes greetings, day labels, the Add box's
  `detect`, book shapes, prep groups, editor drafts and fixes, cook steps and timers, check
  wording, `kicker` and `displayDuration`. **Never re-implement one of these in JavaScript.**
  If something's missing, add it to `crumb-core` (a PR against master first), then expose it
  in `core_bridge.rs`.
- **`Store`** (`src/store.rs`) holds what the web keeps in `localStorage`
  (`Store.read(key, fallbackJson)` / `Store.write(key, json)`) and in `sessionStorage`
  (`readSession` / `writeSession`), with the web's own keys (`crumb:recent`, `crumb:timers`,
  `crumb:scale:<id>`, `crumb:cook:<id>`, `crumb:prep:<id>`, …). `Store.changed(key)` fires on
  every write.
- **`Session`** handles signing in (`connect`, `login`, `signIn`, `signUp`, `logout`), with
  `mode` and `statusJson` from `/api/auth/status`.
- **Navigation.** `ApplicationWindow.window.go("recipe", {id: 7})`, `.back()` and
  `.toast({title, description, tone, action: {label, onselect}})`. Routes and their files are
  in `qml/Main.qml`. Each page gets `session`, `routeId` and `params`.
- **The kit** (`qml/`):
  - `Heading` (level 1/2/3), `Body` (`muted`, `bold`), `Card`
  - `CrumbButton` (`kind`: primary / soft / outline / ghost / danger, `iconName`, `small`)
  - `StyledField`, `Icon` (lucide names, as `@lucide/svelte`)
  - `Photo` (`recipeId`, `image`), `RecipeCard`, `EmptyState`
  - `ScrollPage` (the web's `<main>` column), `Modal`, `CrumbMenu`, `Toaster`
- **Icons** are lucide SVGs in `assets/icons` (ISC). Copy a missing one from
  `lucide-static`'s `icons/` folder with the web's name.

## Rules

- Match the web's wording exactly: labels, empty states, toasts and hints.
- Colours only from `Palette` (`bg`, `paper`, `tint`, `text`, `textMuted`, `line`,
  `primary`, `tile`/`onTile`, `butter`/`onButter`, `nav`, `error`). Fonts:
  `Palette.fontSans`, `fontSerif`, `fontHand` (greetings and tips only) and `fontNotes` (the
  cook's notes). Nothing under 13px. Butter only for the one main action and the active nav
  item.
- No QML warnings: `--smoke` fails on any.
- Each page file is owned by one person at a time (see below). Shared kit changes go through
  the orchestrator.

## Checking a screen

```bash
cargo build -p crumb-desktop-linux
QT_QPA_PLATFORM=offscreen target/debug/crumb-desktop --smoke
desktop/linux/tools/compare.sh recipe:9 /some/scratch/dir 1280x1000
```

`compare.sh` screenshots the same screen from the web (headless Chromium) and the desktop
(offscreen, via `--route` and `--shot`). Offscreen uses Qt's software renderer, so shader
effects don't show there. Qt logs go to the journal: `journalctl --user -t crumb-desktop`.

## Screens

| Screen | Web source | Desktop files |
|---|---|---|
| Add box, Add, Import | `islands/TopBox.svelte`, `pages/add.astro`, `islands/ImportTools.svelte`, `pages/import.astro` | `TopBox.qml`, `AddPage.qml`, `ImportPage.qml` |
| Home, Shelf, Cookbook | `pages/index.astro`, `islands/HomeFeed.svelte`, `islands/ShelfPage.svelte`, `islands/CookbookPage.svelte`, `components/Bookshelf.svelte`, `components/OpenBook.svelte`, `components/CookbookSpine.svelte`, `components/BookColorPicker.svelte` | `HomePage.qml`, `ShelfPage.qml`, `CookbookPage.qml`, `Bookshelf.qml`, `Book*.qml` |
| Recipes list | `islands/RecipesPage.svelte`, `pages/recipes/index.astro` | `RecipesPage.qml` |
| Recipe | `islands/RecipePage.svelte`, `components/WeeChefCard.svelte`, `components/ShareSheet.svelte`, `components/ScaleControl.svelte`, `components/RecipeVideo.svelte` | `RecipePage.qml`, `ScaleControl.qml`, `WeeChefCard.qml`, `ShareSheet.qml` |
| Cook, Prep, timers | `islands/CookPage.svelte`, `islands/PrepPage.svelte`, `components/PrepBowl.svelte`, `islands/TimerDock.svelte`, `lib/timers.svelte.ts` | `CookPage.qml`, `PrepPage.qml`, `PrepBowl.qml`, `TimerDock.qml` |
| Editor, New recipe | `components/RecipeEditor.svelte`, `islands/EditPage.svelte`, `islands/NewRecipePage.svelte` | `EditPage.qml`, `RecipeEditor.qml` |
| Suggestions | `islands/SuggestionsPage.svelte`, `components/TryNext.svelte`, `components/WeeChefCard.svelte` | `SuggestionsPage.qml` |
| More, Connect, Connections | `pages/more.astro`, `islands/MoreSettings.svelte`, `pages/connect.astro`, `islands/ConnectorUrl.svelte`, `pages/more/connections.astro` | `MorePage.qml`, `ConnectPage.qml`, `ConnectionsPage.qml` |
| Account, sign-in | `islands/AccountPage.svelte`, `components/AccountSection.svelte`, `islands/LoginForm.svelte`, `pages/{login,signup,setup,invite,reset-password,email-change}.astro` | `AccountPage.qml`, `LoginPage.qml` |

Web-only by nature, and left out on purpose:
- Passkeys, and starting a Google or Apple sign-in (both need a browser; linked accounts can
  still be listed and unlinked).
- The browser extension and `/preview`.
- Printing.
- The theme picker: the desktop follows Omarchy.
