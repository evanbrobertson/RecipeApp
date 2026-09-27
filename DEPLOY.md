# Deploying to Railway

Crumb is a single Rust binary serving a static Astro frontend and an SQLite file, so it needs one Railway service and one volume.
The repo includes a `Dockerfile` and `railway.json` (health check on `/api/health`).

These steps deploy straight from the GitHub repo, which is the simplest setup for your own copy. The hosted
Crumb instead runs prebuilt images from GHCR, with a dev environment and a manual promotion to production:
see [docs/RELEASING.md](./docs/RELEASING.md). With an image source, `railway.json` is not read, so its health
check is set on the service instead.

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
| `ANTHROPIC_API_KEY` | Optional. Turns on Wee Chef: it parses pasted text instead of the heuristic parser |
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
- **Sites that block scrapers:** pages are fetched with Firefox's and then Safari's TLS and HTTP/2
  fingerprint, which most bot filters accept (from a home connection; datacenter IPs are judged more
  harshly). The image also includes Chromium, and the app retries pages that are still blocked, or
  JavaScript-rendered, in a real headless browser. This gets past simple bot filters, but big
  bot-protection services (Cloudflare, DataDome) can still spot headless browsers and datacenter IPs. When
  that happens, paste the recipe text instead or ask Claude to save it. Chromium adds about 250 MB to the
  image and briefly uses 200–300 MB of RAM per blocked import. To skip it, set the Docker build arg
  `WITH_CHROMIUM=false` (Railway: add it as a service variable) or set `BROWSER_SCRAPING=off`.
- The app is meant for one person or household and runs as a single instance. Don't scale it to multiple replicas, because SQLite lives on one volume.

## Hosted edition

The hosted edition (`AUTH_MODE=hosted`) adds a second service, `crumb-auth` ([`auth/`](./auth)), which runs
[Better Auth](https://www.better-auth.com) for accounts, sessions and households. Railway services can't share a
volume, so it keeps its own SQLite database on its own volume. Only Crumb's server talks to it, over Railway's
private network; don't give it a public domain.

1. **Add the service:** New → GitHub repo → this repo, then Settings → **Root Directory** `auth`. Railway reads
   `auth/railway.json` and builds `auth/Dockerfile`. Name it `crumb-auth`.
2. **Attach a volume** to it (any mount path). It stores `auth.db` there.
3. **Set its variables:**

   | Variable               | Value                                                                    |
   | ---------------------- | ------------------------------------------------------------------------ |
   | `SITE_URL`             | Crumb's public URL, e.g. `https://crumb.example.com` (cookies and links) |
   | `BETTER_AUTH_SECRET`   | A long random secret (`openssl rand -base64 32`)                         |
   | `AUTH_INTERNAL_SECRET` | Another long random secret, also set on Crumb's service                  |
   | `EMAIL_FROM`           | Optional. The sender, e.g. `Crumb <hello@crumb.example.com>`             |
   | `SES_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | Optional. Amazon SES; with them, emails are sent and new accounts confirm their email |

4. **On Crumb's service**, set `AUTH_MODE=hosted`, `AUTH_SERVICE_URL=http://crumb-auth.railway.internal:3100`
   and the same `AUTH_INTERNAL_SECRET`. Crumb's own volume keeps each household's recipes
   (`households/{id}/recipes.db`) and the ids that link them to Better Auth (`accounts.db`).

Every household starts with an empty box. To give the recipes already in `recipes.db` to one account, set
`HOSTED_HOME_OWNER` to its email before it first signs in.
