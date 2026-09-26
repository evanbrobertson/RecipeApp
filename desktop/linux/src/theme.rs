//! Omarchy theme mapping to Crumb's colour roles. Pure Rust, no Qt: the same code runs in
//! unit tests, on the watcher thread and in `Palette`.
//!
//! An Omarchy theme is a `colors.toml` in `current/theme/`. The mapping below was tuned by
//! hand against real themes, so the rules are kept exactly as written — they deliberately
//! differ from a naive "use the tokens directly" approach (see `dim` and `butter`).
//!
//! Without Omarchy, Crumb falls back to its own "Green Tile" palettes, mirroring the light
//! `:root` and `.dark` tokens in `web/src/styles/app.css`.

use std::path::{Path, PathBuf};

/// The luminance below which a background counts as dark when `mode` is absent.
const DARK_LUMINANCE: f64 = 0.179;
/// The WCAG AA contrast a text colour should reach.
const MIN_CONTRAST: f64 = 4.5;

/// Crumb's default background, used when a theme has no `background`/`bg`.
const DEFAULT_BG: Rgb = Rgb::new(0x14, 0x1c, 0x17);
/// Crumb's default text, used when a theme has no `foreground`/`fg`.
const DEFAULT_FG_DARK: Rgb = Rgb::new(0xef, 0xe9, 0xda);
const DEFAULT_FG_LIGHT: Rgb = Rgb::new(0x1c, 0x2b, 0x22);
/// Crumb's tile green, used when a theme has no `accent`.
const DEFAULT_ACCENT: Rgb = Rgb::new(0x3a, 0x78, 0x59);
/// Crumb's error red, used when a theme has no `red`.
const DEFAULT_RED: Rgb = Rgb::new(0xa8, 0x43, 0x2c);

/// An 8-bit sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// A colour from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `#rrggbb` exactly (six hex digits, case-insensitive). Anything else —
    /// including CSS injection attempts — is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let bytes = text.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' {
            return None;
        }
        let mut value = [0u8; 3];
        for (index, channel) in value.iter_mut().enumerate() {
            let pair = &text[1 + index * 2..1 + index * 2 + 2];
            *channel = u8::from_str_radix(pair, 16).ok()?;
        }
        Some(Self::new(value[0], value[1], value[2]))
    }

    /// The colour as a lower-case `#rrggbb` string.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// WCAG relative luminance (sRGB linearised, then the 0.2126/0.7152/0.0722 mix).
pub fn luminance(color: Rgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = value as f64 / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// The WCAG contrast ratio between two colours (1.0 to 21.0).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (hi, lo) = {
        let (a, b) = (luminance(a), luminance(b));
        if a >= b { (a, b) } else { (b, a) }
    };
    (hi + 0.05) / (lo + 0.05)
}

/// Channel-wise blend: `w * a + (1 - w) * b`, rounded.
pub fn mix(a: Rgb, b: Rgb, w: f64) -> Rgb {
    fn channel(a: u8, b: u8, w: f64) -> u8 {
        (w * a as f64 + (1.0 - w) * b as f64)
            .round()
            .clamp(0.0, 255.0) as u8
    }
    Rgb::new(
        channel(a.r, b.r, w),
        channel(a.g, b.g, w),
        channel(a.b, b.b, w),
    )
}

/// The first candidate reaching [`MIN_CONTRAST`] against `bg`, else the highest-contrast one.
pub fn readable(bg: Rgb, candidates: &[Rgb]) -> Rgb {
    let mut best = candidates[0];
    let mut best_ratio = contrast(bg, best);
    for &candidate in candidates {
        let ratio = contrast(bg, candidate);
        if ratio >= MIN_CONTRAST {
            return candidate;
        }
        if ratio > best_ratio {
            best = candidate;
            best_ratio = ratio;
        }
    }
    best
}

