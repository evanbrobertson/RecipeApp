# Crumb for Android

A native Android app for Crumb (issue #9): Kotlin and Jetpack Compose, no WebView. It is a
client for your own Crumb server. Scraping, Wee Chef and the recipe database stay on the
server; the phone keeps a copy of what it has shown so recipes open without a connection.

## Build

Needs JDK 17, the Android SDK (platform 37, build-tools 36.1) and NDK 28, and Rust with the
Android targets and cargo-ndk, because the app links `crumb-core`:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
sdkmanager "ndk;28.2.13676358"
```

```bash
cd android
./gradlew :app:assembleDebug        # app/build/outputs/apk/debug/app-debug.apk
./gradlew :app:installDebug         # onto a connected phone (adb)
./gradlew :app:testDebugUnitTest    # JVM unit tests
./gradlew :app:lintDebug
./gradlew :app:assembleRelease      # minified; signed if a keystore is configured
```

Set `JAVA_HOME` and `ANDROID_HOME` (or `sdk.dir` in `local.properties`) first; the NDK is
found under the SDK, or set `ANDROID_NDK_HOME`. Every build cross-compiles the core for
arm64-v8a, armeabi-v7a and x86_64; add `-PcrumbAbis=arm64-v8a` for a quicker loop on a phone.

### crumb-core

Scaling, mise en place, step timers, step ingredients, durations and text export are the
same Rust code the server and desktop app use (`crates/crumb-core`), exposed to Kotlin by
`crates/crumb-ffi` with [UniFFI](https://mozilla.github.io/uniffi-rs/). Gradle runs it all
before `preBuild`:

- `cargoBuildCoreAndroid`: `cargo ndk` builds `libcrumb_ffi.so` per ABI into `app/build/rustJniLibs`
- `cargoBuildCoreHost` + `generateCoreBindings`: a host build of the same library, from which
  UniFFI generates `app.crumb.core` into `app/build/generated/uniffi` (nothing generated is
  committed). The JVM unit tests load that host library through JNA, so they exercise the
  real core, not a mock.

To add a function, export it from `crates/crumb-ffi/src/lib.rs` with `#[uniffi::export]`
(records with `#[derive(uniffi::Record)]`; recipes cross as the API's JSON) and rebuild.

### Trying it against a local server

Release builds only talk HTTPS, except to the phone itself. To use a server on your
computer, tunnel it over USB:

```bash
PORT=3000 APP_PASSWORD=test cargo run      # from the repo root
adb reverse tcp:3000 tcp:3000
```

Then sign in to `http://localhost:3000`. Debug builds also accept plain HTTP on the local
network (`http://192.168.x.x:3000`).

## Release signing

Unsigned unless a keystore is configured. Locally, create `android/keystore.properties`
(git-ignored):

```properties
storeFile=/absolute/path/to/crumb-release.jks
storePassword=…
keyAlias=crumb
keyPassword=…
```

In GitHub Actions, set the repository secrets `CRUMB_KEYSTORE_BASE64` (the `.jks`, base64),
`CRUMB_KEYSTORE_PASSWORD`, `CRUMB_KEY_ALIAS` and `CRUMB_KEY_PASSWORD`. Keep the keystore
safe: Android only installs updates signed with the same key. The version code comes from
the release version (see docs/RELEASING.md), so a newer APK always installs over an older one.

## How it fits together

The app is the web app's phone layout, screen for screen: Home, Recipes, Shelf, Review
(Wee Chef suggestions) and More in the tab bar, and the recipe, cook mode, mise en place,
editor, cookbook, import and Claude-connector screens behind them. Android 9 (API 28) and up.

```
app/src/main/java/app/crumb/android/
  CrumbApplication.kt   AppContainer: HTTP, session, cache, repository, importer, timers, stores
  MainActivity.kt       Edge-to-edge Compose host; takes Share → Crumb text, photos and files
  data/
    CrumbApi.kt         The whole REST API the web uses (src/api.rs, src/share.rs), cookie auth
    Models.kt           The server's JSON shapes
    SessionStore.kt     Server address + session cookie, encrypted with an Android Keystore key
    RecipeCache.kt      Last copy of lists, recipes and cookbooks as JSON, one folder per server
    RecipeRepository.kt Server first, cache when offline
    LocalState.kt       What the web keeps in browser storage: scale, cook step, prep progress,
                        recently viewed, Surprise me's recent picks
    Importer.kt         Links, text, photos and files in; Incoming.kt reads Share intents
    PhotoImport.kt      Photos → JPEG ≤1568px → Wee Chef, or ML Kit on the phone when the
                        server can't read photos (the web's tesseract.js path)
    Photos.kt           /img/{id}/{width}?v={fnv1a}, the same URLs as web/src/lib/img.ts
    Durations.kt        ISO times via crumb-core, shown the way the server writes them
  timers/               Kitchen timers: exact alarms, countdown + ringing notifications,
                        restored after a reboot; the in-app dock
  ui/
    components/         The web's shared classes as Compose: buttons, cards, rows, chips,
                        modal, menu, input, toasts, tile surface, empty states
    theme/              Green Tile tokens, fonts, light/dark/system/sunrise & sunset
    books/              The 3D bookshelf and open book, ported from books.ts
    …                   One package per screen
```

- **Auth:** the same `crumb_session` cookie the website uses. The password is never stored.
  A 401 on any request returns to sign-in; the cache stays for when you sign back in.
- **Theme:** colours, radii and fonts come from `web/src/styles/app.css` and
  `web/src/assets/fonts` (converted to TTF); icons are Lucide, like the web. "Sunrise &
  sunset" (the default) runs the web's suncalc port on the phone's time zone or saved location.
- **Photos:** Coil, through the server's resizer with the session cookie; falls back to the
  original image URL, then the web's striped placeholder.
- **Timers:** "Start 10 minutes timer" in cook mode sets an alarm-clock alarm, so it rings on
  time in Doze and with the app closed, with a live countdown notification. Android 13+ asks to
  post notifications the first time; if exact alarms aren't allowed, the app offers the
  setting (they still ring, a little late). Timers come back after a reboot.
- **Shared logic:** anything the web and desktop apps also compute comes from `crumb-core`
  through `app.crumb.core`, never a Kotlin rewrite.

## Roadmap

1. ~~Sign in, recipe box with search, recipe, cook mode, cookbooks, Add with Share → Crumb,
   offline reading of what's been opened, CI~~
2. ~~`crumb-core` through UniFFI~~
3. Parity with the web's phone layout (in progress): kitchen timers, photo and file import and
   Share → Crumb for them, the 3D shelf, Home, recipes with filters and select, the recipe page
   with sharing and Wee Chef checks, cook mode, mise en place, the editor, suggestions, More
4. Save a recipe or cookbook for offline use, with its photos
5. Play Store: signing key, listing, privacy policy
