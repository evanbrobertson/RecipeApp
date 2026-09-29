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
- **Reading** (`src/background.ts`, `src/page.ts`): when you say yes, or click the toolbar
  button (on any page, recipe or not), the extension reads the recipe from the page in your
  browser and opens `{your Crumb}/preview?url={the page}&via=extension` in a tab beside it. Many
  recipe sites turn Crumb's server away with a bot check but not your browser, which is already
  past it; this is how those recipes get in. What it reads is only the recipe: the page's
  `application/ld+json` blocks that hold a schema.org `Recipe` or, if none does, the recipe
  card's HTML (WP Recipe Maker, Tasty Recipes, Mediavine Create, Zip Recipes, WPZOOM or Recipe
  microdata, with scripts, forms and every attribute the parser doesn't use removed), plus the
  page's `og:image` and `og:title`, up to 512 KB (a bigger recipe isn't sent, and Crumb reads the
  link itself). It sends **the link plus the recipe it found on the page, and only to your
  Crumb**. It never reads or sends cookies, storage, sign-ins or the rest of the page.
- **Handing it over**: like a YouTube video's words, it waits in session storage for that one
  tab; the content script on your Crumb's tab posts it to the page (`window.postMessage`, same
  origin), and the page sends it to your Crumb with the signed-in session. The extension never
  makes a request of its own, so it needs no host permissions and holds no credentials. If nothing
  could be read, the preview reads the link as before.
- **The preview** is the server's (`src/preview.rs`): a "Reading…" page at once, then the
  recipe, or why it couldn't be read. A link already in your box opens that recipe instead.
  **Add to my Crumb** saves it through the usual import, reusing what the preview showed.
- **YouTube videos** (`src/youtube.ts`): YouTube often turns Crumb's server away ("Sign in to
  confirm you're not a bot"), so on a YouTube video the extension reads the video's page in your
  browser: its title, description and the words of its captions (the caption track, else the
  "Show transcript" panel). The toolbar button, or the card it shows on videos that mention a
  recipe or ingredients, opens your Crumb's Add page with the link filled in, and hands that page
  the text, and the import starts. The words come from the listed caption track, else the "Show
  transcript" panel, else the player's own caption request (the CC button is pressed and put back
  if the player hasn't fetched captions yet); each miss is logged to the YouTube tab's console as
  `[Crumb] …`. When nothing could be read, the Add page says so. Your YouTube sign-in is never read or sent, and the
  requests to YouTube are the ones its own page makes.
- **Saying it's here** (`src/content.ts`): on a Crumb page it sets `data-crumb-extension` (the
  extension's version) on `<html>`, so the site can tell (`web/src/lib/extension.ts`).
- **Setting up**: install it, then open your Crumb while signed in and choose **Use this Crumb**
  on the card it shows (Crumb pages carry `<meta name="application-name" content="Crumb">`), or
  type the address on the settings page.

Permissions: `storage` (your Crumb's address and settings, synced with your browser profile),
`activeTab` (the page's address when you click the toolbar button) and `scripting` (on that click,
reading a page or YouTube tab that was open before the extension was installed or updated). No
host permissions. The content script runs on all http(s) pages because noticing recipes as they
open is the point; it only looks, and reads the recipe out of the page only when you click.

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
