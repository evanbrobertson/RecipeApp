//! Reading a recipe in Crumb before saving it: `/preview?url={recipe page}`.
//!
//! What the browser extension (`extension/`) and the "Read in Crumb" bookmark open. The page
//! is scraped as an import would scrape it, tidied as an import would tidy it, and shown in
//! the share page's layout (`shell/preview/index.html`, filled by [`crate::share::render`])
//! with an "Add to my Crumb" button. Nothing is saved until that button is pressed: it
//! posts the link to `/api/recipes/import` like the Add box, and the import takes this
//! scrape from [`take`] instead of fetching the page again.
//!
//! The page comes in steps, so a new tab shows something at once:
//! - a link already in the box goes straight to that recipe; a cooking video, or another
//!   Crumb's shared cookbook, goes to the Add page (they're saved, not previewed);
//! - otherwise the first answer is a "reading…" page, which reloads itself with `go=1`;
//!   that request scrapes and answers with the recipe, or with why it couldn't be read.
//!
//! Scraping fetches whatever address it's given, so it only runs for a request the cook
//! made from Crumb or the browser itself (`Sec-Fetch-Site` of `same-origin` or `none`, as
//! from a tab the extension opens). Arriving from another site (a bookmarklet, or any link
//! a page could plant) first asks "Read this recipe in Crumb?", and only the answer, a
//! same-origin request, scrapes. Browsers that send no `Sec-Fetch-Site` are trusted.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::extract::Query;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Router, routing};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::error::AppError;
use crate::households::HouseholdId;
use crate::model::{Recipe, now_secs};
use crate::scraper::Scraped;
use crate::share::{self, escape};

/// The page's Astro template.
pub const TEMPLATE: &str = "shell/preview/index.html";
/// How long a scrape waits for its Add.
const KEEP: Duration = Duration::from_secs(30 * 60);
/// Scrapes kept at once, across households; the oldest go first.
const MAX_KEPT: usize = 64;
/// Scrapes one household may have kept, so one box can't push everyone else's out.
const MAX_KEPT_PER_HOUSEHOLD: usize = 8;

type Key = (HouseholdId, String);

/// Scrapes shown in a preview, by household and link, for the import that follows.
static KEPT: LazyLock<Mutex<HashMap<Key, (Instant, Scraped)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn kept() -> std::sync::MutexGuard<'static, HashMap<Key, (Instant, Scraped)>> {
    let mut map = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, (at, _)| at.elapsed() < KEEP);
    map
}

/// Keeps a scrape for the import that follows: what a preview showed, or what the cook's
/// browser read from the page (`scraper::page`).
pub fn keep(state: &AppState, url: &str, scraped: &Scraped) {
    let mut map = kept();
    let key = (state.household, url.to_string());
    // A household over its own share loses its own oldest first
    while !map.contains_key(&key)
        && map.keys().filter(|(h, _)| *h == state.household).count() >= MAX_KEPT_PER_HOUSEHOLD
    {
        let Some(oldest) = map
            .iter()
            .filter(|((h, _), _)| *h == state.household)
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest);
    }
    while map.len() >= MAX_KEPT {
        let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest);
    }
    map.insert(key, (Instant::now(), scraped.clone()));
}

fn peek(state: &AppState, url: &str) -> Option<Scraped> {
    kept()
        .get(&(state.household, url.to_string()))
        .map(|(_, s)| s.clone())
}

/// The scrape a preview of `url` showed this household, for saving it (once).
pub fn take(state: &AppState, url: &str) -> Option<Scraped> {
    kept()
        .remove(&(state.household, url.to_string()))
        .map(|(_, s)| s)
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/preview", routing::get(page))
}

#[derive(Deserialize)]
struct Params {
    url: Option<String>,
    go: Option<String>,
    /// `extension`: the extension opened this tab and is about to hand over the page as it
    /// read it (see [`waiting`]).
    via: Option<String>,
}

