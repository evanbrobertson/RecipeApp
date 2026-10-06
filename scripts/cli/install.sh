#!/usr/bin/env bash
# Installs the crumb CLI from its GitHub Release (tags `cli-vX.Y.Z`) and its agent skill.
#
#   curl -fsSL https://raw.githubusercontent.com/evanbrobertson/RecipeApp/master/scripts/cli/install.sh | bash
#   ... | bash -s -- --version 0.2.0 --no-skill
#
# Goes to /usr/local/bin when that is writable, else ~/.local/bin. Run it again to update.
# CRUMB_CLI_REPO overrides the repository (owner/name); GITHUB_TOKEN is used if set, for rate limits.
set -euo pipefail

repo=${CRUMB_CLI_REPO:-evanbrobertson/RecipeApp}
version=latest
skill=1

usage() {
  sed -n '2,8p' "${BASH_SOURCE[0]}" 2>/dev/null | sed 's/^# \{0,1\}//' || true
  echo "Options: --version X.Y.Z, --no-skill, -h"
}

while [[ $# -gt 0 ]]; do
  case $1 in
    -h | --help) usage; exit 0 ;;
    --version) version=${2:?--version needs a value}; shift 2 ;;
    --no-skill) skill=0; shift ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done

for tool in curl tar sha256sum; do
  command -v "$tool" >/dev/null || { echo "crumb install: $tool is required" >&2; exit 1; }
done

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  *) echo "crumb install: no build for $(uname -s) $(uname -m) yet (Linux x86_64 and arm64 only)" >&2; exit 1 ;;
esac

api() {
  local auth=()
  [[ -n ${GITHUB_TOKEN:-} ]] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
  curl -fsSL "${auth[@]}" -H "Accept: application/vnd.github+json" "https://api.github.com/repos/$repo/$1"
}

if [[ $version == latest ]]; then
  # The repository also releases the app (v*), the relay and the extension: take the newest cli-v*
  tag=$(api "releases?per_page=100" | grep -o '"tag_name": *"cli-v[0-9][^"]*"' | head -n1 | sed 's/.*"\(cli-v[^"]*\)"/\1/')
  [[ -n $tag ]] || { echo "crumb install: no CLI release found in $repo" >&2; exit 1; }
else
  tag=cli-v${version#cli-v}
  tag=${tag/cli-vv/cli-v}
fi
ver=${tag#cli-v}
name=crumb-cli-$ver-$target
base=https://github.com/$repo/releases/download/$tag

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
echo "Downloading crumb $ver ($target)"
curl -fsSL -o "$work/$name.tar.gz" "$base/$name.tar.gz"
curl -fsSL -o "$work/$name.tar.gz.sha256" "$base/$name.tar.gz.sha256"
(cd "$work" && sha256sum --check --status "$name.tar.gz.sha256") || {
  echo "crumb install: the download doesn't match its checksum; nothing was installed" >&2
  exit 1
}
tar -xzf "$work/$name.tar.gz" -C "$work"

if [[ -w /usr/local/bin ]]; then
  dest=/usr/local/bin
else
  dest=$HOME/.local/bin
  mkdir -p "$dest"
fi
install -m 0755 "$work/$name/crumb" "$dest/crumb"
echo "Installed $dest/crumb"
case ":$PATH:" in *":$dest:"*) ;; *) echo "Add $dest to your PATH to use it" ;; esac

if [[ $skill == 1 ]]; then
  "$dest/crumb" skill install
fi
echo
echo "Next: make a token in Crumb (More > Account > API tokens), then run: crumb login --server <your Crumb address> --token-stdin"
