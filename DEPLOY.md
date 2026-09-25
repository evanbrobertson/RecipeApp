# Deploying to Railway

Crumb is a single Rust binary serving a static Astro frontend and an SQLite file, so it needs one Railway service and one volume.
The repo includes a `Dockerfile` and `railway.json` (health check on `/api/health`).

## 1. Create the service

- **Dashboard:** New Project → Deploy from GitHub repo → pick this repo.
- **CLI:**

  ```bash
  npm i -g @railway/cli
  railway login
  railway init
  railway up
  ```

Railway detects `railway.json` and builds with the Dockerfile.

## 2. Attach a volume (required)

Railway's filesystem is wiped on every deploy. Add a volume to the service (right-click the service →
**Attach volume**, or `railway volume add`). Any mount path works (for example `/data`): the app reads
`RAILWAY_VOLUME_MOUNT_PATH` and stores `recipes.db` there. If no volume is attached, the logs show a warning.

## 3. Set variables

| Variable                 | Value                                                                 |
| ------------------------ | --------------------------------------------------------------------- |
| `APP_PASSWORD`      | A long password. **Required**: without it the app is public           |
| `ANTHROPIC_API_KEY` | Optional. Claude parses pasted text instead of the heuristic parser   |
| `SITE_URL`          | Only needed with a custom domain (e.g. `https://recipes.example.com`) |

The older `NUXT_APP_PASSWORD`, `NUXT_ANTHROPIC_API_KEY`, `NUXT_ANTHROPIC_MODEL` and
`NUXT_PUBLIC_SITE_URL` names are still read as fallbacks.

```bash
railway variables --set APP_PASSWORD='something-long-and-random'
```

## 4. Generate a domain

Service → Settings → Networking → **Generate Domain**. The app listens on Railway's `PORT` automatically.

## 5. Connect Claude

Open `https://<your-domain>/connect`, copy the connector URL (`https://<your-domain>/mcp`) and add it in
Claude under **Settings → Connectors → Add custom connector**. Approve with your app password.

## Notes

- **Moving existing recipes:** copy your old `sqlite.db` onto the volume as `recipes.db` (for example with
  `railway ssh`). The schema upgrades itself on start and keeps recipes and cookbooks.
- **Backups:** Railway volumes support backups. SQLite runs in WAL mode, so back up `recipes.db`,
  `recipes.db-wal` and `recipes.db-shm` together, or run `sqlite3 recipes.db ".backup backup.db"`.
- **Changing the password** signs out every browser. Claude's connector keeps working until you remove it.
- **Sites that block scrapers:** the image includes Chromium, and the app retries blocked or
  JavaScript-rendered pages in a real headless browser. This gets past simple bot filters, but big
  bot-protection services (Cloudflare, DataDome) can still spot headless browsers and datacenter IPs. When
  that happens, paste the recipe text instead or ask Claude to save it. Chromium adds about 250 MB to the
  image and briefly uses 200–300 MB of RAM per blocked import. To skip it, set the Docker build arg
  `WITH_CHROMIUM=false` (Railway: add it as a service variable) or set `BROWSER_SCRAPING=off`.
- The app is meant for one person or household and runs as a single instance. Don't scale it to multiple replicas, because SQLite lives on one volume.