/// The foreground (near-black or white) with the best chance of sitting on `color`.
pub fn on(color: Rgb) -> Rgb {
    if luminance(color) > DARK_LUMINANCE {
        Rgb::new(0x17, 0x17, 0x17)
    } else {
        Rgb::new(0xff, 0xff, 0xff)
    }
}

/// The full set of colour roles the UI binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: Rgb,
    pub paper: Rgb,
    pub tint: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub line: Rgb,
    pub primary: Rgb,
    pub tile: Rgb,
    pub on_tile: Rgb,
    pub butter: Rgb,
    pub on_butter: Rgb,
    pub nav: Rgb,
    pub error: Rgb,
    pub dark: bool,
}

impl Theme {
    /// The text-role pairs that should reach [`MIN_CONTRAST`]. Fills and status colours
    /// (`error`, `line`) are not text and are deliberately excluded.
    pub fn text_roles(&self) -> [(&'static str, Rgb, Rgb); 6] {
        [
            ("text on bg", self.text, self.bg),
            ("text on paper", self.text, self.paper),
            ("textMuted on bg", self.text_muted, self.bg),
            ("primary on paper", self.primary, self.paper),
            ("onTile on tile", self.on_tile, self.tile),
            ("onButter on butter", self.on_butter, self.butter),
        ]
    }

    /// Crumb's built-in light palette, mirroring the light `:root` tokens.
    pub const fn crumb_light() -> Self {
        Self {
            bg: Rgb::new(0xf5, 0xf1, 0xe6),
            paper: Rgb::new(0xff, 0xfd, 0xf8),
            tint: Rgb::new(0xe4, 0xec, 0xe3),
            text: Rgb::new(0x1c, 0x2b, 0x22),
            text_muted: Rgb::new(0x56, 0x63, 0x5a),
            line: Rgb::new(0xe3, 0xdf, 0xd0),
            primary: Rgb::new(0x2f, 0x6b, 0x4f),
            tile: Rgb::new(0x2f, 0x6b, 0x4f),
            on_tile: Rgb::new(0xff, 0xfd, 0xf8),
            butter: Rgb::new(0xf3, 0xda, 0x8b),
            on_butter: Rgb::new(0x1c, 0x2b, 0x22),
            nav: Rgb::new(0x1c, 0x2b, 0x22),
            error: Rgb::new(0xa8, 0x43, 0x2c),
            dark: false,
        }
    }

    /// Crumb's built-in dark palette, mirroring the `.dark` tokens.
    pub const fn crumb_dark() -> Self {
        Self {
            bg: Rgb::new(0x14, 0x1c, 0x17),
            paper: Rgb::new(0x1d, 0x27, 0x21),
            tint: Rgb::new(0x24, 0x32, 0x2a),
            text: Rgb::new(0xef, 0xe9, 0xda),
            text_muted: Rgb::new(0xa8, 0xb3, 0xaa),
            line: Rgb::new(0x2c, 0x38, 0x30),
            primary: Rgb::new(0x93, 0xc4, 0xa3),
            tile: Rgb::new(0x3a, 0x78, 0x59),
            on_tile: Rgb::new(0xf5, 0xf0, 0xe2),
            butter: Rgb::new(0xf0, 0xd5, 0x82),
            on_butter: Rgb::new(0x14, 0x1c, 0x17),
            nav: Rgb::new(0x0d, 0x13, 0x0f),
            error: Rgb::new(0xe5, 0x9a, 0x83),
            dark: true,
        }
    }
}

/// Reads a colour by the first of `keys` that has a valid `#rrggbb` value. Missing or
/// invalid keys are skipped, so an injected value can never win.
fn pick(table: &toml::Table, keys: &[&str]) -> Option<Rgb> {
    keys.iter().find_map(|key| {
        table
            .get(*key)
            .and_then(|value| value.as_str())
            .and_then(Rgb::parse)
    })
}

/// Maps a `colors.toml` to Crumb's roles. Unparseable TOML is `None`.
pub fn from_omarchy(text: &str) -> Option<Theme> {
    let table: toml::Table = text.parse().ok()?;

    let mode = table.get("mode").and_then(|value| value.as_str());
    let bg = pick(&table, &["background", "bg"]).unwrap_or(DEFAULT_BG);
    let dark = match mode {
        Some("dark") => true,
        Some("light") => false,
        _ => luminance(bg) < DARK_LUMINANCE,
    };

    let fg = pick(&table, &["foreground", "fg"]).unwrap_or(if dark {
        DEFAULT_FG_DARK
    } else {
        DEFAULT_FG_LIGHT
    });
    let muted = pick(&table, &["muted", "dark_foreground", "dark_fg"]).unwrap_or(fg);
    let surface = pick(&table, &["lighter_background", "lighter_bg"]).unwrap_or(bg);
    let elevated = pick(&table, &["dark_background", "dark_bg"]).unwrap_or(surface);
    let accent = pick(&table, &["accent"]).unwrap_or(DEFAULT_ACCENT);
    let selection = pick(&table, &["selection"]).unwrap_or(accent);
    let red = pick(&table, &["red"]).unwrap_or(DEFAULT_RED);
    let bright_red = pick(&table, &["bright_red"]).unwrap_or(red);

    // Omarchy's `muted` is a border shade; mixing the foreground toward the background
    // makes a text-weight dim that still reads.
    let dim = readable(bg, &[mix(fg, bg, 0.7), mix(fg, bg, 0.8), fg]);
    // Butter is the one main action, sitting on the elevated surface.
    let butter = readable(elevated, &[selection, accent, fg]);

    Some(Theme {
        bg,
        paper: surface,
        tint: elevated,
        text: fg,
        text_muted: dim,
        line: muted,
        primary: readable(surface, &[accent, fg]),
        tile: accent,
        on_tile: on(accent),
        butter,
        on_butter: on(butter),
        nav: elevated,
        error: readable(surface, &[red, bright_red]),
        dark,
    })
}

/// The `current/` directory that holds `theme/` and `theme.name`: the XDG state copy if it
/// exists, else `~/.config/omarchy/current`.
pub fn current_dir(home: Option<&Path>, state: Option<&Path>) -> Option<PathBuf> {
    let state_current = match state {
        Some(state) => state.join("omarchy/current"),
        None => home.map(|home| home.join(".local/state/omarchy/current"))?,
    };
    if state_current.is_dir() {
        return Some(state_current);
    }
    let config_current = home.map(|home| home.join(".config/omarchy/current"));
    match config_current {
        Some(path) if path.is_dir() => Some(path),
        _ => None,
    }
}

/// The theme file inside a `current/` directory.
pub fn theme_file(current: &Path) -> PathBuf {
    current.join("theme/colors.toml")
}

/// Loads the active Omarchy theme, or `None` when there is no readable, valid one.
pub fn load_omarchy(home: Option<&Path>, state: Option<&Path>) -> Option<Theme> {
    let current = current_dir(home, state)?;
    let text = std::fs::read_to_string(theme_file(&current)).ok()?;
    from_omarchy(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hand-tuned `solitude` palette from the task, with the expected roles.
    const SOLITUDE: &str = r##"
mode = "dark"
accent = "#798186"
selection = "#343d41"
muted = "#4b4e55"
background = "#101315"
dark_background = "#0c0e10"
lighter_background = "#101315"
foreground = "#cacccc"
dark_foreground = "#4b4e55"
red = "#565d60"
bright_red = "#de6145"
"##;

    const FIXTURES: &[(&str, &str)] = &[
        ("nord", include_str!("../tests/fixtures/themes/nord.toml")),
        (
            "gruvbox",
            include_str!("../tests/fixtures/themes/gruvbox.toml"),
        ),
        (
            "tokyo-night",
            include_str!("../tests/fixtures/themes/tokyo-night.toml"),
        ),
        (
            "catppuccin-latte",
            include_str!("../tests/fixtures/themes/catppuccin-latte.toml"),
        ),
    ];

    #[test]
    fn the_solitude_palette_maps_to_the_expected_roles() {
        let theme = from_omarchy(SOLITUDE).expect("valid toml");
        assert_eq!(theme.text_muted.hex(), "#929495");
        assert_eq!(theme.line.hex(), "#4b4e55");
        assert_eq!(theme.error.hex(), "#de6145");
        assert_eq!(theme.butter.hex(), "#798186");
        assert_eq!(theme.tile.hex(), "#798186");
        assert_eq!(theme.on_tile.hex(), "#171717");
        assert!(theme.dark);
    }

    #[test]
    fn legacy_bg_and_fg_aliases_work() {
        let theme = from_omarchy("bg = \"#101315\"\nfg = \"#cacccc\"\n").expect("valid toml");
        assert_eq!(theme.bg.hex(), "#101315");
        assert_eq!(theme.text.hex(), "#cacccc");
        assert!(theme.dark);
    }

    #[test]
    fn an_explicit_light_mode_wins_over_luminance() {
        let theme = from_omarchy("mode = \"light\"\nbackground = \"#101315\"\n").expect("toml");
        assert!(!theme.dark);
    }

    #[test]
    fn an_absent_mode_is_inferred_from_background_luminance() {
        let dark = from_omarchy("background = \"#101315\"\n").expect("toml");
        assert!(dark.dark);
        let light = from_omarchy("background = \"#f5f1e6\"\n").expect("toml");
        assert!(!light.dark);
    }

    #[test]
    fn an_injection_attempt_is_ignored() {
        let theme = from_omarchy("accent = \"red; color: orange\"\n").expect("valid toml");
        assert_eq!(theme.tile.hex(), DEFAULT_ACCENT.hex());
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let theme =
            from_omarchy("background = \"not a colour\"\naccent = \"#GGGGGG\"\n").expect("toml");
        assert_eq!(theme.bg.hex(), DEFAULT_BG.hex());
        assert_eq!(theme.tile.hex(), DEFAULT_ACCENT.hex());
    }

    #[test]
    fn invalid_toml_is_none() {
        assert_eq!(from_omarchy("this is not = = toml"), None);
    }

    #[test]
    fn every_fixture_reaches_contrast_on_its_text_roles() {
        for (name, text) in FIXTURES {
            let theme = from_omarchy(text).unwrap_or_else(|| panic!("{name}: invalid toml"));
            for (role, fg, bg) in theme.text_roles() {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= MIN_CONTRAST,
                    "{name}: {role} is {ratio:.2}:1 ({fg:?} on {bg:?})"
                );
            }
        }
    }

    #[test]
    fn the_builtin_palettes_reach_contrast_on_their_text_roles() {
        for (name, theme) in [
            ("light", Theme::crumb_light()),
            ("dark", Theme::crumb_dark()),
        ] {
            for (role, fg, bg) in theme.text_roles() {
                let ratio = contrast(fg, bg);
                assert!(ratio >= MIN_CONTRAST, "{name}: {role} is {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn current_dir_prefers_state_then_config() {
        let home = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        assert_eq!(current_dir(Some(home.path()), Some(state.path())), None);

        let config = home.path().join(".config/omarchy/current");
        std::fs::create_dir_all(&config).unwrap();
        assert_eq!(
            current_dir(Some(home.path()), Some(state.path())).as_deref(),
            Some(config.as_path())
        );

        let state_current = state.path().join("omarchy/current");
        std::fs::create_dir_all(&state_current).unwrap();
        assert_eq!(
            current_dir(Some(home.path()), Some(state.path())).as_deref(),
            Some(state_current.as_path())
        );
    }

    #[test]
    fn load_omarchy_reads_the_current_theme() {
        let home = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let current = state.path().join("omarchy/current/theme");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("colors.toml"), SOLITUDE).unwrap();

        let theme = load_omarchy(Some(home.path()), Some(state.path())).expect("loaded");
        assert_eq!(theme.tile.hex(), "#798186");
    }
}
