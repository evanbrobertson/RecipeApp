# Releasing

Crumb ships as one Docker image per commit on `master`. The image is built once, tested in the Railway **dev** environment, and then promoted to **stable** (Railway `production`) by retagging it. Nothing is rebuilt on the way, so stable runs the exact bytes you tried in dev.

```
pull request ──► PR workflow: checks (+ Docker build if build inputs changed)
      │
   merge
      ▼
master push ──► Main workflow
                 ├─ checks ───────────────────────────┐
                 ├─ version (e.g. 3.1.0-main.42)      │
                 └─ image: build once, push to GHCR ──┤
                      sha-abc1234, 3.1.0-main.42      │
                      (web source maps → Sentry)      ▼
                                              deploy-dev: tag :main, Sentry release + dev deploy,
                                                          railway redeploy (dev)
you test dev, then run Promote (channel: stable)
      ▼
Promote ──► retag sha-abc1234 as :stable and :3.1.0, tag v3.1.0 + GitHub Release,
            move release/stable, finalize the Sentry release, railway redeploy (production)
```

## Workflows

| File | Trigger | What it does |
| --- | --- | --- |
| `.github/workflows/checks.yml` | called by the others | `cargo fmt --check`, clippy `-D warnings`, `cargo test`; web `bun install --frozen-lockfile`, `astro check`, oxlint, build; `site/` build when `site/package.json` exists. The Linux desktop app's clippy and smoke test run in an `archlinux:latest` container, because cxx-qt needs Qt 6.5+ |
| `.github/workflows/pr.yml` | pull request to `master` | The checks, plus a Docker build (no push) only when `Dockerfile`, `.dockerignore`, `Cargo.toml`/`Cargo.lock`, `web/package.json`/`web/bun.lock` or `web/astro.config.mjs` change. A release build of the server (fat LTO, one codegen unit) takes several minutes, and code-only changes are already covered by the checks and by the master build, so most PRs skip it |
| `.github/workflows/main.yml` | push to `master` (not docs-only) | Checks and image build in parallel; after both pass, `:main` moves to the new image, Sentry gets the release and a `dev` deploy, and Railway dev redeploys. Separately, a release build of the desktop executable is uploaded as a `crumb-desktop-linux-<version>` workflow artifact (30 days); it does not block the deploy |
| `.github/workflows/promote.yml` | manual (Actions → Promote → Run workflow) | Input `channel` (`stable` or `beta`) and optional `sha` (default: latest `master`). A commit without an image (a docs-only push) falls back to its newest ancestor with one, and the commit's checks must have passed. Retags, tags, releases, deploys; then attaches the desktop tarball, its `.sha256` and an AUR `PKGBUILD` to the release |

Master builds queue rather than cancel (concurrency group `main`); promotions run one at a time. Third-party actions are pinned to commit SHAs, and each job gets only the permissions it needs.

## Versions and tags

Git tags are the source of truth. `scripts/release/version.sh` reads the conventional commit messages since the last stable tag `vX.Y.Z`: a breaking change (`feat!:` or a `BREAKING CHANGE:` footer) bumps the major, `feat:` the minor, anything else the patch. With no tag yet, the first release is the version in `Cargo.toml` (currently 3.0.0). `Cargo.toml` is not bumped by CI, so treat its version as the starting point only.

| Image tag | Moves? | Set by |
| --- | --- | --- |
| `sha-<7 chars>` | never | Main |
| `<next>-main.<run>` (e.g. `3.1.0-main.42`) | never | Main |
| `main` | every master build | Main (after checks pass) |
| `X.Y.Z` / `X.Y.Z-beta.N` | never | Promote |
| `stable` / `beta` | every promotion | Promote |

The `-main.<run>` version is the build's identity everywhere: it is the image label `org.opencontainers.image.version`, `SENTRY_RELEASE` inside the image, and the release name in Sentry. The GitHub Release and git tag `vX.Y.Z` say which build they are.

**Rollback:** run Promote with the `sha` of an older commit that already has a stable tag. The tag and release are reused, and `:stable` and Railway move back to that image. A commit older than the newest stable tag that was never released is refused.

**Beta** exists as a channel (tag `vX.Y.Z-beta.N`, a GitHub pre-release, image tag `:beta`, branch `release/beta`). It deploys only once a `beta` GitHub environment with a `RAILWAY_TOKEN` exists; until then it only tags.

