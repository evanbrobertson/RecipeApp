# Crumb for Android

A native Android app for Crumb (issue #9): Kotlin and Jetpack Compose, no WebView. It is a
client for your own Crumb server. Scraping, Wee Chef and the recipe database stay on the
server; the phone keeps a copy of what it has shown so recipes open without a connection.

## Build

Needs JDK 17 and the Android SDK (platform 37, build-tools 36.1).

```bash
cd android
./gradlew :app:assembleDebug        # app/build/outputs/apk/debug/app-debug.apk
./gradlew :app:installDebug         # onto a connected phone (adb)
./gradlew :app:testDebugUnitTest    # JVM unit tests
./gradlew :app:lintDebug
./gradlew :app:assembleRelease      # minified; signed if a keystore is configured
```

Set `JAVA_HOME` and `ANDROID_HOME` (or `sdk.dir` in `local.properties`) first.

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
safe: Android only installs updates signed with the same key. The version code is the
workflow run number.

## How it fits together

```
app/src/main/java/app/crumb/android/
  CrumbApplication.kt   AppContainer: HTTP client, session, cache, repository, image loader
  MainActivity.kt       Edge-to-edge Compose host; receives Share → Crumb text
  data/
    CrumbApi.kt         The server's REST API (src/api.rs), cookie auth, error messages
    SessionStore.kt     Server address + session cookie, encrypted with an Android Keystore key
    RecipeCache.kt      Last copy of lists, recipes and cookbooks as JSON, one folder per server
    RecipeRepository.kt Server first, cache when offline
    Photos.kt           /img/{id}/{width}?v={fnv1a}, the same URLs as web/src/lib/img.ts
    Durations.kt, ImportInput.kt
  ui/                   One package per screen; theme/ holds the Green Tile tokens
```

- **Auth:** the same `crumb_session` cookie the website uses. The password is never stored.
  A 401 on any request returns to sign-in; the cache stays for when you sign back in.
- **Theme:** colours, radii and fonts come from `web/src/styles/app.css` and
  `web/src/assets/fonts` (converted to TTF). Butter is only the main action and the active tab.
- **Photos:** Coil, through the server's resizer with the session cookie; falls back to the
  original image URL like the web does.
- **Shared logic:** scaling, parsing and ranking belong to `crumb-core` (Rust) and will come in
  through UniFFI rather than being rewritten in Kotlin.

## Roadmap

1. ~~Sign in, recipe box with search, recipe, cook mode, cookbooks, Add with Share → Crumb,
   offline reading of what's been opened, CI~~
2. Kitchen timers: AlarmManager exact alarms and notifications, surviving screen-off,
   process death and reboot; start a timer from a cook step
3. Save a recipe or cookbook for offline use, with its photos
4. Photo import (camera / photo picker → `/api/recipes/import/photos`)
5. `crumb-core` through UniFFI: ingredient scaling and mise en place in cook mode
6. Editing, cookbook management, Wee Chef suggestions
7. Play Store: signing key, listing, privacy policy
