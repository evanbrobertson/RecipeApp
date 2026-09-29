//! Crumb: a private recipe box. Paste a link, keep just the recipe.

pub use crumb_core::{categories, fractions, markdown, model, suggest, text_parser};

pub mod account_api;
pub mod accounts;
pub mod api;
pub mod auth;
pub mod browser;
pub mod checks;
pub mod config;
pub mod db;
pub mod error;
pub mod guard;
pub mod hosted;
pub mod households;
pub mod images;
pub mod importers;
pub mod llm;
pub mod mcp;
pub mod oauth;
pub mod photos;
pub mod preview;
pub mod recipes;
pub mod relay;
pub mod scraper;
pub mod share;
pub mod site_terms;
pub mod sites;
pub mod social;
pub mod suggestions;
pub mod telemetry;
pub mod throttle;
pub mod trash;
pub mod video;
pub mod video_jobs;
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
    /// People and their sessions with `AUTH_MODE=accounts`; with `hosted`, the ids that map
    /// Better Auth's people and organizations onto local ones.
    pub accounts: Option<Arc<accounts::Accounts>>,
    /// The Better Auth service, with `AUTH_MODE=hosted`.
    pub hosted: Option<Arc<hosted::Hosted>>,
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
    /// Sign-in attempts per client address and account (see [`throttle`]).
    pub logins: Arc<throttle::Throttle>,
    /// OAuth client registrations and similar counted actions.
    pub rates: Arc<throttle::Rate>,
    /// Video imports waiting and running, across households (see [`video_jobs`]).
    pub video_jobs: Arc<video_jobs::VideoJobs>,
    /// `crumb-relay`s asked for pages the server's own fetches were blocked on (see [`relay`]).
    pub relays: Arc<relay::Relays>,
    /// How each recipe site was last read, across households (see [`sites`]).
    pub sites: Arc<sites::Sites>,
    /// Wee Chef's look at sites' terms of service (see [`site_terms`]).
    pub terms: Arc<site_terms::Flagger>,
}

impl AppState {
    pub fn new(db: db::Db, config: config::Config, mut browser: browser::Browser) -> Self {
        // Nothing the server fetches, redirects and Chromium's requests included, may reach a
        // site whose terms forbid automated fetching (see `site_terms`)
        crumb_fetch::guard::set_veto(site_terms::host_is_listed);
        let video_jobs = Arc::new(video_jobs::VideoJobs::new(video_jobs::Limits::from_config(
            &config,
        )));
        // Headless Chromium shares the videos' heavy-work budget
        browser.share_budget(video_jobs.heavy.clone());
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("HTTP client");
        let relays = Arc::new(relay::Relays::from_config(&config));
        let sites = Arc::new(
            match &config.sites_db {
                Some(path) => sites::Sites::open(path),
                None => sites::Sites::open_in_memory(),
            }
            .expect("sites database"),
        );
        let households = Arc::new(households::Households::new(
            db,
            config.households_dir.clone(),
        ));
        let home = households.home().clone();
        let hosted = config.hosted().then(|| {
            let (Some(url), Some(secret)) =
                (&config.auth_service_url, &config.auth_internal_secret)
            else {
                panic!("AUTH_MODE=hosted needs AUTH_SERVICE_URL and AUTH_INTERNAL_SECRET");
            };
            Arc::new(hosted::Hosted::new(
                url,
                secret,
                config.hosted_home_owner.clone(),
            ))
        });
        let accounts = (config.accounts() || config.hosted()).then(|| {
            let opened = match &config.accounts_db {
                Some(path) => accounts::Accounts::open(path),
                None => accounts::Accounts::open_in_memory(),
            };
            Arc::new(opened.expect("accounts database"))
        });
        let terms = Arc::new(site_terms::Flagger::new());
        Self {
            accounts,
            hosted,
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
            logins: Arc::default(),
            rates: Arc::default(),
            video_jobs,
            relays,
            sites,
            terms,
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
        .merge(account_api::routes())
        .merge(social::routes())
        // Hosted: the rest of /api/auth/* is Better Auth's
        .route("/api/auth/{*rest}", axum::routing::any(hosted::proxy))
        .merge(oauth::routes())
        .merge(mcp::routes())
        .merge(images::routes())
        .merge(share::api_routes())
        .merge(share::public_routes())
        .merge(preview::routes())
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
        // State changes must come from this site; /mcp checks a browser's Origin; HSTS over HTTPS
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            guard::same_origin_only,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            guard::mcp_origin,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            guard::hsts,
        ))
        // The client's address, for the sign-in and registration limits
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            throttle::client_ip_layer,
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