## Desktop (Linux)

The native Qt6/QML desktop app (`desktop/linux`, binary `crumb-desktop`) shares the server's version stream: a desktop change merged to `master` bumps the same `vX.Y.Z`, so there is one release per version, not one per artifact.

- **Dev builds.** Every master push builds `crumb-desktop` in release mode and uploads `crumb-desktop-linux-x86_64-<version>.tar.gz` plus its `.sha256` as the `crumb-desktop-linux-<version>` workflow artifact (30 days). It is a separate job from the server image and never blocks or changes the dev deploy. This needs Qt 6.5+, so it runs in an `archlinux:latest` container (`build-tarball.sh` assembles the tarball from the already-built binary).
- **Promote.** After the image is retagged and the GitHub Release made, a `desktop-release` job builds the same commit, renders the AUR `PKGBUILD` from `PKGBUILD.in`, and uploads the tarball, its `.sha256` and the `PKGBUILD` to the release. It runs for both `stable` and `beta`, after the server promotion, so a failure there never rolls it back.
- **Tarball layout.** `crumb-desktop/{crumb-desktop, crumb-desktop.desktop, crumb-desktop.png, install.sh, README.txt, LICENSE}`. `install.sh` installs per user under `~/.local` (and supports `--uninstall`); the PKGBUILD is the AUR `crumb-desktop-bin` package for a system install.

## Android

The Android app (`android/`, with `crumb-core` linked through `crates/crumb-ffi`) rides the same version too.

- **Checks.** `android.yml` runs on every change to `android/`, `crates/crumb-core`, `crates/crumb-ffi` or the Cargo files: it cross-compiles the core for arm64-v8a, armeabi-v7a and x86_64, generates the Kotlin bindings, runs the JVM tests against the real core, lints, builds debug and release APKs, and fails if an ABI is missing `libcrumb_ffi.so`. The toolchain (JDK, SDK, NDK, Rust targets, cargo-ndk) is one composite action, `.github/actions/android-setup`.
- **Dev builds.** Every master push builds `crumb-android-<version>.apk` plus `.sha256` (`android/scripts/build-release-apk.sh`) as the `crumb-android-<version>` workflow artifact (30 days), beside the deploy, never blocking it.
- **Promote.** An `android-release` job builds the promoted commit and uploads the APK and its `.sha256` to the GitHub Release, after the server promotion.
- **Version code.** Derived from the version so newer always installs over older: `X.Y.Z` → `X·10⁷ + Y·10⁴ + Z·10 + 9`, and a master build (`-main.N`) ends in 0 instead, so the promoted release replaces its dev builds.
- **Signing.** With the `CRUMB_KEYSTORE_BASE64`, `CRUMB_KEYSTORE_PASSWORD`, `CRUMB_KEY_ALIAS` and `CRUMB_KEY_PASSWORD` secrets the APK is signed; without them it is built and named `-unsigned` (see `android/README.md`).

## iOS

The iPhone app (`ios/`, with `crumb-core` linked through `crates/crumb-ffi` as an XCFramework) rides the same version.

- **Checks.** `ios.yml` runs on every change to `ios/`, `crates/crumb-core`, `crates/crumb-ffi` or the Cargo files, on macOS runners: it builds the core for iPhone, the simulator and the Mac, runs CrumbKit's tests against the real core, checks swift-format (printing the patch when something isn't formatted), runs the app's unit and UI tests on a simulator against the built-in demo server, and builds Release for iPhone unsigned. The toolchain (Xcode, Rust Apple targets, XcodeGen) is one composite action, `.github/actions/ios-setup`.
- **Dev builds.** Every master push runs `ios-release.yml`: a versioned Release archive (`ios/scripts/build-release.sh`), uploaded to **TestFlight** when the App Store Connect secrets below exist, and kept as the `crumb-ios-<version>` workflow artifact (30 days). It never blocks deploy-dev.
- **Promote.** An `ios-release` job builds the promoted commit and attaches `crumb-ios-<version>.ipa` (or `-unsigned.ipa`) and its `.sha256` to the GitHub Release. Submitting a TestFlight build for App Store review is done in App Store Connect.
- **Version and build.** `3.1.0-main.57` ships as version `3.1.0`; the build number is the build's UTC time, `YYYYMMDD.HHMM`, so each upload is newer than the last.
- **Signing.** Cloud-managed through an App Store Connect API key (`APPLE_TEAM_ID`, `APP_STORE_CONNECT_KEY_ID`, `APP_STORE_CONNECT_ISSUER_ID`, `APP_STORE_CONNECT_PRIVATE_KEY`; optionally `IOS_CERTIFICATE_P12_BASE64` and `IOS_CERTIFICATE_PASSWORD`). Without them the build is unsigned. See `ios/README.md`.

