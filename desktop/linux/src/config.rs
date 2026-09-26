//! Server URL settings, kept in `$XDG_CONFIG_HOME/crumb-desktop/config.json` (falling
//! back to `~/.config/...`). `CRUMB_SERVER` overrides the file.
//!
//! Everything here takes the base directory as a parameter, so tests never touch the
//! real config or the process environment.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Environment variable that overrides the saved server URL.
pub const ENV_SERVER: &str = "CRUMB_SERVER";

const FILE_NAME: &str = "config.json";

/// The app's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub server_url: Option<String>,
}

/// The on-disk shape. Kept separate so absent/extra keys are tolerated.
#[derive(Debug, Serialize, Deserialize)]
struct FileConfig {
    #[serde(default, rename = "serverUrl")]
    server_url: Option<String>,
}

/// `$XDG_CONFIG_HOME/crumb-desktop`, or `~/.config/crumb-desktop`.
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("crumb-desktop")
}

/// Reads `base_dir/config.json`; a missing or unreadable file is simply empty.
pub fn load(base_dir: &Path) -> Config {
    let path = base_dir.join(FILE_NAME);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    match serde_json::from_str::<FileConfig>(&text) {
        Ok(file) => Config {
            server_url: file
                .server_url
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty()),
        },
        Err(err) => {
            eprintln!(
                "crumb-desktop: ignoring unreadable {}: {err}",
                path.display()
            );
            Config::default()
        }
    }
}

/// Writes the server URL, creating the directory if needed.
pub fn save(base_dir: &Path, server_url: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(base_dir)?;
    let file = FileConfig {
        server_url: Some(server_url.trim().to_string()),
    };
    let text = serde_json::to_string_pretty(&file).unwrap_or_default();
    std::fs::write(base_dir.join(FILE_NAME), text)
}

/// The environment override, when set to a non-blank value, wins over the file.
pub fn apply_env(config: Config, env_server: Option<String>) -> Config {
    match env_server
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
    {
        Some(url) => Config {
            server_url: Some(url),
        },
        None => config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_has_no_server() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Config::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), "https://crumb.example").unwrap();
        assert_eq!(
            load(dir.path()).server_url.as_deref(),
            Some("https://crumb.example")
        );
    }

    #[test]
    fn the_environment_overrides_the_file() {
        // Pure function: no process environment is mutated, so tests stay parallel-safe.
        assert_eq!(
            apply_env(
                Config {
                    server_url: Some("https://file.example".into()),
                },
                Some("https://env.example".into()),
            )
            .server_url
            .as_deref(),
            Some("https://env.example")
        );
        assert_eq!(
            apply_env(
                Config {
                    server_url: Some("https://file.example".into()),
                },
                None,
            )
            .server_url
            .as_deref(),
            Some("https://file.example")
        );
        assert_eq!(
            apply_env(Config::default(), Some("   ".into())).server_url,
            None
        );
    }
}
