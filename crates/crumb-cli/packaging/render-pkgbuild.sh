#!/usr/bin/env bash
# Fills PKGBUILD.in for the AUR crumb-cli-bin package.
#
#   render-pkgbuild.sh VERSION SHA256 OUT
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $(basename "$0") VERSION SHA256 OUT" >&2
  exit 2
fi

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
sed -e "s|@VERSION@|$1|g" -e "s|@SHA256@|$2|g" "$here/PKGBUILD.in" >"$3"