## Secrets and variables

Repository secrets (Settings → Secrets and variables → Actions):

| Secret | Needed for | Notes |
| --- | --- | --- |
| `SENTRY_AUTH_TOKEN` | source maps, releases, deploys | A Sentry **organization auth token** (Settings → Developer Settings → Organization Tokens) for `team-evan`. Optional: without it the build makes no source maps and the Sentry steps are skipped |
| `RELEASE_TOKEN` | moving `release/*` | Optional. `GITHUB_TOKEN` cannot update a branch across commits that change `.github/workflows`, so Promote only warns when that happens. A fine-grained PAT for this repo with **Contents: read and write** and **Workflows: read and write** fixes it |

Environment secrets and variables (Settings → Environments). The workflows use GitHub environments `dev`, `stable` and (later) `beta`; GitHub creates them on first use, but create them up front to add the secrets:

| Environment | Secret `RAILWAY_TOKEN` | Variable `RAILWAY_ENVIRONMENT` | Variable `APP_URL` (optional) |
| --- | --- | --- | --- |
| `dev` | Railway project token for the `dev` environment | `dev` (default) | dev URL, shown on the deployment |
| `stable` | Railway project token for the `production` environment | `production` (default) | the app's URL |

`RAILWAY_SERVICE` (variable, default `RecipeApp`) overrides the service name. `GITHUB_TOKEN` pushes images to GHCR; nothing else is needed for that. The Sentry org and project (`team-evan` / `crumb-recipes`) and the image name are set at the top of `main.yml` and `promote.yml`.

## One-time setup

### GitHub

```bash
# Environments (a required reviewer on stable is optional)
gh api -X PUT repos/evanbrobertson/RecipeApp/environments/dev
gh api -X PUT repos/evanbrobertson/RecipeApp/environments/stable

# Secrets (each command prompts for the value)
gh secret set SENTRY_AUTH_TOKEN --repo evanbrobertson/RecipeApp
gh secret set RAILWAY_TOKEN --repo evanbrobertson/RecipeApp --env dev
gh secret set RAILWAY_TOKEN --repo evanbrobertson/RecipeApp --env stable
gh secret set RELEASE_TOKEN --repo evanbrobertson/RecipeApp   # optional, see above

# Optional variables
gh variable set APP_URL --repo evanbrobertson/RecipeApp --env dev --body https://<dev-domain>
gh variable set APP_URL --repo evanbrobertson/RecipeApp --env stable --body https://<prod-domain>
```

After the first Main run, open the `recipeapp` package (github.com/evanbrobertson?tab=packages → recipeapp → Package settings) and check that it is linked to the repository and **public**, so Railway can pull it without credentials. Change it with **Change visibility → Public** if needed.

For Sentry to list the commits in each release, install the GitHub integration in Sentry (Settings → Integrations → GitHub) and add this repository. Without it, the release is still created and the commit step logs a warning.

### Railway

Railway project tokens are scoped to one environment. Create one per environment in the dashboard: Project **Recipes** → Settings → Tokens → pick the environment (`dev`, then `production`). Those are the two `RAILWAY_TOKEN` secrets above.

Run these once with the Railway CLI (logged in with your account; `railway link --project Recipes` in the repo first, or add `--project <id>` to each command). Do the dev environment first and confirm it works before switching production.

