# The Crumb relay

Some recipe sites' bot protection (Cloudflare and the like) turns away a **datacenter IP address** whatever browser fingerprint arrives from it. Crumb's server runs in one, so a few sites block it even though a home connection reads them fine.

`crumb-relay` is a small program you run on a Raspberry Pi, a home-lab box or a cheap droplet on another network. When the server has been refused twice (Firefox's fingerprint, then Safari's), it asks your relays to fetch the same page with the same two fingerprints. The relay sends back the page; the server reads the recipe from it as always.

```
Crumb server ── Firefox ─► site ✗ blocked
             ── Safari  ─► site ✗ blocked
             ── POST /fetch ─► relay (Pi, over Tailscale) ─► site ✓
             ── headless Chromium (last, if installed)
```

- Relays run **only after the server's own fetches were both blocked**, so they see a small share of imports. A site that isn't blocking the server never reaches them.
- The relay is reached over **Tailscale**: it opens no public port.
- It keeps nothing. It logs the site's host name and how the fetch went, never the full address, a query string or a page.

## Set up a relay on a Raspberry Pi

You need a Pi 3 or newer (or any small Linux box) with Raspberry Pi OS Bookworm, Debian 12, Ubuntu 22.04 or newer, on 64-bit or 32-bit ARMv7 or x86-64. Older systems can use the Docker image below.

### 1. Tailscale

```bash
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up            # opens a login link; add --hostname=pi1 to name it
tailscale ip -4              # e.g. 100.101.102.103
```

The relay's address is that `100.x` address (or its MagicDNS name such as `pi1.your-tailnet.ts.net`) and port `8787`. In Tailscale's access controls, allow only the Crumb server's node (or tag) to reach port 8787 on the relays.

### 2. A token

The token is the relay's password. Make one long random secret and use the same value on every relay and on the server:

```bash
openssl rand -hex 32
```

The relay refuses to start with a token under 24 characters.

### 3. The program

