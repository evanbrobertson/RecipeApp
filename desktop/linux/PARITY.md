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
  `detect`, the shelf's layout, prep groups, editor drafts and fixes, cook steps and timers, check
  wording, `kicker` and `displayDuration`. **Never re-implement one of these in JavaScript.**
  If something's missing, add it to `crumb-core` (a PR against master first), then expose it
  in `core_bridge.rs`.
- **`Store`** (`src/store.rs`) holds what the web keeps in `localStorage`
  (`Store.read(key, fallbackJson)` / `Store.write(key, json)`) and in `sessionStorage`
  (`readSession` / `writeSession`), with the web's own keys (`crumb:recent`, `crumb:timers`,
  `crumb:scale:<id>`, `crumb:cook:<id>`, `crumb:cook-timers:<id>`, `crumb:cook-checked:<id>`,
  `crumb:cook-pinned`, `crumb:prep:<id>`, …). `Store.changed(key)` fires on
  every write.
- **Errors** reach an error callback as `(message, {code, site})`: `code` is the server's
  machine-readable reason when it gives one (`site_blocked`, `site_terms`), which the Add
  box and the preview answer with `BlockedNudge` instead of a toast.
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
  - `CrumbSwitch` (the web's `.switch`), `BlockedNudge`
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

For motion, `--smoke-page shelf --shot <dir>` films the shelf (fixture books, the test
server's) in a 1280-wide window, where the web's shelf page puts it: hover, the pot riding, a
stack settling and the open and close flights, as `<dir>/<scene>-<ms>.png`. With
`CRUMB_SERVER` set and signed in, the open book shows that server's contents.

## The shelf

- **Layout** is core's: `Core.shelfLayout(booksJson, width, single, addable)` gives each row's
  books, pot, crock and new-book outline (`x`, `y` up from the plank, `w`, `h`, `z`, a book's
  spine, `tilt` and `pivot`, `stack`, `on`), with `Core.shelfMetrics()` for the plank and row
  sizes and `Core.spineFor(book)` for a spine off the shelf. `Bookshelf.qml` only draws and
  moves what it's given; keep it that way.
- **Paint.** Spines, the wall, planks, brackets and the open book's cloth are `Canvas`
  paintings: the cloth grain is `assets/shelf/grain.png`, the web's `--cloth-grain` noise
  rendered once, blended `qt-soft-light`. Soft shadows are Canvas shadows (or layered boxes
  for the open book), not shader effects, so they look the same with and without a GPU. The
  pot and crock are the web's SVGs (`assets/shelf/`). Book cloth comes from `Core.bookLook`;
  the wall (`tint`, grout towards `text`) and the plank (`tile`) from `Palette`.
- **The open book** (`BookOpenModal.qml`) is OpenBook.svelte's 3D flight without a 3D scene:
  the page, the spine, and the cover's two faces are flat items, each with its own 4×4 matrix
  (perspective × flight × stage × face, depth flattened), and faces turned away are hidden.
  When shut, the cover's inside face is drawn as the back cover. The flight starts once its
  first frame is on screen.
- **Differs from the web:** no blur behind the scrim (`backdrop-filter` needs shaders), and
  no reduced-motion setting to follow (Qt doesn't report one).

## Cook mode

- **Timers.** Every step has one adjustable timer, suggested and never trusted: `Core.cookSteps`
  gives each step's `suggestTimers` choices (the first is the suggestion), and `StepTimer.qml`
  nudges (`Core.nudgeTimer`), types minutes (`Core.timerFromMinutes`, -1 for "not a length") and
  words the length (`Core.timerWords`) between `Core.timerMin()` and `Core.timerMax()`. A step
  with no choices offers "Set a timer", starting at `Core.timerDefault()`. A changed length is
  kept per step for the visit. There is no "You'll need" card: guessing a step's ingredients was
  wrong too often.
- **Ingredients.** From 768 px wide they dock on the right, with the scale control and the
  tick list, and the header button pins or unpins them (`crumb:cook-pinned`, on by default).
  Narrower, the button opens them in a modal. Running timers centre in what's left
  (`ApplicationWindow.timerInset`).
- The arrow keys and Space step through the recipe, except while the minutes field is open.

## Screens

| Screen | Web source | Desktop files |
|---|---|---|
| Add box, Add, Import | `islands/TopBox.svelte`, `pages/add.astro`, `islands/PopularLinks.svelte`, `components/BlockedNudge.svelte`, `islands/ImportTools.svelte`, `pages/import.astro` | `TopBox.qml`, `AddPage.qml`, `BlockedNudge.qml`, `ImportPage.qml` |
| Preview (a Popular link) | `pages/shell/preview/index.astro`, `islands/PreviewActions.svelte`, `src/preview.rs` | `PreviewPage.qml` |
| Home, Shelf, Cookbook | `pages/index.astro`, `islands/HomeFeed.svelte`, `islands/ShelfPage.svelte`, `islands/CookbookPage.svelte`, `components/Bookshelf.svelte`, `components/OpenBook.svelte`, `components/BookSpine.svelte`, `components/BookColorPicker.svelte` | `HomePage.qml`, `ShelfPage.qml`, `CookbookPage.qml`, `ShelfCard.qml`, `Bookshelf.qml`, `BookSpine.qml`, `BookOpenModal.qml`, `BookColorPicker.qml` |
| Recipes list | `islands/RecipesPage.svelte`, `pages/recipes/index.astro` | `RecipesPage.qml` |
| Recipe | `islands/RecipePage.svelte`, `components/WeeChefCard.svelte`, `components/ShareSheet.svelte`, `components/ScaleControl.svelte`, `components/RecipeVideo.svelte` | `RecipePage.qml`, `ScaleControl.qml`, `WeeChefCard.qml`, `ShareSheet.qml` |
| Cook, Prep, timers | `islands/CookPage.svelte`, `components/StepTimer.svelte`, `islands/PrepPage.svelte`, `components/PrepBowl.svelte`, `islands/TimerDock.svelte`, `lib/timers.svelte.ts` | `CookPage.qml`, `StepTimer.qml`, `CookIngredients.qml`, `PrepPage.qml`, `PrepBowl.qml`, `TimerDock.qml` |
| Editor, New recipe | `components/RecipeEditor.svelte`, `islands/EditPage.svelte`, `islands/NewRecipePage.svelte` | `EditPage.qml`, `RecipeEditor.qml` |
| Suggestions | `islands/SuggestionsPage.svelte`, `components/TryNext.svelte`, `components/WeeChefCard.svelte` | `SuggestionsPage.qml` |
| More, Connect, Connections, Trash | `pages/more.astro`, `islands/MoreSettings.svelte`, `pages/connect.astro`, `islands/ConnectorUrl.svelte`, `pages/more/connections.astro`, `pages/more/trash.astro`, `islands/TrashPage.svelte` | `MorePage.qml`, `ConnectPage.qml`, `ConnectionsPage.qml`, `TrashPage.qml` |
| Account, sign-in | `islands/AccountPage.svelte`, `components/AccountSection.svelte`, `islands/LoginForm.svelte`, `pages/{login,signup,setup,invite,reset-password,email-change}.astro` | `AccountPage.qml`, `LoginPage.qml` |

Web-only by nature, and left out on purpose:
- Passkeys, and starting a Google or Apple sign-in (both need a browser; linked accounts can
  still be listed and unlinked).
- The browser extension, and the preview's hand-over from it (`via=extension`). The preview
  itself is here, for Popular links, through `POST /api/recipes/preview`.
- The blocked-site nudge can't know whether the extension is installed: it always opens the
  recipe in the browser (where the extension's toolbar button reads it) and links to where
  to get the extension, instead of the web's installed / not installed / phone variants.
- Printing.
- The theme picker: the desktop follows Omarchy.
