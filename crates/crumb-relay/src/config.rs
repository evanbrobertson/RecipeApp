//! Settings from the environment (see docs/RELAY.md). A relay is a small service that fetches
//! pages for whoever holds the token, so a setting that is wrong stops it starting rather than
//! being warned about and ignored.

use std::net::SocketAddr;
use std::time::Duration;

/// The shortest token accepted: 24 characters, about what `openssl rand -hex 16` gives.
pub const MIN_TOKEN_LEN: usize = 24;

#[derive(Debug, Clone)]
pub struct Config {
    /// `RELAY_TOKEN`: what the server sends as `Authorization: Bearer`.
    pub token: String,
    /// `RELAY_LISTEN`
    pub listen: SocketAddr,
    /// `RELAY_NAME`: how this relay is called in the server's log.
    pub name: String,
    /// `RELAY_HOST_INTERVAL_SECS`: the least time between two fetches from the same site.
    pub host_interval: Duration,
    /// `RELAY_CONCURRENCY`: fetches running at once; more are answered 429.
    pub concurrency: usize,
    /// `RELAY_PER_MINUTE`: fetches started in any minute; more are answered 429.
    pub per_minute: usize,
}

impl Config {
    pub const DEFAULT_LISTEN: &'static str = "0.0.0.0:8787";

    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads settings through `get` (the environment, or a map in tests). Blank = unset.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let get = |key: &str| {
            get(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let number = |key: &str, default: u64, min: u64| -> Result<u64, String> {
            match get(key) {
                None => Ok(default),
                Some(raw) => match raw.parse::<u64>() {
                    Ok(n) if n >= min => Ok(n),
                    _ => Err(format!(
                        "{key} should be a whole number of at least {min}, not \"{raw}\""
                    )),
                },
            }
        };

        let token = get("RELAY_TOKEN")
            .ok_or("RELAY_TOKEN is required: a long random secret, e.g. `openssl rand -hex 32`")?;
        if token.len() < MIN_TOKEN_LEN {
            return Err(format!(
                "RELAY_TOKEN is too short: use at least {MIN_TOKEN_LEN} characters, e.g. `openssl rand -hex 32`"
            ));
        }
        let listen = get("RELAY_LISTEN").unwrap_or_else(|| Self::DEFAULT_LISTEN.into());
        let listen = listen.parse().map_err(|_| {
            format!("RELAY_LISTEN should be an address and port like 100.101.102.103:8787, not \"{listen}\"")
        })?;
        Ok(Self {
            token,
            listen,
            name: get("RELAY_NAME").unwrap_or_else(hostname),
            host_interval: Duration::from_secs(number("RELAY_HOST_INTERVAL_SECS", 5, 0)?),
            concurrency: number("RELAY_CONCURRENCY", 2, 1)? as usize,
            per_minute: number("RELAY_PER_MINUTE", 20, 1)? as usize,
        })
    }
}

/// The machine's name, for `RELAY_NAME` when it isn't set.
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "crumb-relay".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(pairs: &[(&str, &str)]) -> Result<Config, String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|key| map.get(key).cloned())
    }

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn defaults() {
        let c = config(&[("RELAY_TOKEN", TOKEN)]).unwrap();
        assert_eq!(c.listen, "0.0.0.0:8787".parse().unwrap());
        assert_eq!(c.host_interval, Duration::from_secs(5));
        assert_eq!((c.concurrency, c.per_minute), (2, 20));
        assert!(!c.name.is_empty());
    }

    #[test]
    fn refuses_to_start_without_a_decent_token() {
        assert!(config(&[]).unwrap_err().contains("RELAY_TOKEN is required"));
        assert!(config(&[("RELAY_TOKEN", "  ")]).is_err());
        assert!(
            config(&[("RELAY_TOKEN", "short")])
                .unwrap_err()
                .contains("too short")
        );
        assert!(config(&[("RELAY_TOKEN", &"x".repeat(MIN_TOKEN_LEN - 1))]).is_err());
        assert!(config(&[("RELAY_TOKEN", &"x".repeat(MIN_TOKEN_LEN))]).is_ok());
    }

    #[test]
    fn settings_are_read_and_checked() {
        let c = config(&[
            ("RELAY_TOKEN", TOKEN),
            ("RELAY_LISTEN", "100.101.102.103:9000"),
            ("RELAY_NAME", "pi1"),
            ("RELAY_HOST_INTERVAL_SECS", "0"),
            ("RELAY_CONCURRENCY", "1"),
            ("RELAY_PER_MINUTE", "6"),
        ])
        .unwrap();
        assert_eq!(c.listen.to_string(), "100.101.102.103:9000");
        assert_eq!(c.name, "pi1");
        assert_eq!(c.host_interval, Duration::ZERO);
        assert_eq!((c.concurrency, c.per_minute), (1, 6));

        for (key, value) in [
            ("RELAY_LISTEN", "not-an-address"),
            ("RELAY_CONCURRENCY", "0"),
            ("RELAY_PER_MINUTE", "0"),
            ("RELAY_PER_MINUTE", "lots"),
            ("RELAY_HOST_INTERVAL_SECS", "-1"),
        ] {
            let err = config(&[("RELAY_TOKEN", TOKEN), (key, value)]).unwrap_err();
            assert!(err.contains(key), "{err}");
        }
    }
}
