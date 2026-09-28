# Crumb browser extension

Open a recipe anywhere on the web and the extension asks **"Read this recipe in Crumb?"**. Say
yes and your Crumb opens it in a new tab: read and tidied as an import would be, in the same
layout as a shared recipe, with **Add to my Crumb**. Nothing is saved until you press it.

Chrome, Edge (and other Chromium browsers) and Firefox 140+ (Manifest V3).

## How it works

- **Noticing a recipe** (`src/content.ts`, `src/detect.ts`): on every http(s) page, it looks for
  a schema.org `Recipe` in the page's JSON-LD (including `@graph`s and lists) or microdata: the
  same data Crumb's scraper reads first, so a page that asks is a page Crumb can read. This
  happens in the page; nothing is sent anywhere.
- **Asking** (`src/toast.ts`): a small card in a closed shadow root, so the page's styles can't
  touch it. "Not on this site" stops it asking there; the settings page lists those sites.
- **Reading** (`src/background.ts`): opens `{your Crumb}/preview?url={the page}` in a tab beside
  the page. That is the only thing the extension ever sends, and only to your Crumb. The toolbar
  button does the same for any page, recipe or not.
- **The preview** is the server's (`src/preview.rs`): a "Reading…" page at once, then the
  recipe, or why it couldn't be read. A link already in your box opens that recipe instead.
  **Add to my Crumb** saves it through the usual import, reusing the page the preview read.
- **Setting up**: install it, then open your Crumb while signed in and choose **Use this Crumb**
  on the card it shows (Crumb pages carry `<meta name="application-name" content="Crumb">`), or
  type the address on the settings page.

Permissions: `storage` (your Crumb's address and settings, synced with your browser profile) and
`activeTab` (the page's address when you click the toolbar button). The content script runs on
all http(s) pages because noticing recipes as they open is the point.

## Develop

```bash
bun install
bun run build        # dist/chrome and dist/firefox, unpacked
bun test             # detection, settings and manifest
bun run check        # tsc
bun run lint         # oxlint, then web-ext lint on the Firefox build
bun run format       # oxfmt
```

Load the build unpacked: Chrome → `chrome://extensions` → Developer mode → **Load unpacked** →
`dist/chrome`. Firefox → `about:debugging#/runtime/this-firefox` → **Load Temporary Add-on** →
`dist/firefox/manifest.json`. Run `cargo run` for a Crumb on `http://localhost:3000` to point it
at.

## Releases

Its own versions (`extension-vX.Y.Z`), released from `master` by
`.github/workflows/extension.yml`: a GitHub Release with both zips, and the Chrome Web Store and
addons.mozilla.org when their keys are set. See [docs/RELEASING.md](../docs/RELEASING.md#browser-extension).
A Firefox zip from a GitHub Release loads only as a temporary add-on; the signed version comes
from addons.mozilla.org.
