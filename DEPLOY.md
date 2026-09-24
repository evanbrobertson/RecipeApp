# Deploying to Railway

The app is a single Node server with an SQLite file, so it needs one Railway service and one volume.
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
| `NUXT_APP_PASSWORD`      | A long password. **Required**: without it the app is public           |
| `NUXT_ANTHROPIC_API_KEY` | Optional. Claude parses pasted text instead of the heuristic parser   |
| `NUXT_PUBLIC_SITE_URL`   | Only needed with a custom domain (e.g. `https://recipes.example.com`) |

```bash
railway variables --set NUXT_APP_PASSWORD='something-long-and-random'
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
- **Some sites block scrapers** (403). Copy the recipe text and paste it instead, or ask Claude to save it.
- The app is meant for one person or household and runs as a single instance. Don't scale it to multiple replicas, because SQLite lives on one volume.
