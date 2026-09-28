#!/usr/bin/env bash
# Screenshots one screen in the web app and in the desktop app, for parity checks.
#
#   desktop/linux/tools/compare.sh <route> [out-dir] [WIDTHxHEIGHT]
#
# <route> is a desktop route: home, recipes, recipe:9, cook:9, prep:9, edit:9, new, shelf,
# cookbook:2, suggestions, add, import, more, account, connect, connections, login.
# Writes <out-dir>/<route>-web.png and <route>-desktop.png. Needs a running Crumb server
# (CRUMB_SERVER, default http://127.0.0.1:3100), chromium, and a built crumb-desktop.
set -euo pipefail

route="${1:?usage: compare.sh <route> [out-dir] [WxH]}"
out="${2:-.}"
size="${3:-1280x1000}"
server="${CRUMB_SERVER:-http://127.0.0.1:3100}"
root="$(cd "$(dirname "$0")/../../.." && pwd)"
bin="${CRUMB_DESKTOP_BIN:-${CARGO_TARGET_DIR:-$root/target}/debug/crumb-desktop}"

name="${route%%:*}"
id="${route#*:}"
[ "$id" = "$route" ] && id=""
case "$name" in
  home) path="/" ;;
  recipes) path="/recipes" ;;
  recipe) path="/recipes/$id" ;;
  cook | prep | edit) path="/recipes/$id/$name" ;;
  new) path="/recipes/new" ;;
  shelf) path="/cookbooks" ;;
  cookbook) path="/cookbooks/$id" ;;
  account | connections) path="/more/$name" ;;
  *) path="/$name" ;;
esac

mkdir -p "$out"
file="${route//:/-}"
chromium --headless=new --no-sandbox --hide-scrollbars --virtual-time-budget=6000 \
  --window-size="${size/x/,}" --screenshot="$out/$file-web.png" "$server$path" >/dev/null 2>&1
CRUMB_SERVER="$server" QT_QPA_PLATFORM=offscreen timeout 60 "$bin" \
  --route "$route" --shot "$out/$file-desktop.png" --size "$size"
echo "$out/$file-web.png"
echo "$out/$file-desktop.png"
