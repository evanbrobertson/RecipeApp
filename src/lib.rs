//! Crumb: a private recipe box. Paste a link, keep just the recipe.

pub mod api;
pub mod auth;
pub mod browser;
pub mod checks;
pub mod config;
pub mod db;
pub mod error;
pub mod fractions;
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

#[derive(Clone)]
pub struct AppState {
    pub db: db::Db,
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
    /// Wee Chef's import checks: the background queue.
    pub checks: Arc<checks::Checks>,
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
        Self {
            db,
            web: Arc::new(web::Web::new(config.web_dist.clone())),
            images: Arc::new(images::Images::new(
                config.image_cache.clone(),
                images::CACHE_CAP_BYTES,
            )),
            config: Arc::new(config),
            http,
            browser: Arc::new(browser),
            zone: Arc::default(),
            ai: Arc::default(),
            checks: Arc::default(),
            share_misses: Arc::default(),
        }
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