```bash
# 1. A dev environment copied from production (variables and service settings)
railway environment new dev --duplicate production

# 2. Dev's own volume and settings. A duplicated environment gets its own, empty volume;
#    check, and add one if it is missing:
railway volume list --environment dev
railway volume add --mount-path /data --service RecipeApp --environment dev   # only if missing

# 3. Dev variables: its own name in Sentry, and its own domain
railway variable set SENTRY_ENVIRONMENT=dev --service RecipeApp --environment dev
railway variable delete SITE_URL --service RecipeApp --environment dev   # if production sets a custom domain
railway domain --service RecipeApp --environment dev                     # generate a dev domain

# 4. Serve dev from the image, sleeping when idle. With an image source railway.json is not
#    read, so set the health check here too.
railway environment edit --environment dev \
  --service-config RecipeApp source.image ghcr.io/evanbrobertson/recipeapp:main \
  --service-config RecipeApp deploy.sleepApplication true \
  --service-config RecipeApp deploy.healthcheckPath /api/health \
  --service-config RecipeApp deploy.healthcheckTimeout 60 \
  --service-config RecipeApp deploy.restartPolicyType ON_FAILURE \
  --message "Serve dev from GHCR :main"

# 5. Sentry for both environments (DSNs are public; the browser gets it from the server)
railway variable set SENTRY_DSN=https://5673198da635c1da800169122b8ec4c4@o4508773394153472.ingest.us.sentry.io/4512148583481344 \
  --service RecipeApp --environment dev
railway variable set SENTRY_DSN=https://5673198da635c1da800169122b8ec4c4@o4508773394153472.ingest.us.sentry.io/4512148583481344 \
  SENTRY_ENVIRONMENT=stable --service RecipeApp --environment production --skip-deploys

# 6. After a Promote to stable has tagged :stable at least once, switch production from the
#    GitHub repo (which auto-deploys every push) to the image. Promote the current master
#    first so :stable exists, then:
railway environment edit --environment production \
  --service-config RecipeApp source.image ghcr.io/evanbrobertson/recipeapp:stable \
  --service-config RecipeApp deploy.healthcheckPath /api/health \
  --service-config RecipeApp deploy.healthcheckTimeout 60 \
  --service-config RecipeApp deploy.restartPolicyType ON_FAILURE \
  --message "Serve production from GHCR :stable"

# Check what each environment now runs
railway environment config --environment dev --json
railway environment config --environment production --json
```

If step 4 or 6 leaves the GitHub repo connected as well (the config shows `source.repo`), disconnect it for that environment in the dashboard (service → Settings → Source → Disconnect). Otherwise pushes to `master` still deploy to production directly. The production volume at `/data` and its database stay as they are.

Order for the first rollout: merge this change (it still auto-deploys production from the repo), check that Main builds and deploys dev, run **Promote → stable** once so `:stable` and `v3.0.0` exist, then do step 6.

### How CI deploys

CI calls `railway redeploy --from-source --service RecipeApp --environment <env> --yes` from the pinned `ghcr.io/railwayapp/cli` image, with that environment's project token. For an image-sourced service, `--from-source` makes Railway pull the configured tag (`:main` or `:stable`) again, which the workflow has just moved to the new image. If you would rather pin each deploy to an exact image, the same token can run `railway environment edit --environment <env> --service-config RecipeApp source.image ghcr.io/evanbrobertson/recipeapp:sha-<short>` instead.

## Day to day

1. Open a PR with conventional commit messages (`feat:`, `fix:`, `feat!:` ...). Checks must pass.
2. Merge. Main builds the image and deploys it to dev (about 10 minutes with a warm cache).
3. Try it on dev. Errors show up in Sentry under environment `dev` and release `X.Y.Z-main.N`.
4. Actions → **Promote** → channel `stable` → Run. It creates `vX.Y.Z` and a GitHub Release, and production redeploys.

## Sentry

- **Server:** `src/telemetry.rs`. On only when `SENTRY_DSN` is set. Errors and panics become events, warnings become their breadcrumbs, and routed requests become transactions sampled at `SENTRY_TRACES_SAMPLE_RATE` (default 0.1; health checks, photos and static files are never traced). Cookies, auth headers, IPs, query strings and bodies are stripped.
- **Browser:** `web/src/lib/sentry-boot.ts`. The server puts the DSN, environment and release in a `Server-Timing` header on HTML responses, so one image serves every environment and a copy without `SENTRY_DSN` loads no Sentry code. The SDK loads after the page is idle (tracing 0.2 on stable, 1.0 elsewhere), and Session Replay loads after that (5% of sessions, every session with an error, all text masked and all media blocked).
- **Source maps:** made only when `SENTRY_AUTH_TOKEN` is present at build time (CI passes it to `docker build` as a BuildKit secret), uploaded with debug IDs during `bun run build`, then deleted, so they are never served.
- **Releases:** Main creates the release (`X.Y.Z-main.N`) with its commits and a `dev` deploy. Promote adds a `stable` (or `beta`) deploy and, for stable, finalizes the release.
