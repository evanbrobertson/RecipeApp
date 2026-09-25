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
    fn html(&self, rel: &str) -> Option<String> {
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

fn html_response(status: StatusCode, body: String) -> Response {
    let mut res = (status, body).into_response();
    let h = res.headers_mut();
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
fn inline_json(data: &Value) -> String {
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
        .route("/connect", routing::get(connector_page))
        .route("/import", routing::get(connector_page))
        .route("/login", routing::get(login))
}

async fn home(State(state): State<AppState>) -> Response {
    let data = (|| {
        let conn = state.db.lock();
        Ok(json!({
            "recipes": recipes::to_value(&recipes::list_recipes(&conn, None, Some(8), None)?),
            "cookbooks": recipes::to_value(&recipes::list_cookbooks(&conn)?),
        }))
    })();
    page(&state, "index.html", data)
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
    let data = (|| {
        let conn = state.db.lock();
        Ok(json!({
            "recipe": recipes::to_value(&recipes::require_recipe(&conn, id)?),
            "cookbooks": recipes::to_value(&recipes::list_cookbooks(&conn)?),
            "inCookbooks": recipes::recipe_cookbook_ids(&conn, id)?,
        }))
    })();
    page(&state, "shell/recipe/index.html", data)
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
        Ok(json!({"recipe": recipes::to_value(&recipes::require_recipe(&conn, id)?)}))
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
    let data = (|| {
        let conn = state.db.lock();
        Ok(json!({
            "cookbook": recipes::to_value(&recipes::get_cookbook(&conn, id)?),
            "recipes": recipes::to_value(&recipes::list_recipes(&conn, None, None, None)?),
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
    let data = json!({"connector": crate::api::connector_info(&state, &headers)});
    render(&state, &format!("{name}/index.html"), data)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_json_cannot_close_the_script() {
        let s = inline_json(&json!({"t": "</script><script>alert(1)</script>"}));
        assert!(!s.contains("</"));
        assert!(s.contains("\\u003c/script>"));
    }
}
