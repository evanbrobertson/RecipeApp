#!/usr/bin/env bash
# Fill PKGBUILD.in for the AUR crumb-desktop-bin package.
#
#   render-pkgbuild.sh VERSION SHA256 OUT
#
# VERSION is the release version (`3.1.0`, or `3.1.0-beta.1`). Arch forbids hyphens in
# pkgver, so the package version uses underscores while the tarball URL keeps the real name.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $(basename "$0") VERSION SHA256 OUT" >&2
  exit 2
fi

version=$1
sha256=$2
out=$3
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
pkgver=${version//-/_}

sed -e "s|@PKGVER@|$pkgver|g" \
  -e "s|@VERSION@|$version|g" \
  -e "s|@SHA256@|$sha256|g" \
  "$here/PKGBUILD.in" > "$out"