/// Whether the request came from Crumb or the browser itself, not from another site.
fn trusted(headers: &HeaderMap) -> bool {
    match headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        None => true,
        Some(site) => matches!(site, "same-origin" | "none"),
    }
}

fn add_page(url: &str) -> Response {
    let q = percent_encoding::utf8_percent_encode(url, percent_encoding::NON_ALPHANUMERIC);
    Redirect::to(&format!("/add?url={q}")).into_response()
}

async fn page(
    crate::Scoped(state): crate::Scoped,
    headers: HeaderMap,
    Query(params): Query<Params>,
) -> Response {
    let Some(raw) = params
        .url
        .as_deref()
        .map(str::trim)
        .filter(|u| crate::model::is_valid_url(u))
    else {
        return Redirect::to("/add").into_response();
    };
    let url = crate::recipes::unwrap_share_link(raw);
    let saved = crate::recipes::find_by_url(&state.db.lock(), &url);
    match saved {
        Ok(Some(id)) => return Redirect::to(&format!("/recipes/{id}")).into_response(),
        Ok(None) => {}
        Err(err) => return err.into_response(),
    }
    if crate::video::is_video_url(&url) {
        return add_page(&url);
    }
    if let Some(scraped) = peek(&state, &url) {
        return ready(&state, &url, &scraped);
    }
    if !trusted(&headers) {
        return waiting(&state, &url, "ask");
    }
    // The extension read the recipe in the cook's browser and hands it over from this page
    // (`POST /api/preview`), then reloads with `go=1`: nothing is scraped meanwhile. If it
    // could hand nothing over, that reload scrapes as usual.
    if params.go.is_none() && params.via.as_deref() == Some("extension") {
        return waiting(&state, &url, "handover");
    }
    if params.go.is_none() {
        return waiting(&state, &url, "loading");
    }
    match crate::scraper::scrape_page(&state, &url).await {
        // Another Crumb's shared cookbook: nothing to read here, but it saves whole
        Ok(scraped) if scraped.recipe.title.trim().is_empty() => add_page(&url),
        Ok(scraped) => {
            keep(&state, &url, &scraped);
            ready(&state, &url, &scraped)
        }
        Err(err) => failed(&state, &url, &err),
    }
}

/// The recipe as saving it would keep it (tidied), in the share page's layout.
fn ready(state: &AppState, url: &str, scraped: &Scraped) -> Response {
    let mut fields = scraped.recipe.clone();
    crate::checks::tidy_import(&mut fields, "url");
    let now = now_secs();
    let recipe = Recipe {
        id: 0,
        url: Some(url.to_string()),
        source: "url".into(),
        title: fields.title,
        description: fields.description,
        image: fields.image,
        author: fields.author,
        prep_time: fields.prep_time,
        cook_time: fields.cook_time,
        total_time: fields.total_time,
        freeze_time: fields.freeze_time,
        recipe_yield: fields.recipe_yield,
        recipe_category: fields.recipe_category,
        recipe_cuisine: fields.recipe_cuisine,
        ingredients: fields.ingredients,
        instructions: fields.instructions,
        nutrition: fields.nutrition.map(Value::Object),
        notes: fields.notes,
        video_embed: fields
            .video
            .as_deref()
            .and_then(crumb_core::embed::video_embed),
        video: fields.video,
        original_url: None,
        created_at: now,
        updated_at: now,
    };
    // A preview's page shows no absolute links, so no origin is needed
    html(state, StatusCode::OK, |t| {
        share::render(t, &recipe, &share::Place::preview(), "")
    })
}

