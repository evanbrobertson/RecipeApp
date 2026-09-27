"""Adds U+2150-215F to Nunito Sans as composites laid out exactly as its own `frac` feature
sets "N/D" (the way ¼ ½ ¾ are built), with gvar deltas solved so every location matches."""
import itertools, random, sys
import numpy as np
import uharfbuzz as hb
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent
from fontTools.ttLib.tables.TupleVariation import TupleVariation
from fontTools.varLib.models import supportScalar

SRC, OUT = sys.argv[1], sys.argv[2]
NAMES = "zero one two three four five six seven eight nine".split()
FRACTIONS = {
    0x2150: ("1", "7", "oneseventh"), 0x2151: ("1", "9", "oneninth"),
    0x2152: ("1", "10", "onetenth"), 0x2153: ("1", "3", "onethird"),
    0x2154: ("2", "3", "twothirds"), 0x2155: ("1", "5", "onefifth"),
    0x2156: ("2", "5", "twofifths"), 0x2157: ("3", "5", "threefifths"),
    0x2158: ("4", "5", "fourfifths"), 0x2159: ("1", "6", "onesixth"),
    0x215A: ("5", "6", "fivesixths"), 0x215B: ("1", "8", "oneeighth"),
    0x215C: ("3", "8", "threeeighths"), 0x215D: ("5", "8", "fiveeighths"),
    0x215E: ("7", "8", "seveneighths"), 0x215F: ("1", "", "fraction.one"),
}

f = TTFont(SRC)
axes = [a.axisTag for a in f["fvar"].axes]
face = hb.Face(hb.Blob.from_file_path(SRC))
font = hb.Font(face)


def shape(num, den, coords):
    """(components [(glyph, x, y)], advance) at normalized `coords`, as `frac` sets it."""
    font.set_var_coords_normalized([coords.get(a, 0.0) for a in axes])
    buf = hb.Buffer()
    buf.add_str(f"{num}⁄{den or '2'}")
    buf.guess_segment_properties()
    hb.shape(font, buf, {"frac": True})
    x, out = 0, []
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        out.append((font.glyph_to_string(info.codepoint), x + pos.x_offset, pos.y_offset, pos.x_advance))
        x += pos.x_advance
    if not den:  # ⅟: the numerator and the slash only
        out = out[:2]
        x = out[1][1] + out[1][3]
    comps = []
    for name, cx, cy, _ in out:
        # Numerators as the template does it: the denominator figure raised
        if name.endswith(".numr"):
            base = f["glyf"][name].components[0]
            comps.append((base.glyphName, cx + base.x, cy + base.y))
        else:
            comps.append((name, cx, cy))
    return comps, x


# The deltas' regions: the ones ½ uses, which cover every axis its layout varies on
regions = [tv.axes for tv in f["gvar"].variations["onehalf"]]
peaks = [{a: p for a, (lo, p, hi) in r.items()} for r in regions]
S = np.array([[supportScalar(pk, r) for r in regions] for pk in peaks])


def vector(comps, adv):
    return np.array([[c[1], c[2]] for c in comps] + [[0, 0], [adv, 0], [0, 0], [0, 0]], float)


glyf, hmtx, gvar = f["glyf"], f["hmtx"], f["gvar"]
order = f.getGlyphOrder()
cmap_tables = [t for t in f["cmap"].tables if t.isUnicode()]
worst = 0
for cp, (num, den, name) in FRACTIONS.items():
    base_comps, base_adv = shape(num, den, {})
    base = vector(base_comps, base_adv)
    diffs = []
    for pk in peaks:
        comps, adv = shape(num, den, pk)
        assert [c[0] for c in comps] == [c[0] for c in base_comps], (name, pk)
        diffs.append(vector(comps, adv) - base)
    D = np.linalg.solve(S, np.stack(diffs).reshape(len(peaks), -1)).reshape(len(peaks), -1, 2)
    g = Glyph()
    g.numberOfContours = -1
    g.components = []
    for cname, cx, cy in base_comps:
        c = GlyphComponent()
        # ROUND_XY_TO_GRID, as on ¼ ½ ¾
        c.glyphName, c.x, c.y, c.flags = cname, int(cx), int(cy), 0x4
        g.components.append(c)
    order.append(name)
    glyf.glyphs[name] = g
    glyf.glyphOrder = order
    g.recalcBounds(glyf)
    hmtx[name] = (int(base_adv), g.xMin)
    gvar.variations[name] = [
        TupleVariation(dict(r), [tuple(int(round(v)) for v in pt) for pt in d])
        for r, d in zip(regions, D) if np.abs(d).max() >= 0.5
    ]
    for t in cmap_tables:
        t.cmap[cp] = name
    # Check between the regions' peaks: the model must match the shaper everywhere
    rng = random.Random(cp)
    for _ in range(200):
        loc = {a: rng.uniform(-1, 1) for a in axes}
        comps, adv = shape(num, den, loc)
        want = vector(comps, adv)
        got = base + sum(supportScalar(loc, r) * d for r, d in zip(regions, D))
        worst = max(worst, np.abs(want - got).max())
f.setGlyphOrder(order)
glyf.recalcBounds = True
f["maxp"].numGlyphs = len(order)
f.save(OUT)
print(f"added {len(FRACTIONS)} fractions; worst mismatch vs frac shaping: {worst:.2f} units")
