# Linux desktop app (Qt6/QML) — handoff

Issue #9 asks for native apps. This is the plan for the Linux one: a real Qt6/QML app written from
scratch, built for Omarchy (Hyprland, Wayland) first. It is not a webview.

## Decisions so far

- **Toolkit: Qt6 with QML, Rust through [cxx-qt](https://github.com/KDAB/cxx-qt).** Omarchy 4's own
  desktop (`omarchy-shell`: bar, menus, panels, notifications) is Quickshell, which is Qt6/QML, so a QML
  app can look like part of the desktop. GTK/libadwaita apps only get plain `Adwaita`/`Adwaita-dark` from
  `omarchy-theme-set-gnome`, never the palette. Avoid Qt Widgets: Omarchy sets
  `QT_QPA_PLATFORMTHEME=gtk3`, so Widgets would imitate GTK.
- **The hosted server is the source of truth.** The app is a client of the existing REST API
  (`/api/**`), so recipes stay in sync with the web app and the phone. No private database per desktop.
- **Behaviour is shared through a Rust core crate; only the UI differs per surface.** Anything the app
  computes locally (scaling, mise en place, timers) comes from `crumb-core`, not from a QML copy of the
  TypeScript.
- **Offline is out of scope for v1.** A read-only cache comes later; full offline editing would need
  syncing and is a separate project.

## Relation to the webview branch

`feat/omarchy-desktop` wraps the existing web pages in WebKitGTK (from an agent's patch, with fixes for
the NVIDIA Wayland crash, orphaned server, and theme contrast). It works as a stopgap but is not the
native app. Its `desktop/omarchy_theme.py` has useful contrast rules to port: Omarchy's `muted` is a
border shade and too dim for text; `selection` is a background, not a text colour; `red` can be grey in
monochrome themes (fall back to `bright_red`).

## Architecture

```
Cargo workspace
├── crates/crumb-core     # pure logic: no axum, no tokio, no rusqlite
├── crumb (repo root)     # server, depends on crumb-core
└── desktop/              # crumb-desktop: cxx-qt bridge + QML, depends on crumb-core
    ├── src/              # Rust QObjects: RecipeListModel, RecipeModel, Session, Theme, Timers
    └── qml/              # Main.qml, pages/, components/
```

### Phase 0: extract `crumb-core`

Today the crate is a single package at the root. Convert it to a workspace and move the pure parts into
`crates/crumb-core`:

- From the server: `model.rs` (types, `normalize_sections`, validation), `fractions.rs`,
  `categories.rs`, and anything in `text_parser.rs` / `markdown.rs` that doesn't need I/O.
- **Port from `web/src/lib/` to Rust** (about 600 lines, the only cooking logic that currently lives only
  in TypeScript):
  - `ingredients.ts`: `parseNumber`, `parseIngredient`, `formatQuantity`, `scaleIngredient`,
    `pluralUnit`, `miseEnPlace`, `findTimers`, `ingredientsForStep`
  - `format.ts`: `recipeToText`, `recipeToMarkdown`, `bookToText`
  - `recipe.ts`: `countItems`, `kicker`, `hostOf`, `webLink`
- Port the TS tests with them, and add table tests that run the same inputs through both the TS and the
  Rust versions, so the web and desktop can't drift. (Later the web could call the Rust versions through
  WASM; not needed now.)
- The server must behave exactly as before: `cargo test`, clippy and the production DB are unaffected.
  Ship this phase on its own.

Android (Compose) can use the same crate later through uniffi, or call the API.

### Phase 1: skeleton app

- `desktop/` crate with cxx-qt (cargo-only build via `cxx-qt-build`, no CMake if possible).
- Needs `qt6-base`, `qt6-declarative` (and `qt6-wayland`) on Arch/Omarchy.
- App ID and Wayland `app_id`: `crumb-desktop`, matching `crumb-desktop.desktop` (`StartupWMClass`), so
  Hyprland window rules and the launcher icon work.
