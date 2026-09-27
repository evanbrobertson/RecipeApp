# Fonts

Crumb self-hosts three woff2 files in `web/src/assets/fonts/`, each with a metric-matched fallback
in `web/src/styles/app.css`.

| File | Font | Source |
| --- | --- | --- |
| `nunito-sans.woff2` | Nunito Sans, variable weight 200–1000 | Built by `scripts/fonts/build-nunito-sans.sh` |
| `dm-serif-display.woff2` | DM Serif Display | Google Fonts, Latin subset |
| `caveat.woff2` | Caveat | Google Fonts, Latin subset |

## Nunito Sans

Imports write quantities as Unicode fractions and scaling can produce any of them, so the body font
carries every vulgar fraction (U+2150–215F) as well as ¼ ½ ¾. Nunito Sans only draws ¼ ½ ¾ itself.
The rest are added as composites built the same way: the numerator figure raised, the fraction slash,
then the denominator. They're laid out exactly as the font's own `frac` feature sets "1⁄3" at every
weight (`scripts/fonts/nunito_fractions.py`).

To rebuild (needs `uv`):

```bash
scripts/fonts/build-nunito-sans.sh
```

The script downloads the variable source from a pinned `google/fonts` commit (checksum verified) and
adds the fractions. It then pins optical size 12, width 100 and YTLC 500 as Google's web font does,
keeping weight variable, and cuts the file to Google's Latin subset plus the fractions. The output is
byte-for-byte reproducible. Keep U+2150–215F in `UNICODES` when changing the subset.

Every other glyph matches Google's own file: advances, kerning and outlines are identical at every
weight, so the fallback's `size-adjust` and overrides still line up. If a rebuild changes vertical
metrics or widths, re-check the fallback in `app.css`.
