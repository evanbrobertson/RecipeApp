#!/usr/bin/env bash
# Formats the iOS Swift sources with swift-format (Xcode's toolchain) and ios/.swift-format.
#   scripts/format.sh          rewrite files in place
#   scripts/format.sh --lint   check the lint rules only, failing on any finding
set -euo pipefail
cd "$(dirname "$0")/.."

paths=(CrumbKit/Package.swift CrumbKit/Sources/CrumbKit CrumbKit/Tests Crumb CrumbShare CrumbTests CrumbUITests)
if [[ ${1:-} == --lint ]]; then
  xcrun swift-format lint --strict --recursive --parallel --configuration .swift-format "${paths[@]}"
else
  xcrun swift-format format --in-place --recursive --parallel --configuration .swift-format "${paths[@]}"
fi
