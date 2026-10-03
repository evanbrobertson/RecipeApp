//! Where the server and token come from: flags, then the environment, then the file `crumb login`
//! wrote. The file is the only place a token is ever kept, it is `0600`, and nothing here logs it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub server: Option<String>,
    pub token: Option<String>,
}

/// `$CRUMB_CONFIG_DIR`, else `$XDG_CONFIG_HOME/crumb`, else `~/.config/crumb`.
pub fn dir(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        return Some(dir.to_path_buf());
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg).join("crumb"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/crumb"))
}

fn file(dir: &Path) -> PathBuf {
    dir.join("credentials.toml")
}

pub fn load(dir: &Path) -> Saved {
    std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes the file with owner-only permissions from the start, never world-readable.
pub fn save(dir: &Path, saved: &Saved) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    std::fs::create_dir_all(dir)?;
    let text = toml::to_string_pretty(saved).map_err(std::io::Error::other)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut out = options.open(file(dir))?;
    #[cfg(unix)]
    out.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    out.write_all(text.as_bytes())
}
