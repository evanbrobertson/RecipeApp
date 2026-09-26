"""Read Omarchy's active palette and adapt Crumb's existing colour tokens."""

import os
from pathlib import Path
import re
import tomllib


HEX = re.compile(r"#[0-9a-fA-F]{6}\Z")


def theme_file(home: Path | None = None) -> Path | None:
    home = home or Path.home()
    # Quattro moved the live theme to XDG state; support older installations too.
    state = Path(os.environ.get("XDG_STATE_HOME", home / ".local/state"))
    for path in (
        state / "omarchy/current/theme/colors.toml",
        home / ".config/omarchy/current/theme/colors.toml",
    ):
        if path.is_file():
            return path
    return None


def _colour(palette: dict, *keys: str, default: str) -> str:
    for key in keys:
        value = palette.get(key)
        if isinstance(value, str) and HEX.fullmatch(value):
            return value.lower()
    return default


def _luminance(colour: str) -> float:
    values = [int(colour[i : i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in values]
    return sum(a * b for a, b in zip(linear, (0.2126, 0.7152, 0.0722)))


def _on(colour: str) -> str:
    return "#171717" if _luminance(colour) > 0.179 else "#ffffff"


def _contrast(a: str, b: str) -> float:
    hi, lo = sorted((_luminance(a), _luminance(b)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


def _mix(a: str, b: str, weight: float) -> str:
    """`weight` of `a` blended with the rest of `b`, as six-digit hex."""
    channels = (
        round(int(a[i : i + 2], 16) * weight + int(b[i : i + 2], 16) * (1 - weight))
        for i in (1, 3, 5)
    )
    return "#" + "".join(f"{c:02x}" for c in channels)


def _readable(bg: str, *candidates: str) -> str:
    """The first candidate with 4.5:1 contrast on `bg`, else the one with the most."""
    for colour in candidates:
        if _contrast(colour, bg) >= 4.5:
            return colour
    return max(candidates, key=lambda c: _contrast(c, bg))


def read_theme(path: Path | None) -> tuple[str, str] | None:
    if path is None:
        return None
    try:
        with path.open("rb") as file:
            palette = tomllib.load(file)
    except (OSError, ValueError):
        return None

    mode = palette.get("mode")
    bg = _colour(palette, "background", "bg", default="#141c17")
    dark = mode == "dark" or (mode != "light" and _luminance(bg) < 0.179)
    fg = _colour(palette, "foreground", "fg", default="#efe9da" if dark else "#1c2b22")
    muted = _colour(palette, "muted", "dark_foreground", "dark_fg", default=fg)
    surface = _colour(palette, "lighter_background", "lighter_bg", default=bg)
    elevated = _colour(palette, "dark_background", "dark_bg", default=surface)
    accent = _colour(palette, "accent", default="#3a7859")
    selection = _colour(palette, "selection", default=accent)
    # Butter is the main action's fill and the active nav item's text, so it must read on the nav.
    butter = _readable(elevated, selection, accent, fg)
    red = _colour(palette, "red", default="#a8432c")
    bright_red = _colour(palette, "bright_red", default=red)
    # Omarchy's `muted` is a border shade, too dim for text; blend the foreground instead.
    dim = _readable(bg, _mix(fg, bg, 0.7), _mix(fg, bg, 0.8), fg)
    # Constrain values to six-digit hex above: the palette is inserted into CSS.
    tokens = {
        "bg": bg,
        "paper": surface,
        "tint": elevated,
        "bg-muted": elevated,
        "bg-elevated": surface,
        "bg-accented": elevated,
        "border": muted,
        "border-muted": elevated,
        "border-accented": muted,
        "text": fg,
        "text-muted": dim,
        "text-dimmed": dim,
        "primary": _readable(surface, accent, fg),
        "primary-hover": _readable(surface, accent, fg),
        "tile": accent,
        "tile-grout": bg,
        "on-tile": _on(accent),
        "butter": butter,
        "butter-hover": butter,
        "on-butter": _on(butter),
        "nav": elevated,
        "nav-inactive": dim,
        "error": _readable(surface, red, bright_red),
        "success": accent,
        "shelf-wood": accent,
        "shelf-wood-dark": bg,
    }
    css = ":root, :root.dark {" + "".join(f"--{k}:{v}!important;" for k, v in tokens.items()) + "}"
    # The hero tile sets its own dark base on the element, which :root cannot override.
    css += f".tile-surface {{--tile-base:{accent}!important;}}"
    return ("dark" if dark else "light", css)
