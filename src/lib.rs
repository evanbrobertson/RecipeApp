//! Crumb: a private recipe box. Paste a link, keep just the recipe.

pub mod api;
pub mod auth;
pub mod browser;
pub mod claude;
pub mod config;
pub mod db;
pub mod error;
pub mod importers;
pub mod markdown;
pub mod mcp;
pub mod model;
pub mod oauth;
pub mod recipes;
pub mod scraper;
pub mod text_parser;
pub mod web;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue};
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: db::Db,
    pub config: Arc<config::Config>,
    pub http: reqwest::Client,
    pub web: Arc<web::Web>,
    pub browser: Arc<browser::Browser>,
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
            config: Arc::new(config),
            http,
            browser: Arc::new(browser),
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
        .merge(web::routes())
        .fallback(web::static_files)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_login,
        ))
        .layer(CompressionLayer::new())
        .layer(security_header("x-content-type-options", "nosniff"))
        .layer(security_header(
            "referrer-policy",
            "strict-origin-when-cross-origin",
        ))
        .layer(security_header("x-frame-options", "DENY"))
        .layer(security_header(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
