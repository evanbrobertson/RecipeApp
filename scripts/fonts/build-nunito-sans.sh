#!/usr/bin/env bash
# Rebuilds web/src/assets/fonts/nunito-sans.woff2: Nunito Sans from Google Fonts' variable
# source, with the vulgar fractions U+2150-215F added (see nunito_fractions.py), opsz, wdth
# and YTLC pinned as Google's own web font does, weight kept variable (200-1000), and cut to
# the Latin subset. Needs uv. See docs/FONTS.md.
set -euo pipefail

SRC_COMMIT=a0e524c05906bece66cd5bcdc9216ff1d044fcbf
SRC_SHA256=f934d7142fb4784bf828da485b7dcbd90c0c80d514e9d49a5da0ed3a1ae2491d
# Google Fonts' "latin" subset, plus the vulgar fractions the importer and scaler write
UNICODES="U+0000,U+000D,U+0020-007E,U+00A0-00FF,U+0102,U+0131,U+0152-0153,U+02BB-02BC,\
U+02C6,U+02DA,U+02DC,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+2009,U+200B,U+2013-2014,\
U+2018-201A,U+201C-201E,U+2022,U+2026,U+2032-2033,U+2039-203A,U+2044,U+20AC,U+2122,U+2212,\
U+2215,U+2150-215F"
FEATURES="calt,ccmp,dnom,frac,liga,locl,numr,kern,mark,mkmk"

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/../../web/src/assets/fonts/nunito-sans.woff2"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/src.ttf" \
  "https://github.com/google/fonts/raw/$SRC_COMMIT/ofl/nunitosans/NunitoSans%5BYTLC,opsz,wdth,wght%5D.ttf"
echo "$SRC_SHA256  $work/src.ttf" | sha256sum -c --quiet

# Same bytes on every run: set iteration order and the head timestamp are pinned
export PYTHONHASHSEED=0 SOURCE_DATE_EPOCH=1735689600
fonttools=(uvx --from 'fonttools[woff]==4.*' --with uharfbuzz --with numpy)
"${fonttools[@]}" python "$here/nunito_fractions.py" "$work/src.ttf" "$work/fractions.ttf"
"${fonttools[@]}" fonttools varLib.instancer "$work/fractions.ttf" \
  opsz=12 wdth=100 YTLC=500 wght=200:1000 -o "$work/instance.ttf"
"${fonttools[@]}" pyftsubset "$work/instance.ttf" --unicodes="$UNICODES" \
  --layout-features="$FEATURES" --flavor=woff2 --output-file="$out"
ls -l "$out"