/// Before the scrape: "reading…" (which carries on by itself) or, arriving from another
/// site, the question.
fn waiting(state: &AppState, url: &str, step: &str) -> Response {
    let host = share::host_of(url).unwrap_or_default();
    let (title, lead) = match step {
        "ask" => ("Read this recipe in Crumb?", format!("From {host}")),
        _ => ("Reading the recipe…", format!("From {host}")),
    };
    let data = json!({"preview": {"state": step, "url": url, "host": host}});
    let lines = |n: usize| {
        let mut out = String::from("<div class=\"preview-skeleton\" aria-hidden=\"true\">");
        for _ in 0..n {
            out.push_str("<div class=\"skeleton rounded-ctl\"></div>");
        }
        out.push_str("</div>");
        out
    };
    html(state, StatusCode::OK, |t| {
        let mut page = share::start_page(t, title, "", &data);
        share::fill(
            &mut page,
            "<!--share:intro-->",
            &format!(
                "<p class=\"kicker\">{}</p><h1 class=\"page-title share-title\">{}</h1>",
                escape(&lead),
                escape(title)
            ),
        );
        share::fill(&mut page, "<!--share:ingredients-->", &lines(6));
        share::fill(&mut page, "<!--share:method-->", &lines(4));
        page
    })
}

/// The scrape didn't give a recipe: why, with the page's own link.
fn failed(state: &AppState, url: &str, err: &AppError) -> Response {
    let (status, message) = (err.status, err.message.as_str());
    let host = share::host_of(url).unwrap_or_default();
    let mut data = json!({"preview": {
        "state": "failed", "url": url, "host": host, "message": message,
    }});
    if let Some(code) = err.code {
        data["preview"]["code"] = code.into();
    }
    let title = "Couldn't read that recipe";
    html(state, status, |t| {
        let mut page = share::start_page(t, title, "", &data);
        share::fill(
            &mut page,
            "<!--share:intro-->",
            &format!(
                "<p class=\"kicker\">From {}</p><h1 class=\"page-title share-title\">{title}</h1>\
                 <p class=\"share-desc\">{}</p>",
                escape(&host),
                escape(message)
            ),
        );
        // No recipe, so no Ingredients and Method (the template hides them around this)
        share::fill(
            &mut page,
            "<!--share:ingredients-->",
            "<span class=\"preview-empty\" hidden></span>",
        );
        page
    })
}

/// The template filled by `fill`, with the preview's CSP (from the template, as for shares).
/// Never cached anywhere: it's the cook's own reading.
fn html(state: &AppState, status: StatusCode, fill: impl FnOnce(&str) -> String) -> Response {
    let Some(template) = state.web.html(TEMPLATE) else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };
    let csp = share::preview_policy(&template);
    let mut res = (status, fill(&template)).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex"),
    );
    if let Ok(v) = HeaderValue::from_str(&csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_crumb_or_the_browser_itself_can_start_a_scrape() {
        let with = |v: &str| {
            let mut h = HeaderMap::new();
            h.insert("sec-fetch-site", HeaderValue::from_str(v).unwrap());
            h
        };
        assert!(trusted(&HeaderMap::new()));
        assert!(trusted(&with("none")));
        assert!(trusted(&with("same-origin")));
        assert!(!trusted(&with("same-site")));
        assert!(!trusted(&with("cross-site")));
    }

    #[test]
    fn one_household_cannot_push_out_anothers_kept_scrape() {
        let home = AppState::new(
            crate::db::open_in_memory().unwrap(),
            crate::config::Config::default(),
            crate::browser::Browser::disabled(),
        );
        let mut other = home.clone();
        other.household = 7_777;
        let scraped = Scraped {
            recipe: crate::model::RecipeFields::default(),
            crumb: None,
        };
        keep(&other, "https://other.test/mine", &scraped);
        for i in 0..MAX_KEPT * 2 {
            keep(&home, &format!("https://flood.test/{i}"), &scraped);
        }
        assert!(peek(&other, "https://other.test/mine").is_some());
        let mine = kept().keys().filter(|(h, _)| *h == home.household).count();
        assert_eq!(mine, MAX_KEPT_PER_HOUSEHOLD);
        // Its own newest are the ones kept
        assert!(peek(&home, &format!("https://flood.test/{}", MAX_KEPT * 2 - 1)).is_some());
        assert!(peek(&home, "https://flood.test/0").is_none());
        take(&other, "https://other.test/mine");
        for i in 0..MAX_KEPT * 2 {
            take(&home, &format!("https://flood.test/{i}"));
        }
    }
}
