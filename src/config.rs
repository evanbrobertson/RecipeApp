//! Settings from the environment. The `NUXT_*` names from the previous version are still
//! read so existing deployments keep their password and keys.

use std::path::PathBuf;

use axum::http::HeaderMap;

#[derive(Debug, Clone)]
pub struct Config {
    /// Password for the web UI and the Claude connector. None = no auth (local dev only).
    pub app_password: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub anthropic_model: String,
    pub anthropic_base_url: String,
    pub site_url: Option<String>,
    pub railway_domain: Option<String>,
    pub web_dist: PathBuf,
    pub host: String,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_password: None,
            anthropic_api_key: None,
            anthropic_model: "claude-sonnet-5".into(),
            anthropic_base_url: "https://api.anthropic.com".into(),
            site_url: None,
            railway_domain: None,
            web_dist: PathBuf::from("web/dist"),
            host: "0.0.0.0".into(),
            port: 3000,
        }
    }
}

fn env(keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty())
}

impl Config {
    pub fn from_env() -> Self {
        let d = Config::default();
        Self {
            app_password: env(&["APP_PASSWORD", "NUXT_APP_PASSWORD"]),
            anthropic_api_key: env(&["ANTHROPIC_API_KEY", "NUXT_ANTHROPIC_API_KEY"]),
            anthropic_model: env(&["ANTHROPIC_MODEL", "NUXT_ANTHROPIC_MODEL"])
                .unwrap_or(d.anthropic_model),
            anthropic_base_url: env(&["ANTHROPIC_BASE_URL"])
                .map(|u| u.trim_end_matches('/').to_string())
                .unwrap_or(d.anthropic_base_url),
            site_url: env(&["SITE_URL", "NUXT_PUBLIC_SITE_URL"]),
            railway_domain: env(&["RAILWAY_PUBLIC_DOMAIN"]),
            web_dist: env(&["WEB_DIST"]).map(PathBuf::from).unwrap_or(d.web_dist),
            host: env(&["HOST"]).unwrap_or(d.host),
            port: env(&["PORT"])
                .and_then(|p| p.parse().ok())
                .unwrap_or(d.port),
        }
    }

    pub fn auth_enabled(&self) -> bool {
        self.app_password.is_some()
    }

    /// Public origin of the app, used for OAuth metadata and the connector URL.
    pub fn public_origin(&self, headers: &HeaderMap) -> String {
        if let Some(url) = &self.site_url {
            return url.trim_end_matches('/').to_string();
        }
        if let Some(domain) = &self.railway_domain {
            return format!("https://{domain}");
        }
        let first = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let proto = first("x-forwarded-proto").unwrap_or_else(|| "http".into());
        let host = first("x-forwarded-host")
            .or_else(|| first("host"))
            .unwrap_or_else(|| format!("localhost:{}", self.port));
        format!("{proto}://{host}")
    }
}
