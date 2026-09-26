//! Serves the Astro build: static files with long-lived caching, and the app's pages
//! with their data inlined so each one renders from a single request.

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Router, routing};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Component, PathBuf};
use std::sync::RwLock;
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::AppState;
use crate::error::AppResult;
use crate::recipes;
use crate::suggestions;

pub const MARKER: &str = r#"<script type="application/json" id="page-data">null</script>"#;

/// HTML templates, cached in release builds and re-read in debug builds.
pub struct Web {
    dist: PathBuf,
    cache: RwLock<HashMap<String, Option<String>>>,
}

impl Web {
    pub fn new(dist: PathBuf) -> Self {
        if !dist.join("index.html").exists() {
            tracing::warn!("No web build at {} (set WEB_DIST)", dist.display());
        }
        Self {
            dist,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Reads `rel` (e.g. "recipes/index.html") from the build.
    pub fn html(&self, rel: &str) -> Option<String> {
        if cfg!(debug_assertions) {
            return std::fs::read_to_string(self.dist.join(rel)).ok();
        }
        if let Some(hit) = self.cache.read().ok().and_then(|c| c.get(rel).cloned()) {
            return hit;
        }
        let loaded = std::fs::read_to_string(self.dist.join(rel)).ok();
        if let Ok(mut c) = self.cache.write() {
            c.insert(rel.to_string(), loaded.clone());
        }
        loaded
    }

    /// A clean page path ("/add", "/recipes/new") to its HTML file, if the build has one.
    fn page_for(&self, path: &str) -> Option<String> {
        let rel = path.trim_matches('/');
        if rel.is_empty() {
            return self.html("index.html");
        }
        let safe = std::path::Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if !safe || rel.starts_with("shell") {
            return None;
        }
        self.html(&format!("{rel}/index.html"))
            .or_else(|| self.html(&format!("{rel}.html")))
    }
}

/// A strong validator for a response body: quoted hex of its SHA-256 (truncated).
pub fn etag_for(body: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(body);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

fn html_response(status: StatusCode, body: String) -> Response {
    let tag = etag_for(body.as_bytes());
    let mut res = (status, body).into_response();
    let h = res.headers_mut();
    if let Ok(tag) = HeaderValue::from_str(&tag) {
        h.insert(header::ETAG, tag);
    }
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert(
        "speculation-rules",
        HeaderValue::from_static("\"/speculation-rules.json\""),
    );
    res
}

pub fn not_found(state: &AppState) -> Response {
    match state.web.html("404.html") {
        Some(page) => html_response(StatusCode::NOT_FOUND, page),
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// JSON safe to drop inside a <script> element.
pub fn inline_json(data: &Value) -> String {
    serde_json::to_string(data)
        .unwrap_or_else(|_| "null".into())
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Serves a template with its page data filled in.
fn render(state: &AppState, template: &str, data: Value) -> Response {
    let Some(page) = state.web.html(template) else {
        return not_found(state);
    };
    let script = MARKER.replace("null", &inline_json(&data));
    html_response(StatusCode::OK, page.replacen(MARKER, &script, 1))
}

fn page(state: &AppState, template: &str, data: AppResult<Value>) -> Response {
    match data {
        Ok(data) => render(state, template, data),
        Err(err) if err.status == StatusCode::NOT_FOUND => not_found(state),
        Err(err) => err.into_response(),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", routing::get(home))
        .route("/recipes", routing::get(recipe_list))
        .route("/recipes/{id}", routing::get(recipe))
        .route("/recipes/{id}/{view}", routing::get(recipe_view))
        .route("/cookbooks", routing::get(cookbook_list))
        .route("/cookbooks/{id}", routing::get(cookbook))
        .route("/more", routing::get(connector_page))
        .route("/suggestions", routing::get(suggestions_page))
        .route("/connect", routing::get(connector_page))
        .route("/import", routing::get(connector_page))
        .route("/login", routing::get(login))
        .route("/random", routing::get(random))
}

async fn home(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let data = (|| {
        let ctx = suggestions::context(&state, &headers, 0);
        let opts = suggestions::Options {
            limit: 4,
            may_call_ai: true,
            ..Default::default()
        };
        let suggested = suggestions::suggestions(&state, ctx, &opts)?;
        let conn = state.db.lock();
        Ok(json!({
            "recipes": recipes::to_value(&recipes::list_recipes(&conn, None, Some(8), None)?),
            "cookbooks": recipes::to_value(&recipes::list_cookbooks(&conn)?),
            "suggestions": recipes::to_value(&suggested),
            "recipeCount": conn.query_row("SELECT count(*) FROM recipes", [], |r| r.get::<_, i64>(0))?,
            // The Add box's photo mode: Wee Chef reads them, or OCR runs in the browser
            "vision": crate::llm::available(&state),
        }))
    })();
    page(&state, "index.html", data)
}

/// Surprise me: a redirect to a random recipe. Never cached, and never prerendered
/// (see speculation-rules.json), or hovering the link would pick one.
async fn random(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    suggestions::zone(&state, &headers);
    let current = q.get("current").and_then(|c| c.parse().ok());
    let picked = suggestions::random(
        &state,
        &suggestions::id_set(q.get("exclude")),
        current,
        &Default::default(),
    );
    let location = match picked {
        Ok(Some(id)) => format!("/recipes/{id}?from=random"),
        Ok(None) => "/".into(),
        Err(err) => return err.into_response(),
    };
    let mut res = crate::auth::found(&location);
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn recipe_list(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let query: String = q
        .get("q")
        .map(|s| s.chars().take(200).collect())
        .unwrap_or_default();
    let data = (|| {
        let conn = state.db.lock();
        Ok(json!({
            "recipes": recipes::to_value(&recipes::list_recipes(&conn, Some(&query), None, None)?),
            "cookbooks": recipes::to_value(&recipes::list_cookbooks(&conn)?),
            "q": query,
        }))
    })();
    page(&state, "recipes/index.html", data)
}

fn numeric(id: &str) -> Option<i64> {
    id.parse::<i64>()
        .ok()
        .filter(|i| *i > 0 && id.bytes().all(|b| b.is_ascii_digit()))
}

async fn recipe(State(state): State<AppState>, Path(id): Path<String>, req: Request) -> Response {
    let Some(id) = numeric(&id) else {
        return static_files(State(state), req).await;
    };
    let mut hero = None;
    let origin = state.config.public_origin(req.headers());
    let data = (|| {
        let conn = state.db.lock();
        let recipe = recipes::require_recipe(&conn, id)?;
        hero = recipe.image.clone().filter(|i| !i.is_empty());
        Ok(json!({
            "recipe": recipes::to_value(&recipe),
            "cookbooks": recipes::to_value(&recipes::list_cookbooks(&conn)?),
            "inCookbooks": recipes::recipe_cookbook_ids(&conn, id)?,
            "cookStats": recipes::to_value(&recipes::cook_stats(&conn, id)?),
            "checks": crate::checks::for_recipe(&conn, id)?,
            // "Check with Wee Chef" in the menu
            "weeChefChecks": crate::checks::enabled(&state),
            // The share sheet's link, if there is one
            "share": crate::share::for_recipe(&conn, id)?
                .map(|s| crate::share::to_json(&s, &origin)),
        }))
    })();
    let mut res = page(&state, "shell/recipe/index.html", data);
    if res.status() == StatusCode::OK
        && let Some(link) =
            hero.and_then(|i| HeaderValue::from_str(&crate::images::hero_preload(id, &i)).ok())
    {
        res.headers_mut().insert(header::LINK, link);
    }
    res
}

async fn recipe_view(
    State(state): State<AppState>,
    Path((id, view)): Path<(String, String)>,
    req: Request,
) -> Response {
    let (Some(id), true) = (
        numeric(&id),
        matches!(view.as_str(), "cook" | "prep" | "edit"),
    ) else {
        return static_files(State(state), req).await;
    };
    let data = (|| {
        let conn = state.db.lock();
        let mut data = json!({"recipe": recipes::to_value(&recipes::require_recipe(&conn, id)?)});
        if view == "edit" {
            data["checks"] = crate::checks::for_recipe(&conn, id)?;
        }
        Ok(data)
    })();
    page(&state, &format!("shell/{view}/index.html"), data)
}

async fn cookbook_list(State(state): State<AppState>) -> Response {
    let data = (|| {
        Ok(json!({"cookbooks": recipes::to_value(&recipes::list_cookbooks(&state.db.lock())?)}))
    })();
    page(&state, "cookbooks/index.html", data)
}

async fn cookbook(State(state): State<AppState>, Path(id): Path<String>, req: Request) -> Response {
    let Some(id) = numeric(&id) else {
        return static_files(State(state), req).await;
    };
    let origin = state.config.public_origin(req.headers());
    let data = (|| {
        let conn = state.db.lock();
        Ok(json!({
            "cookbook": recipes::to_value(&recipes::get_cookbook(&conn, id)?),
            "recipes": recipes::to_value(&recipes::list_recipes(&conn, None, None, None)?),
            // The share sheet's link, if there is one
            "share": crate::share::for_cookbook(&conn, id)?
                .map(|s| crate::share::to_json(&s, &origin)),
        }))
    })();
    page(&state, "shell/cookbook/index.html", data)
}

async fn connector_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    let name = req.uri().path().trim_matches('/').to_string();
    let mut data = json!({"connector": crate::api::connector_info(&state, &headers)});
    // More lists the share links that are live
    if name == "more" {
        let origin = state.config.public_origin(&headers);
        match crate::share::list(&state.db.lock(), &origin) {
            Ok(shares) => data["shares"] = json!(shares),
            Err(err) => tracing::warn!("[share] couldn't list shares: {err}"),
        }
    }
    render(&state, &format!("{name}/index.html"), data)
}

/// The recipes with suggestions waiting and, when Wee Chef checks are set up, their
/// progress for the page's Check all card. Empty is fine: the page says so.
async fn suggestions_page(State(state): State<AppState>) -> Response {
    let data = (|| -> AppResult<Value> {
        let conn = state.db.lock();
        let mut data = json!({"recipes": crate::checks::to_review(&conn)?});
        if crate::checks::enabled(&state) {
            data["checks"] = crate::checks::status(&state, &conn)?;
        }
        Ok(data)
    })();
    match data {
        Ok(data) => render(&state, "suggestions/index.html", data),
        Err(err) => err.into_response(),
    }
}

/// On page loads, tells the page how many recipes have Wee Chef suggestions waiting
/// (`Server-Timing: crumb-review;desc="3"`), so the nav can show Suggestions before it
/// paints. The count is folded into the ETag, so a change is never answered with a 304.
pub async fn review_hint(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let page = req
        .headers()
        .get("sec-fetch-dest")
        .is_some_and(|v| v == "document")
        || req
            .headers()
            .get(header::ACCEPT)
            .is_some_and(|v| v.to_str().is_ok_and(|a| a.contains("text/html")));
    // Nor share pages: nothing about the box goes to someone with a link
    let skip = req.uri().path() == "/login" || req.uri().path().starts_with("/s/");
    let mut res = next.run(req).await;
    // A 304 carries no content type, but the browser refreshes its stored headers from it
    let html = res.status() == StatusCode::NOT_MODIFIED
        || header_str(res.headers(), header::CONTENT_TYPE)
            .is_some_and(|v| v.starts_with("text/html"));
    if !page || skip || !html || !crate::checks::enabled(&state) {
        return res;
    }
    let Ok(count) = crate::checks::review_count(&state.db.lock()) else {
        return res;
    };
    if let Ok(v) = HeaderValue::from_str(&format!("crumb-review;desc=\"{count}\"")) {
        res.headers_mut().append("server-timing", v);
    }
    if let Some(tag) = header_str(res.headers(), header::ETAG).map(String::from)
        && tag.len() >= 2
        && tag.ends_with('"')
        && let Ok(v) = HeaderValue::from_str(&format!("{}-r{count}\"", &tag[..tag.len() - 1]))
    {
        res.headers_mut().insert(header::ETAG, v);
    }
    res
}

async fn login(State(state): State<AppState>, req: Request) -> Response {
    if crate::auth::is_logged_in(&state.config, req.headers()) {
        return crate::auth::found("/");
    }
    static_files(State(state), req).await
}

fn cache_control(path: &str) -> &'static str {
    if path.starts_with("/_astro/") || path.starts_with("/fonts/") {
        "public, max-age=31536000, immutable"
    } else if path == "/speculation-rules.json" {
        "public, max-age=3600"
    } else {
        "public, max-age=86400"
    }
}

/// Everything that isn't an app route: pages by clean URL, then files from the build.
pub async fn static_files(State(state): State<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/shell/") || path == "/shell" {
        return not_found(&state);
    }
    let last = path.rsplit('/').next().unwrap_or("");
    if !last.contains('.') {
        return match state.web.page_for(&path) {
            Some(page) => html_response(StatusCode::OK, page),
            None => not_found(&state),
        };
    }

    let res = match ServeDir::new(&state.config.web_dist)
        .precompressed_br()
        .precompressed_gzip()
        .append_index_html_on_directories(false)
        .oneshot(req)
        .await
    {
        Ok(res) => res.map(Body::new),
        Err(err) => match err {},
    };
    if res.status() == StatusCode::NOT_FOUND {
        return not_found(&state);
    }
    let mut res = res;
    let h = res.headers_mut();
    let is_html = h
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if is_html {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        h.insert(
            "speculation-rules",
            HeaderValue::from_static("\"/speculation-rules.json\""),
        );
    } else {
        h.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_control(&path)),
        );
    }
    if path == "/speculation-rules.json" {
        h.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/speculationrules+json"),
        );
    }
    res
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Whether an `If-None-Match` value matches `tag` (weak comparison, as RFC 9110 asks).
fn none_match(if_none_match: &str, tag: &str) -> bool {
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_string();
    let tag = bare(tag);
    if_none_match
        .split(',')
        .any(|t| t.trim() == "*" || bare(t) == tag)
}

/// Answers `If-None-Match` for responses that carry an `ETag` (pages with injected data,
/// sized images) with an empty 304. Runs outside compression: a compressed body gets
/// its own tag (`"…-br"`), so each encoding keeps a strong validator of its own.
pub async fn conditional(req: Request, next: axum::middleware::Next) -> Response {
    let cacheable = matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let if_none_match = header_str(req.headers(), header::IF_NONE_MATCH).map(String::from);
    let mut res = next.run(req).await;
    if res.status() != StatusCode::OK {
        return res;
    }
    let Some(mut tag) = header_str(res.headers(), header::ETAG).map(String::from) else {
        return res;
    };
    if let Some(enc) = header_str(res.headers(), header::CONTENT_ENCODING)
        .filter(|e| !e.eq_ignore_ascii_case("identity"))
        && tag.len() >= 2
        && tag.starts_with('"')
        && tag.ends_with('"')
    {
        tag = format!("{}-{}\"", &tag[..tag.len() - 1], enc.trim());
        match HeaderValue::from_str(&tag) {
            Ok(v) => {
                res.headers_mut().insert(header::ETAG, v);
            }
            Err(_) => {
                res.headers_mut().remove(header::ETAG);
                return res;
            }
        }
    }
    if !cacheable || !if_none_match.is_some_and(|inm| none_match(&inm, &tag)) {
        return res;
    }
    let mut not_modified = StatusCode::NOT_MODIFIED.into_response();
    for name in [
        header::ETAG,
        header::CACHE_CONTROL,
        header::VARY,
        header::CONTENT_LOCATION,
        header::EXPIRES,
    ] {
        if let Some(v) = res.headers().get(&name) {
            not_modified.headers_mut().insert(name, v.clone());
        }
    }
    for v in res.headers().get_all("server-timing") {
        not_modified
            .headers_mut()
            .append("server-timing", v.clone());
    }
    not_modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_json_cannot_close_the_script() {
        let s = inline_json(&json!({"t": "</script><script>alert(1)</script>"}));
        assert!(!s.contains("</"));
        assert!(s.contains("\\u003c/script>"));
    }

    #[test]
    fn if_none_match_uses_weak_comparison() {
        assert!(none_match("\"abc\"", "\"abc\""));
        assert!(none_match("W/\"abc\", \"def\"", "\"def\""));
        assert!(none_match("W/\"abc\"", "\"abc\""));
        assert!(none_match("*", "\"abc\""));
        assert!(!none_match("\"abc\"", "\"abc-br\""));
        assert_eq!(etag_for(b"x").len(), 34);
    }
}
