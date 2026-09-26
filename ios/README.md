# Crumb for iPhone

A native iOS app for Crumb (issue #9): Swift and SwiftUI, no web view. Like the Android app
it is a thin client for your own Crumb server. Scraping, Wee Chef and the recipe database
stay on the server, recipe logic comes from the shared Rust core, and the iPhone keeps a
copy of what it has shown so recipes open without a connection.

## What's in it

- **Sign in** to your Crumb by address and password (the password is never stored; the
  session cookie goes into the Keychain).
- **Home**: Try next (the server's suggestions, with Wee Chef's blurbs) and Surprise me.
- **Recipes**: search, a grid of cards, and the recipe page: scaling (½× to 3×), ingredients to
  tick off, the method with one-tap timers, the cook's notes, the original source, share as text
  or as a share link, add to cookbooks, log a cook, delete.
- **Cook mode**: one step per page in big type, the ingredients that step uses, timers, and
  the screen kept awake.
- **Kitchen timers** ring as local notifications, so they work with Crumb closed or the phone
  locked. Running timers count down above the tab bar.
- **Add**: a link or pasted recipe, photos of a recipe card or cookbook page (camera or
  library, read by Wee Chef when the server has a vision model), or backup files.
- **Share → Crumb**: a share extension for Safari pages, text from Notes, and photos. It
  saves straight to your Crumb with the app's session.
- **Shelf** (cookbooks in the web's cloth colours), **Review** (Wee Chef's checks), and
  **More** (theme, full backup, sign out).
- **Theme**: light, dark, system, or sunrise & sunset (at your location, or estimated from
  your time zone), matching the web.
- **Offline**: the recipe box, cookbooks and every recipe you've opened, from the last copy
  on the phone, with photos from a disk cache.
- `crumb://recipes/12`, `crumb://cookbooks/3` and `crumb://add?text=…` links.

## Build

Needs a Mac with Xcode 16 or newer, [XcodeGen](https://github.com/yonaskolb/XcodeGen) and
Rust (rustup), because the app links `crumb-core`:

```bash
brew install xcodegen
cd ios
scripts/build-core.sh          # crumb-core for iPhone, simulator and Mac (--release, --quick)
xcodegen generate              # Crumb.xcodeproj from project.yml
open Crumb.xcodeproj           # run the Crumb scheme on a simulator
```

Re-run `build-core.sh` after changing `crates/crumb-core` or `crates/crumb-ffi`, and
`xcodegen generate` after adding files or changing `project.yml`. Neither output is committed.

From the command line:

```bash
(cd CrumbKit && swift test)    # CrumbKit's tests on the Mac, against the real core
xcodebuild test -project Crumb.xcodeproj -scheme Crumb \
  -destination "id=$(scripts/simulator.sh)"      # app unit + UI tests on a simulator
scripts/format.sh              # swift-format (scripts/format.sh --lint to check only)
```

The UI tests run the app with `-demo`, against a pretend server built into Debug builds
(`Crumb/Debug/DemoServer.swift`), so they need no network. Launch with `-demo` yourself to
try the app without a Crumb.

### On your own iPhone

Simulator builds need no Apple account. To run on a phone, copy
`Config/Local.xcconfig.example` to `Config/Local.xcconfig` and set your `DEVELOPMENT_TEAM`
(and `CRUMB_BUNDLE_ID` if `app.crumb.ios` is taken for your team), then `xcodegen generate`
and run from Xcode. The App Group (`group.<bundle id>`) and Keychain group are created by
automatic signing.

### Trying it against a local server

```bash
PORT=3000 APP_PASSWORD=test cargo run      # from the repo root
```

The simulator shares the Mac's network: sign in to `http://localhost:3000`. On a phone, use
the Mac's address on your Wi-Fi (`http://192.168.x.x:3000`). Plain HTTP is only allowed for
the device itself and private network addresses (crumb-core's `allowsCleartext`); anything
else must be HTTPS.

## crumb-core

Everything the web, Android and desktop apps also compute comes from the Rust core
(`crates/crumb-core`) through `crates/crumb-ffi` and [UniFFI](https://mozilla.github.io/uniffi-rs/),
never a Swift rewrite: ingredient scaling and fractions, step timers, the ingredients a step
uses, cook steps, durations ("1h 30m"), the shared recipe text, source links, cookbook
colours, photo URLs (`/img/{id}/{width}?v={fnv1a}`), server address rules, error wording,
link-or-text import classification, offline search, the sunrise & sunset theme, cook counts,
and Wee Chef's check wording.

`scripts/build-core.sh` builds `libcrumb_ffi.a` per platform (`cargo rustc --crate-type
staticlib`, so the crate itself stays a cdylib for Android), wraps them as
`CrumbKit/Frameworks/CrumbCoreFFI.xcframework`, and generates `CrumbKit/Sources/CrumbCore/CrumbCore.swift`.
`CrumbKit` re-exports it, so app code calls `CrumbCore.scaleIngredient(...)` or the Swift
helpers in `CrumbKit/Sources/CrumbKit/RecipeCore.swift`.

To add a function, export it from `crates/crumb-ffi/src/lib.rs` with `#[uniffi::export]`
and re-run `build-core.sh`; Android gets it too.

## How it fits together

```
ios/
  project.yml                XcodeGen spec: app, Share extension, unit and UI tests
  Config/Crumb.xcconfig      Bundle identifier and team (Local.xcconfig overrides)
  CrumbKit/                  Swift package: everything that isn't UI
    Sources/CrumbKit/
      CrumbAPI.swift         The server's REST API (src/api.rs) over URLSession; errors in crumb-core's words
      Models.swift           Codable shapes of the server's JSON
      SessionStore.swift     Server address (App Group defaults) + session cookie (Keychain)
      RecipeCache.swift      Last copy of lists, recipes and cookbooks; RecipeRepository (server first, cache offline)
      KitchenTimers.swift    Running timers, persisted; TimerAlarms schedules them with the system
      ThemeSettings.swift    Theme mode + sun location; crumb-core decides light or dark
      RecipeCore.swift       Recipe logic from crumb-core as Swift helpers
    Tests/                   API client against a stub server, stores, and the real core
  Crumb/                     The app: App/ (model, root, notifications), Screens/, Theme/, Debug/
  CrumbShare/                Share extension
  CrumbTests/, CrumbUITests/ Hosted unit tests and UI tests (demo server)
  scripts/                   build-core.sh, build-release.sh, format.sh, simulator.sh
```

- **Auth:** the same `crumb_session` cookie the website uses, sent by hand (URLSession never
  stores cookies). A 401 anywhere returns to sign-in; the offline copy stays for next time.
- **Theme:** colours, radii and fonts from `web/src/styles/app.css` and `web/src/assets/fonts`
  (Nunito Sans cut into static weights, DM Serif Display, Caveat). Butter is only the main
  action and the active tab.
- **Photos:** through the server's resizer with the session cookie, falling back to the original
  image URL as the web does; cached in memory and a 200 MB disk cache.

## CI and releases

- **`.github/workflows/ios.yml`** runs on every change to `ios/`, `crates/crumb-core`,
  `crates/crumb-ffi` or the Cargo files: crumb-core for Apple platforms, CrumbKit's tests,
  swift-format (it prints the patch to apply if anything isn't formatted), the app's unit and
  UI tests on a simulator, and an unsigned Release build for iPhone. The simulator app is kept
  as an artifact.
- **`.github/workflows/ios-release.yml`** builds a versioned Release (`scripts/build-release.sh`):
  every master push (from `main.yml`, uploaded to TestFlight) and every promotion (from
  `promote.yml`, attached to the GitHub Release). It can also be run by hand.
- **Versions** follow the server's: `3.1.0-main.57` ships as version 3.1.0 with a build number
  from the build time (`YYYYMMDD.HHMM`), so TestFlight always sees a newer build.

### Signing and TestFlight

Without Apple credentials the release build is unsigned (`crumb-ios-<version>-unsigned.ipa`).
To sign and upload, join the Apple Developer Program and add these repository secrets:

| Secret | What |
| --- | --- |
| `APPLE_TEAM_ID` | Your team ID (Membership details) |
| `APP_STORE_CONNECT_KEY_ID` | An App Store Connect API key (Users and Access → Integrations), **Admin** role so Xcode can manage certificates and profiles |
| `APP_STORE_CONNECT_ISSUER_ID` | The issuer ID shown with the key |
| `APP_STORE_CONNECT_PRIVATE_KEY` | The `.p8` file's contents |
| `IOS_CERTIFICATE_P12_BASE64`, `IOS_CERTIFICATE_PASSWORD` | Optional: your own distribution certificate, if you don't want cloud-managed signing |

Create the app in App Store Connect with the bundle ID `app.crumb.ios` (or set the repository
variable `CRUMB_IOS_BUNDLE_ID` to your own). The first upload makes the App Group and
Keychain group through automatic signing.

## Roadmap

1. ~~Sign in, recipe box with search, recipe, cook mode with step ingredients and timers,
   cookbooks, Review, Add (link, text, photos, files), Share → Crumb, offline reading, theme,
   CI with TestFlight~~
2. Recipe editor (today: Edit on the web)
3. Timers as Live Activities on the Lock Screen and Dynamic Island, and AlarmKit alarms on iOS 26
4. Save a recipe or cookbook for offline use, with its photos, ahead of time
5. Widgets (Try next) and Siri/App Shortcuts ("Start cooking…")
6. App Store: privacy details, screenshots from the demo mode, review
