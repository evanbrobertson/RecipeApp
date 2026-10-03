# Crumb for ChatGPT

The plugin package for OpenAI's plugin format: `plugin.json` (listing, review cases), `mcp.json` (the one remote
server, `{origin}/mcp/chatgpt`) and the icons, which are copied from `web/public` when it is built. The server side
is `mcp::Flavor::ChatGpt` and the shelf widget (`web/widget`); see CLAUDE.md.

## Build a package

```bash
bun plugin/chatgpt/build.mjs --origin https://crumb-dev.up.railway.app      # a -dev ZIP
bun plugin/chatgpt/build.mjs --origin https://<publish domain> --developer "<verified name>" --demo-url <video>
bun test plugin/chatgpt
```

The ZIP lands in `plugin/chatgpt/dist/`. The origin is never written in the templates: **the MCP origin of a published
plugin can never change**, so a development origin (`*.railway.app`, localhost) only makes a `-dev` package, and any
other origin must be https with every placeholder filled and the listing inside OpenAI's limits.

## Try it in ChatGPT (developer mode)

You don't need the ZIP to try it. Turn on **Settings → Security and login → Developer mode**, open
chatgpt.com/plugins, choose the plus button and paste `https://<your crumb>/mcp/chatgpt`. Approve the connection with
your Crumb password. Crumb appears in the sidebar as the shelf; in a chat, paste a recipe link and ask for it to be saved.
A local `plugin_asdk_app…` ID from that step is what `@plugin-creator` wants to make a personal marketplace entry.

## Before submitting to the public directory

Not done yet, and the build refuses a real origin until the first four are filled:

- [ ] **Publish domain** decided (it is permanent), with `OPENAI_APPS_CHALLENGE` set from the portal's domain check.
- [ ] **Pages** at `{origin}/privacy`, `/terms` and `/support`: Crumb has none yet, and the review needs all of them.
- [ ] **Verified developer identity** at OpenAI (the `--developer` name) and a **demo video** (`--demo-url`).
- [ ] **Reviewer account** that signs in without MFA or email codes (entered in the portal, never in the ZIP), with sample recipes.
- [ ] `_meta.ui.domain` for the widget (a dedicated origin, unique per plugin), added in `src/mcp_ui.rs`, and screenshots.
- [ ] The five positive and three negative review cases in `plugin.json` run against the real thing; they are written, not run.
- [ ] `category` ("Lifestyle") matches one of the dashboard's categories.
- [ ] OpenAI's rule against scraping "without proper authorization" is why this plugin's tools never fetch pages
      themselves (ChatGPT reads them); keep it that way.