- **Server URL + login:** first run asks for the server URL (default: production) and the app password,
  posts to `POST /api/auth/login`, and keeps the session cookie (90-day `Max-Age`). Store the cookie with
  the Secret Service (`keyring` crate or QtKeychain), not in a plain file. On a 401, return to the login
  screen.
- HTTP from Rust (`reqwest` with rustls) on a background tokio runtime; results reach QML through Qt
  signals, never blocking the UI thread. JSON is camelCase; errors are `{statusCode, statusMessage,
  message}`.

### Phase 2: Omarchy theme

- Read `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` (fall back to
  `~/.config/omarchy/current/theme/colors.toml` for older installs), and `shell.toml` next to it for the
  shell's sizes and font scale if useful.
- `omarchy-theme-set` swaps the whole `current/theme` directory with `mv` and then rewrites
  `current/theme.name`, so watch the `current/` directory (`QFileSystemWatcher`) and reload when
  `theme.name` changes. Don't poll.
- Expose a `Theme` singleton to QML with Crumb's roles, mapped the same way the web tokens are (`bg`,
  `paper`, `tint`, `text`, `textMuted`, `line`, `primary`, `tile`, `onTile`, `butter`, `onButter`, `nav`,
  `error`), and enforce 4.5:1 contrast for text roles (see the rules above).
- Without Omarchy: use Crumb's own Green Tile palette (light/dark from `app.css`).
- Fonts: Nunito Sans, DM Serif Display, Caveat, from `web/src/assets/fonts`, loaded with
  `FontLoader`/`QFontDatabase`. Keep the design language: four radii (16/12/full/book-spine), 1px lines,
  shadows only on menus, nothing under 13px, butter only for the one main action.

### Phase 3: v1 screens

In order:

1. **Recipes**: list/grid with photos, search, category filter. `GET /api/recipes`.
2. **Recipe**: hero photo, ingredients by section with scaling (`crumb-core`), steps, notes, source link
   (opens in the default browser). Log a view with `POST /api/recipes/{id}/viewed`.
3. **Cook mode**: one step at a time, large type, the step's ingredients (`ingredientsForStep`), timers
   found in the step (`findTimers`), "Cooked it" (`POST /api/recipes/{id}/cooked`). Keep the screen awake
   (idle-inhibit through the Wayland protocol or `org.freedesktop.ScreenSaver`).
4. **Timers**: several at once, a dock that follows across screens, desktop notifications when they end
   (`org.freedesktop.Notifications`, which the Omarchy shell shows).
5. **Import**: paste a URL or text → `POST /api/recipes/import`; show the existing recipe when
   `isNew: false`.

Photos: use the server's resizer, `/img/{id}/{width}?v={fnv1a(image)}` (WebP), cached on disk under
`$XDG_CACHE_HOME/crumb-desktop`.

### Later

Prep mode, edit, cookbooks + shelf, suggestions / Surprise me, sharing, Wee Chef checks, backups
(`/api/export`), printing (Qt print dialog), a read-only offline cache, and packaging (AUR `crumb-desktop`
and/or an install script like the webview branch's).

## Things to confirm while building

- Is cxx-qt's cargo-only build enough, or does QML module registration need CMake?
- How long do QML cold starts take on this hardware? The Omarchy shell stays resident; this app won't.
- Hyprland: fractional scaling, and whether to draw a title bar at all (tiled windows usually don't).
- Should the app also work against a local `cargo run` server? Useful for development:
  `CRUMB_SERVER=http://127.0.0.1:3000`.
- Does auth need anything beyond the password cookie (tokens per device, the multi-user work in #5)?

## Done when

- `crumb-core` is extracted, the server is unchanged, and the TS/Rust parity tests pass.
- The app launches from the Omarchy launcher on Wayland, logs in to the real server, and lists,
  searches, opens, scales and cooks recipes with working timers and notifications.
- Switching Omarchy themes recolours the open app within a second, with readable text on every bundled
  theme.
