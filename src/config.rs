//! Settings from the environment. The `NUXT_*` names from the previous version are still
//! read so existing deployments keep their password and keys.

use std::path::PathBuf;

use axum::http::HeaderMap;

/// Which AI API parses pasted text and writes suggestion blurbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    DeepSeek,
}

impl LlmProvider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Claude",
            Self::OpenAi => "OpenAI",
            Self::DeepSeek => "DeepSeek",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAi),
            "deepseek" => Some(Self::DeepSeek),
            _ => None,
        }
    }

    /// (key env vars, model env vars, base URL env var, default model, default base URL)
    fn env_names(
        self,
    ) -> (
        &'static [&'static str],
        &'static [&'static str],
        &'static str,
        &'static str,
        &'static str,
    ) {
        match self {
            Self::Anthropic => (
                &["ANTHROPIC_API_KEY", "NUXT_ANTHROPIC_API_KEY"],
                &["ANTHROPIC_MODEL", "NUXT_ANTHROPIC_MODEL"],
                "ANTHROPIC_BASE_URL",
                "claude-sonnet-5",
                "https://api.anthropic.com",
            ),
            Self::OpenAi => (
                &["OPENAI_API_KEY"],
                &["OPENAI_MODEL"],
                "OPENAI_BASE_URL",
                "gpt-5-mini",
                "https://api.openai.com/v1",
            ),
            Self::DeepSeek => (
                &["DEEPSEEK_API_KEY"],
                &["DEEPSEEK_MODEL"],
                "DEEPSEEK_BASE_URL",
                "deepseek-chat",
                "https://api.deepseek.com",
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    /// No trailing slash. Anthropic's is the host; OpenAI-style ones include the version path.
    pub base_url: String,
}

impl LlmConfig {
    /// A provider with its default model and base URL.
    pub fn new(provider: LlmProvider, api_key: impl Into<String>) -> Self {
        let (_, _, _, model, base_url) = provider.env_names();
        Self {
            provider,
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Password for the web UI and the Claude connector. None = no auth (local dev only).
    pub app_password: Option<String>,
    /// The AI API, when a key is configured.
    pub llm: Option<LlmConfig>,
    /// Model for "Try next" blurbs; defaults to the provider's model.
    pub suggest_model: Option<String>,
    /// AI re-ranking of "Try next" (on whenever an AI API is configured, unless SUGGESTIONS_AI=off).
    pub suggestions_ai: bool,
    pub site_url: Option<String>,
    pub railway_domain: Option<String>,
    pub web_dist: PathBuf,
    /// Where resized recipe photos are kept (`img-cache/` next to the database).
    /// None = resize on every request.
    pub image_cache: Option<PathBuf>,
    pub host: String,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_password: None,
            llm: None,
            suggest_model: None,
            suggestions_ai: true,
            site_url: None,
            railway_domain: None,
            web_dist: PathBuf::from("web/dist"),
            image_cache: None,
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

/// `LLM_PROVIDER` picks the provider; without it, the first provider with a key wins
/// (Anthropic, then OpenAI, then DeepSeek).
fn llm_from_env() -> Option<LlmConfig> {
    let all = [
        LlmProvider::Anthropic,
        LlmProvider::OpenAi,
        LlmProvider::DeepSeek,
    ];
    let chosen = match env(&["LLM_PROVIDER"]) {
        Some(name) => match LlmProvider::from_name(&name) {
            Some(p) => vec![p],
            None => {
                tracing::warn!(
                    "Unknown LLM_PROVIDER \"{name}\" (use anthropic, openai or deepseek)"
                );
                all.to_vec()
            }
        },
        None => all.to_vec(),
    };
    chosen.into_iter().find_map(|provider| {
        let (keys, models, base, _, _) = provider.env_names();
        let mut llm = LlmConfig::new(provider, env(keys)?);
        if let Some(model) = env(models) {
            llm.model = model;
        }
        if let Some(url) = env(&[base]) {
            llm.base_url = url.trim_end_matches('/').to_string();
        }
        Some(llm)
    })
}

impl Config {
    pub fn from_env() -> Self {
        let d = Config::default();
        Self {
            app_password: env(&["APP_PASSWORD", "NUXT_APP_PASSWORD"]),
            llm: llm_from_env(),
            suggest_model: env(&["SUGGEST_MODEL"]),
            suggestions_ai: env(&["SUGGESTIONS_AI"])
                .is_none_or(|v| !matches!(v.to_ascii_lowercase().as_str(), "off" | "false" | "0")),
            site_url: env(&["SITE_URL", "NUXT_PUBLIC_SITE_URL"]),
            railway_domain: env(&["RAILWAY_PUBLIC_DOMAIN"]),
            web_dist: env(&["WEB_DIST"]).map(PathBuf::from).unwrap_or(d.web_dist),
            image_cache: Some(crate::db::image_cache_dir(&crate::db::database_path())),
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