Download the build for your machine from the [releases](https://github.com/evanbrobertson/RecipeApp/releases) tagged `relay-v…` (`aarch64` for a 64-bit Pi OS, `armv7` for a 32-bit one, `x86_64` for a PC or droplet), check it against its `.sha256` file and install it:

```bash
tar -xzf crumb-relay-0.1.0-aarch64-unknown-linux-gnu.tar.gz
sudo install -m 755 crumb-relay-0.1.0-aarch64-unknown-linux-gnu/crumb-relay /usr/local/bin/
```

`uname -m` says which you need: `aarch64`, `armv7l` or `x86_64`. (A Pi Zero or Pi 1 is ARMv6 and isn't built; use a Pi 2 or newer.)

### 4. Run it under systemd

```bash
sudo tee /etc/crumb-relay.env >/dev/null <<EOF
RELAY_TOKEN=paste-the-token-here
RELAY_LISTEN=100.101.102.103:8787
RELAY_NAME=pi1
EOF
sudo chmod 600 /etc/crumb-relay.env
sudo cp crumb-relay-*/crumb-relay.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now crumb-relay
```

`RELAY_LISTEN` should be the Tailscale address, so nothing else on the network can reach the port. The default (`0.0.0.0:8787`) listens on every interface and the relay says so in its log. The unit file is `crates/crumb-relay/crumb-relay.service`:

```ini
[Unit]
Description=Crumb relay
Wants=network-online.target tailscaled.service
After=network-online.target tailscaled.service

[Service]
EnvironmentFile=/etc/crumb-relay.env
ExecStart=/usr/local/bin/crumb-relay
Restart=on-failure
RestartSec=5
DynamicUser=yes
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
MemoryMax=256M

[Install]
WantedBy=multi-user.target
```

Check it, from the Pi and then from another node on the tailnet:

```bash
curl http://100.101.102.103:8787/health        # {"name":"pi1","version":"0.1.0"}
journalctl -u crumb-relay -f
```

### Or with Docker

```bash
docker run -d --name crumb-relay --restart unless-stopped \
  -e RELAY_TOKEN=paste-the-token-here -e RELAY_NAME=pi1 \
  -p 100.101.102.103:8787:8787 \
  ghcr.io/evanbrobertson/recipeapp-relay:latest
```

Publishing the port on the Tailscale address (`-p 100.101.102.103:8787:8787`) is what keeps it private; the container itself listens on all its interfaces. The image is `linux/amd64`, `linux/arm64` and `linux/arm/v7`. To build it from source on the machine instead: `docker build -f crates/crumb-relay/Dockerfile -t crumb-relay .` from the repository root (the first build compiles BoringSSL and takes a while on a Pi).

## Tell the server about it

On the Crumb server (Railway variables):

| Variable | Value |
| --- | --- |
| `SCRAPE_RELAYS` | Comma-separated relay addresses, e.g. `http://pi1.your-tailnet.ts.net:8787,http://100.101.102.103:8787` |
| `SCRAPE_RELAY_TOKEN` | The token |
| `SCRAPE_RELAY_PROXY` | Only if the server reaches Tailscale through a proxy (below) |

The server tries the relays in turn, starting from a different one each time, each with the Firefox and then the Safari fingerprint, and stops at the first that gives a recipe. All relays together get 30 seconds. The Internet Archive's copy of the page is asked as well only if no relay has answered within 2 seconds (or at once when no relays are set up), because an archived copy can be older than what a relay sees. A relay that can't be reached, answers 401 or otherwise breaks is left alone for five minutes; one that answers "busy" (429) is only skipped for that import. The server logs `[relay] <site>: <relay name> worked` when one helps.

## The server reaching your tailnet

The server has to be on the tailnet to reach `100.x` addresses. Railway containers can't run a VPN, but Tailscale can run **in userspace** and offer a SOCKS5 proxy. What follows is the plausible route; it has not been tried on Railway.

**Recommended: a Tailscale service beside the server, no change to Crumb's image.** Add a second Railway service in the same project from the image `tailscale/tailscale:stable` with:

| Variable | Value |
| --- | --- |
| `TS_AUTHKEY` | An [auth key](https://login.tailscale.com/admin/settings/keys): reusable, ephemeral, tagged (e.g. `tag:crumb-server`) |
| `TS_HOSTNAME` | `crumb-server` |
| `TS_USERSPACE` | `true` |
| `TS_SOCKS5_SERVER` | `:1055` |

Then, on the Crumb service, `SCRAPE_RELAY_PROXY=socks5h://<that service's name>.railway.internal:1055`. (`socks5h` lets Tailscale resolve MagicDNS names such as `pi1.your-tailnet.ts.net`; with plain `100.x` addresses either works.) Only relay traffic uses the proxy; every other request the server makes goes out directly. Railway's private network is only reachable from your own project's services, and the proxy has no password, so allow that tag only what it needs in Tailscale's access controls (port 8787 on the relays).

**Follow-up if that proves awkward:** run `tailscaled` in the Crumb image itself (install it, start it with `--tun=userspace-networking --socks5-server=localhost:1055 --state=mem:` before the server, from an entrypoint script, with `TS_AUTHKEY` as a secret) and point `SCRAPE_RELAY_PROXY` at `socks5h://localhost:1055`. That changes the Dockerfile and the process model, so it's left out of the relay's first version.

Anywhere the server can already route to (a self-hosted server on the tailnet, say) needs no proxy at all.

## Settings

| Variable | Default | |
| --- | --- | --- |
| `RELAY_TOKEN` | required | At least 24 characters. Sent as `Authorization: Bearer` |
| `RELAY_LISTEN` | `0.0.0.0:8787` | Address and port. Use the Tailscale address (`tailscale ip -4`) |
| `RELAY_NAME` | the host name | How the relay is called in the server's log and in `/health` |
| `RELAY_HOST_INTERVAL_SECS` | `5` | Least time between two fetches from the same site (0 turns it off) |
| `RELAY_CONCURRENCY` | `2` | Fetches running at once; more get a 429 |
| `RELAY_PER_MINUTE` | `20` | Fetches started in any minute; more get a 429 |
| `RUST_LOG` | `info` | Log level |

A setting that doesn't parse stops the relay starting (exit code 2) instead of being ignored.

### Don't burn a residential IP

The point of a relay is that its address is *not* blocked. A home IP that fetches a site in bursts gets blocked like the datacenter's did, and that's your home connection. The limits are stingy on purpose; loosen them only if you have a reason.

- Leave the per-site interval at 5 seconds or more. Recipe imports are a person pasting a link, so one fetch per site per few seconds is plenty.
- Two relays on two networks share the load better than one relay with higher limits.
- Don't point other tools at it. The relay isn't a general proxy: it fetches only public web pages, only with the two browser fingerprints, and refuses the rest.
- It sits idle most of the time: the server asks only after being blocked twice.

## What the relay refuses

It sits next to a router and a tailnet, so it fetches **only public web addresses**:

- only `http` and `https`, on port 80 or 443, without a user name in the link;
- not an address in a private, loopback, link-local (including cloud metadata, `169.254.169.254`), carrier-grade NAT (`100.64.0.0/10`, which **is Tailscale's range**), unique-local, multicast, documentation or reserved range, IPv6 or IPv4-mapped or -embedded (NAT64, 6to4) included;
- not a name that resolves to any such address (one bad answer among good ones is enough), so `localhost` and MagicDNS names are refused;
- every redirect (at most 10) goes through the same checks.

The checks use the same address the connection is made to: the client's own DNS resolver is the checked one, so there's no second lookup for a hostile DNS server to answer differently (DNS rebinding). Proxy settings from the environment are ignored for the same reason. Refusals answer 400 and cost no rate-limit slot.

## For developers

`POST /fetch` with `{"url": "...", "profile": "firefox" | "safari"}` and the bearer token answers `200 {"status", "body", "relay"}` (the site's status; the body is the page for a 2xx and empty otherwise), or `{"error"}` with 400 (refused link or bad request), 401 (token), 429 (limits, with `Retry-After`) or 502 (site unreachable). `GET /health` answers `{"name", "version"}` without a token. The types are in `crates/crumb-fetch/src/wire.rs`, shared by the server (`src/relay.rs`) and the relay.

`crumb-fetch` holds the browser-profile client both use, so the fingerprints can't drift apart. Its address checks (`crumb_fetch::guard`) are used by the relay only; the server's own fetches of pasted links don't use them.

Releases are automatic: a push to `master` that changes `crates/crumb-relay/` or `crates/crumb-fetch/` works out the next version from the conventional commits touching them (`feat:` a minor, anything else a patch), and `.github/workflows/relay.yml` tags `relay-vX.Y.Z`, builds Linux binaries for x86-64, ARM64 and ARMv7 (on Ubuntu 22.04, so they run on Debian 12 and newer), attaches them to a GitHub Release and pushes the multi-arch image to `ghcr.io/evanbrobertson/recipeapp-relay`. The version in `crates/crumb-relay/Cargo.toml` is only the starting point; each build sets the release's version, so `/health` reports it. Pushing a `relay-vX.Y.Z` tag by hand releases that version; run from the Actions tab the workflow only builds. Pull requests get an ARM64 build and an image build. See [RELEASING.md](RELEASING.md#relay).
