#!/usr/bin/env bash
# Rebuilds web/src/assets/fonts/kalam.woff2: Kalam Regular from Google Fonts, cut to the Latin
# subset (so ñ, é, ° and ½ are drawn by Kalam, not a fallback). Needs uv. See docs/FONTS.md.
set -euo pipefail

SRC_COMMIT=a0e524c05906bece66cd5bcdc9216ff1d044fcbf
SRC_SHA256=57cecb63d4608019371954274ae1d8c397764debd5b19d4a33c1efa4dc923c0b
# Google Fonts' "latin" subset. Kalam draws ¼ ½ ¾ only; the other vulgar fractions come from
# Nunito Sans through a unicode-range face (see Layout.astro)
UNICODES="U+0000,U+000D,U+0020-007E,U+00A0-00FF,U+0102,U+0131,U+0152-0153,U+02BB-02BC,\
U+02C6,U+02DA,U+02DC,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+2009,U+200B,U+2013-2014,\
U+2018-201A,U+201C-201E,U+2022,U+2026,U+2032-2033,U+2039-203A,U+2044,U+20AC,U+2122,U+2212,\
U+2215"

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/../../web/src/assets/fonts/kalam.woff2"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/src.ttf" \
  "https://raw.githubusercontent.com/google/fonts/$SRC_COMMIT/ofl/kalam/Kalam-Regular.ttf"
echo "$SRC_SHA256  $work/src.ttf" | sha256sum -c --quiet

# Same bytes on every run: set iteration order and the head timestamp are pinned
export PYTHONHASHSEED=0 SOURCE_DATE_EPOCH=1735689600
uvx --from 'fonttools[woff]==4.*' pyftsubset "$work/src.ttf" --unicodes="$UNICODES" \
  --layout-features="liga" --flavor=woff2 --output-file="$out"
ls -l "$out"
