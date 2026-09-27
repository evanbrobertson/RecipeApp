//! Crumb: a private recipe box. Paste a link, keep just the recipe.

pub mod api;
pub mod auth;
pub mod browser;
pub mod categories;
pub mod checks;
pub mod config;
pub mod db;
pub mod error;
pub mod fractions;
pub mod households;
pub mod images;
pub mod importers;
pub mod llm;
pub mod markdown;
pub mod mcp;
pub mod model;
pub mod oauth;
pub mod photos;
pub mod recipes;
pub mod scraper;
pub mod share;
pub mod suggest;
pub mod suggestions;
pub mod telemetry;
pub mod text_parser;
pub mod web;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue};
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Everything a request needs. `db`, `zone`, `ai` and `queued` are one household's (see
/// [`households`]): the home household's in the router's state, and the signed-in
/// household's in the state [`Scoped`] hands a handler.
#[derive(Clone)]
pub struct AppState {
    /// Whose box `db` is.
    pub household: households::HouseholdId,
    pub db: db::Db,
    /// Every household's box.
    pub households: Arc<households::Households>,
    pub config: Arc<config::Config>,
    pub http: reqwest::Client,
    pub web: Arc<web::Web>,
    pub browser: Arc<browser::Browser>,
    /// The cook's last-seen time zone, for the Try next context.
    pub zone: Arc<suggestions::Zone>,
    /// Cached AI picks for Try next.
    pub ai: Arc<suggestions::AiState>,
    /// Sized recipe photos: disk cache and failure memory.
    pub images: Arc<images::Images>,
    /// Wee Chef's import checks: how many run at once, across households.
    pub checks: Arc<checks::Checks>,
    /// This household's recipes waiting for a check.
    pub queued: Arc<std::sync::Mutex<std::collections::HashSet<i64>>>,
    /// Unknown share tokens asked for, per client address (see `share::Misses`).
    pub share_misses: Arc<share::Misses>,
}

impl AppState {
    pub fn new(db: db::Db, config: config::Config, browser: browser::Browser) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("HTTP client");
        let households = Arc::new(households::Households::new(
            db,
            config.households_dir.clone(),
        ));
        let home = households.home().clone();
        Self {
            household: home.id,
            db: home.db,
            households,
            web: Arc::new(web::Web::new(config.web_dist.clone())),
            images: Arc::new(images::Images::new(
                config.image_cache.clone(),
                images::CACHE_CAP_BYTES,
            )),
            config: Arc::new(config),
            http,
            browser: Arc::new(browser),
            zone: home.zone,
            ai: home.ai,
            checks: Arc::default(),
            queued: home.queued,
            share_misses: Arc::default(),
        }
    }
}

impl AppState {
    /// The same state working on household `id`'s box.
    pub fn for_household(&self, id: households::HouseholdId) -> error::AppResult<Self> {
        if id == self.household {
            return Ok(self.clone());
        }
        let h = self.households.get(id)?;
        Ok(Self {
            household: h.id,
            db: h.db,
            zone: h.zone,
            ai: h.ai,
            queued: h.queued,
            ..self.clone()
        })
    }
}

/// The state for the household a request is signed in to: the router's state (the home
/// household) unless the login check scoped the request to another (see
/// [`auth::require_login`]). Every handler that reads or writes recipes takes this rather
/// than `State<AppState>`.
pub struct Scoped(pub AppState);

impl axum::extract::FromRequestParts<AppState> for Scoped {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<Scoped>()
                .map_or_else(|| state.clone(), |s| s.0.clone()),
        ))
    }
}

impl Clone for Scoped {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

fn security_header(name: &'static str, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    )
}

/// The whole app. Wrap with `NormalizePathLayer` (see main) so trailing slashes are optional.
pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(api::routes())
        .merge(oauth::routes())
        .merge(mcp::routes())
        .merge(images::routes())
        .merge(share::api_routes())
        .merge(share::public_routes())
        .merge(web::routes())
        .fallback(web::static_files)
        // Inside the login check, so only signed-in page loads learn the count
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            web::review_hint,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_login,
        ))
        // Not the gzipped OCR model (/ocr/eng.traineddata.gz): it's already compressed
        .layer(CompressionLayer::new().compress_when(
            DefaultPredicate::new().and(NotForContentType::const_new("application/gzip")),
        ))
        // Outside compression, so it sees (and tags) the encoding actually sent
        .layer(axum::middleware::from_fn(web::conditional))
        .layer(security_header("x-content-type-options", "nosniff"))
        .layer(security_header(
            "referrer-policy",
            "strict-origin-when-cross-origin",
        ))
        .layer(security_header("x-frame-options", "DENY"))
        .layer(security_header(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=(self)",
        ))
        // Share tokens never reach a span (see telemetry::redact_path)
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                tracing::debug_span!(
                    "request",
                    method = %req.method(),
                    uri = %telemetry::redact_path(req.uri().path()),
                    version = ?req.version(),
                )
            }),
        )
        // Outermost: request transactions and the browser's Sentry hint (no-op without SENTRY_DSN)
        .layer(axum::middleware::from_fn(telemetry::middleware))
        .with_state(state)
}
