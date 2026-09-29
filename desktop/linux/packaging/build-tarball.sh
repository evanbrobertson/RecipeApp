#!/usr/bin/env bash
# Assemble the release tarball for the Linux desktop app. It builds nothing: run
# `cargo build --release -p crumb-desktop-linux --locked` first. The tarball unpacks to a
# `crumb-desktop/` directory holding the binary, desktop entry, icon, install.sh, README and
# the license.
#
#   build-tarball.sh VERSION OUTDIR
#
# Writes OUTDIR/crumb-desktop-linux-x86_64-VERSION.tar.gz and its .sha256 beside it.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $(basename "$0") VERSION OUTDIR" >&2
  exit 2
fi

version=$1
outdir=$2
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
name="crumb-desktop-linux-x86_64-${version}.tar.gz"
binary="$repo/target/release/crumb-desktop"

if [[ ! -x $binary ]]; then
  echo "error: $binary not found; run 'cargo build --release -p crumb-desktop-linux --locked' first" >&2
  exit 1
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
pkg="$stage/crumb-desktop"
mkdir -p "$pkg"

install -m755 "$binary" "$pkg/crumb-desktop"
install -m644 "$repo/desktop/linux/crumb-desktop.desktop" "$pkg/crumb-desktop.desktop"
install -m644 "$repo/web/public/icon-512.png" "$pkg/crumb-desktop.png"
install -m755 "$repo/desktop/linux/packaging/install.sh" "$pkg/install.sh"
install -m644 "$repo/desktop/linux/packaging/README.txt" "$pkg/README.txt"
# The repo is MIT; the license ships with the binary when present
if [[ -f "$repo/LICENSE" ]]; then
  install -m644 "$repo/LICENSE" "$pkg/LICENSE"
fi

mkdir -p "$outdir"
tar -czf "$outdir/$name" -C "$stage" crumb-desktop
(cd "$outdir" && sha256sum "$name" > "$name.sha256")
echo "Wrote $outdir/$name"
echo "Wrote $outdir/$name.sha256"
