//! Share links: read-only public pages at `/s/{token}` for one recipe or one cookbook.
//!
//! One link per recipe or cookbook. Creating it again returns the same link; "Stop sharing"
//! deletes the row, so the URL is gone for good and a later share gets a new token. The token
//! is 16 random bytes, kept as it is (it only grants reading what's shared, and the owner must
//! be able to copy the link again). Shares are not part of backups. The table has room for an
//! expiry, which nothing sets yet.
//!
//! Under `/s/{token}` for a recipe (public, outside the login):
//! - the page: the Astro template `shell/share/index.html` with the recipe rendered into it
//!   as plain HTML (so any scraper keeps the sections), Open Graph tags, schema.org JSON-LD
//!   and a `<link rel="alternate">` to the Crumb export;
//! - `img/{width}` and `og.jpg`: the photo, through the same resizer as `/img`;
//! - `crumb.json`: the recipe in the backup format, which another Crumb imports losslessly.
//!
//! For a cookbook the link is live: it shows whatever is in the book when it's opened.
//! - the page: `shell/share-book/index.html`, the book's name, colour and recipe cards;
//! - `{recipeId}` (and its `img/{width}`, `og.jpg`, `crumb.json`): a recipe's page as above,
//!   while that recipe is in the book (404 from the moment it's taken out);
//! - `og.jpg`: the first photo in the book; `crumb.json`: every recipe in it (up to 500),
//!   each in that book, so saving the link in another Crumb recreates the book.
//!
//! Nothing else about the box shows: no cook log, views, checks, other cookbooks, dates or
//! names. Unknown, stopped and expired tokens all get the same plain 404, and an address that
//! collects too many of those in a minute is turned away for a while.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use base64::Engine;
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use sha2::Digest;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::images::{self, Caching, Variant};
use crate::importers::ImportedRecipe;
use crate::model::{Cookbook, Recipe, RecipeFields, RecipeSummary, iso, now_secs};
use crate::recipes;

/// Random bytes in a token (22 characters of base64url).
const TOKEN_BYTES: usize = 16;
/// The Astro template a recipe's page is rendered into.
pub const TEMPLATE: &str = "shell/share/index.html";
/// And a cookbook's.
pub const BOOK_TEMPLATE: &str = "shell/share-book/index.html";
/// `last_opened_at` is written at most this often per share.
const TOUCH_EVERY_SECS: i64 = 3600;
/// Largest Crumb export another Crumb's share page may hand us (an embedded photo can be big).
const MAX_EXPORT_BYTES: usize = 20 * 1024 * 1024;
const EXPORT_TIMEOUT: Duration = Duration::from_secs(15);

// ─── The table ──────────────────────────────────────────────────────────────

/// What a share shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Recipe(i64),
    Cookbook(i64),
}

impl Target {
    fn column(self) -> &'static str {
        match self {
            Self::Recipe(_) => "recipe_id",
            Self::Cookbook(_) => "cookbook_id",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Recipe(_) => "recipe",
            Self::Cookbook(_) => "cookbook",
        }
    }

    fn id(self) -> i64 {
        match self {
            Self::Recipe(id) | Self::Cookbook(id) => id,
        }
    }

    /// A 404 when the recipe or cookbook isn't there.
    fn require(self, conn: &Connection) -> AppResult<()> {
        match self {
            Self::Recipe(id) => recipes::require_recipe(conn, id).map(|_| ()),
            Self::Cookbook(id) => recipes::require_cookbook(conn, id).map(|_| ()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub id: i64,
    pub token: String,
    pub target: Target,
    pub include_notes: bool,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub last_opened_at: Option<i64>,
}

const COLUMNS: &str = "id, token, kind, recipe_id, cookbook_id, include_notes, created_at, expires_at, last_opened_at";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Share> {
    let kind: String = r.get(2)?;
    let target = match kind.as_str() {
        "cookbook" => Target::Cookbook(r.get(4)?),
        _ => Target::Recipe(r.get(3)?),
    };
    Ok(Share {
        id: r.get(0)?,
        token: r.get(1)?,
        target,
        include_notes: r.get::<_, i64>(5)? != 0,
        created_at: r.get(6)?,
        expires_at: r.get(7)?,
        last_opened_at: r.get(8)?,
    })
}

/// Tokens are base64url; anything else is never looked up.
fn plausible_token(token: &str) -> bool {
    (16..=64).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn for_target(conn: &Connection, target: Target) -> AppResult<Option<Share>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM shares WHERE kind = ?1 AND {} = ?2",
                target.column()
            ),
            params![target.kind(), target.id()],
            from_row,
        )
        .optional()?)
}

pub fn for_recipe(conn: &Connection, recipe_id: i64) -> AppResult<Option<Share>> {
    for_target(conn, Target::Recipe(recipe_id))
}

pub fn for_cookbook(conn: &Connection, cookbook_id: i64) -> AppResult<Option<Share>> {
    for_target(conn, Target::Cookbook(cookbook_id))
}

/// The recipe's or cookbook's share link, made if it has none.
pub fn get_or_create(conn: &Connection, target: Target) -> AppResult<Share> {
    target.require(conn)?;
    for _ in 0..3 {
        if let Some(share) = for_target(conn, target)? {
            return Ok(share);
        }
        // DO NOTHING covers both a share made meanwhile and a (vanishingly unlikely) token clash
        conn.execute(
            &format!(
                "INSERT INTO shares (token, kind, {}, include_notes, created_at)
                 VALUES (?1, ?2, ?3, 1, ?4) ON CONFLICT DO NOTHING",
                target.column()
            ),
            params![
                crate::auth::random_token(TOKEN_BYTES),
                target.kind(),
                target.id(),
                now_secs()
            ],
        )?;
    }
    for_target(conn, target)?.ok_or_else(|| AppError::internal("couldn't create a share"))
}

/// Stops sharing: the link 404s from now on. Whether there was one.
pub fn delete_for(conn: &Connection, target: Target) -> AppResult<bool> {
    Ok(conn.execute(
        &format!(
            "DELETE FROM shares WHERE kind = ?1 AND {} = ?2",
            target.column()
        ),
        params![target.kind(), target.id()],
    )? > 0)
}

/// Whether the share shows the notes. By recipe or cookbook id, so the token never goes into
/// an API URL (and from there into traces and error reports).
pub fn set_include_notes(conn: &Connection, target: Target, include: bool) -> AppResult<Share> {
    target.require(conn)?;
    conn.execute(
        &format!(
            "UPDATE shares SET include_notes = ?1 WHERE kind = ?2 AND {} = ?3",
            target.column()
        ),
        params![include, target.kind(), target.id()],
    )?;
    for_target(conn, target)?.ok_or_else(|| AppError::not_found("Share link not found"))
}

/// What a live token opens.
pub enum Found {
    Recipe(Share, Box<Recipe>),
    Book(Share, Cookbook),
}

impl Found {
    fn share(&self) -> &Share {
        match self {
            Self::Recipe(share, _) | Self::Book(share, _) => share,
        }
    }
}

/// A live share and what it shows: None for an unknown, stopped or expired token.
pub fn lookup(conn: &Connection, token: &str, now: i64) -> AppResult<Option<Found>> {
    if !plausible_token(token) {
        return Ok(None);
    }
    let share = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM shares WHERE token = ?1"),
            [token],
            from_row,
        )
        .optional()?
        .filter(|s| s.expires_at.is_none_or(|at| at > now));
    let Some(share) = share else {
        return Ok(None);
    };
    Ok(match share.target {
        Target::Recipe(id) => {
            recipes::get_recipe(conn, id)?.map(|recipe| Found::Recipe(share, Box::new(recipe)))
        }
        Target::Cookbook(id) => recipes::require_cookbook(conn, id)
            .ok()
            .map(|book| Found::Book(share, book)),
    })
}

/// Notes the page was opened, at most once an hour.
fn touch(conn: &Connection, share: &Share, now: i64) -> AppResult<()> {
    if share
        .last_opened_at
        .is_some_and(|at| now - at < TOUCH_EVERY_SECS)
    {
        return Ok(());
    }
    conn.execute(
        "UPDATE shares SET last_opened_at = ?1 WHERE id = ?2",
        params![now, share.id],
    )?;
    Ok(())
}

pub fn share_url(origin: &str, token: &str) -> String {
    format!("{origin}/s/{token}")
}

/// What the recipe and cookbook pages and the API get: `{token, url, includeNotes, createdAt}`.
pub fn to_json(share: &Share, origin: &str) -> Value {
    json!({
        "token": share.token,
        "url": share_url(origin, &share.token),
        "includeNotes": share.include_notes,
        "createdAt": iso(share.created_at),
    })
}

/// Every live share, newest first, for the More page: what it shows (its title, and the
/// recipe or cookbook id the API's share routes take), its link, and when it was made and
/// last opened.
pub fn list(conn: &Connection, origin: &str) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT s.token, s.kind, s.recipe_id, s.cookbook_id, s.include_notes, s.created_at,
                s.last_opened_at, coalesce(r.title, c.name)
         FROM shares s
         LEFT JOIN recipes r ON s.kind = 'recipe' AND r.id = s.recipe_id
         LEFT JOIN cookbooks c ON s.kind = 'cookbook' AND c.id = s.cookbook_id
         WHERE (s.expires_at IS NULL OR s.expires_at > ?1) AND coalesce(r.id, c.id) IS NOT NULL
         ORDER BY s.created_at DESC, s.id DESC",
    )?;
    let rows = stmt.query_map([now_secs()], |r| {
        let token: String = r.get(0)?;
        let kind: String = r.get(1)?;
        let id: i64 = if kind == "cookbook" {
            r.get(3)?
        } else {
            r.get(2)?
        };
        Ok(json!({
            "kind": kind,
            "id": id,
            "title": r.get::<_, String>(7)?,
            "url": share_url(origin, &token),
            "includeNotes": r.get::<_, i64>(4)? != 0,
            "createdAt": iso(r.get(5)?),
            "lastOpenedAt": r.get::<_, Option<i64>>(6)?.map(iso),
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ─── Routes ─────────────────────────────────────────────────────────────────

/// Behind the login, with the rest of /api.
pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/recipes/{id}/share",
            routing::post(create_recipe_share)
                .patch(update_recipe_share)
                .delete(stop_recipe_share),
        )
        .route(
            "/api/cookbooks/{id}/share",
            routing::post(create_book_share)
                .patch(update_book_share)
                .delete(stop_book_share),
        )
        .route("/api/shares", routing::get(list_shares))
}

/// The public pages under /s/ (see `auth::PUBLIC_PREFIXES`).
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/s/{token}", routing::get(page))
        .route("/s/{token}/{*rest}", routing::get(file))
        .layer(axum::middleware::map_response(public_headers))
}

async fn list_shares(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let origin = state.config.public_origin(&headers);
    Ok(Json(json!(list(&state.db.lock(), &origin)?)))
}

fn recipe_target(id: &str) -> AppResult<Target> {
    Ok(Target::Recipe(crate::api::id_param(id, "id")?))
}

fn book_target(id: &str) -> AppResult<Target> {
    Ok(Target::Cookbook(crate::api::id_param(id, "id")?))
}

async fn create_recipe_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    create(&state, recipe_target(&id)?, &headers)
}

async fn create_book_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    create(&state, book_target(&id)?, &headers)
}

async fn stop_recipe_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    stop(&state, recipe_target(&id)?)
}

async fn stop_book_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    stop(&state, book_target(&id)?)
}

async fn update_recipe_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    update(&state, recipe_target(&id)?, &headers, &body)
}

async fn update_book_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    update(&state, book_target(&id)?, &headers, &body)
}

fn create(state: &AppState, target: Target, headers: &HeaderMap) -> AppResult<Json<Value>> {
    let share = get_or_create(&state.db.lock(), target)?;
    Ok(Json(to_json(&share, &state.config.public_origin(headers))))
}

fn stop(state: &AppState, target: Target) -> AppResult<Json<Value>> {
    let conn = state.db.lock();
    target.require(&conn)?;
    delete_for(&conn, target)?;
    Ok(Json(json!({"ok": true})))
}

fn update(
    state: &AppState,
    target: Target,
    headers: &HeaderMap,
    body: &[u8],
) -> AppResult<Json<Value>> {
    let body: Value =
        serde_json::from_slice(body).map_err(|_| AppError::bad_request("Invalid JSON body"))?;
    let include = body
        .get("includeNotes")
        .and_then(Value::as_bool)
        .ok_or_else(|| AppError::bad_request("includeNotes: expected true or false"))?;
    let share = set_include_notes(&state.db.lock(), target, include)?;
    Ok(Json(to_json(&share, &state.config.public_origin(headers))))
}

/// Every /s/ response: kept out of search indexes, and no Referer to the recipe's links.
async fn public_headers(mut res: Response) -> Response {
    let h = res.headers_mut();
    h.insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, noimageindex"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
        );
    }
    res
}

/// The one answer for every token that isn't live, whatever the reason.
fn missing() -> Response {
    let mut res = (StatusCode::NOT_FOUND, "Not found").into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn too_many() -> Response {
    let mut res = (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
    res
}

/// Looks a token up for a public request, counting misses against the client's address.
fn find(state: &AppState, req: &Request, token: &str) -> Result<Found, Box<Response>> {
    let ip = client_ip(req, state.config.trust_proxy_headers);
    if state.share_misses.blocked(&ip) {
        return Err(Box::new(too_many()));
    }
    match lookup(&state.db.lock(), token, now_secs()) {
        Ok(Some(Found::Recipe(share, recipe))) => {
            Ok(Found::Recipe(share, Box::new(shown(*recipe))))
        }
        Ok(Some(found)) => Ok(found),
        Ok(None) => {
            state.share_misses.miss(&ip);
            Err(Box::new(missing()))
        }
        Err(err) => Err(Box::new(err.into_response())),
    }
}

/// A recipe as a share shows it: its page, JSON-LD and crumb.json show a category from
/// before the fixed list filed under it (or none), as the recipe will be once it's checked.
fn shown(mut recipe: Recipe) -> Recipe {
    recipe.recipe_category = crate::categories::for_import(recipe.recipe_category.as_deref());
    recipe
}

/// A miss inside a live share (a recipe not in the book, a file that isn't there): counted
/// like an unknown token, so a link can't be used to probe a box's recipe ids quickly.
fn miss(state: &AppState, ip: &str) -> Response {
    state.share_misses.miss(ip);
    missing()
}

async fn page(State(state): State<AppState>, Path(token): Path<String>, req: Request) -> Response {
    let found = match find(&state, &req, &token) {
        Ok(found) => found,
        Err(res) => return *res,
    };
    if let Err(err) = touch(&state.db.lock(), found.share(), now_secs()) {
        tracing::warn!("[share] couldn't note an open: {err}");
    }
    let origin = state.config.public_origin(req.headers());
    match found {
        Found::Recipe(share, recipe) => {
            let place = Place::recipe(&share);
            html_page(&state, TEMPLATE, |t| render(t, &recipe, &place, &origin))
        }
        Found::Book(share, book) => {
            let listed = match book_listing(&state.db.lock(), book.id) {
                Ok(listed) => listed,
                Err(err) => return err.into_response(),
            };
            html_page(&state, BOOK_TEMPLATE, |t| {
                render_book(t, &book, &listed, &share, &origin)
            })
        }
    }
}

/// A recipe's page inside a shared cookbook, while it's in the book.
fn book_recipe_page(
    state: &AppState,
    origin: &str,
    share: &Share,
    book: &Cookbook,
    recipe: &Recipe,
) -> Response {
    if let Err(err) = touch(&state.db.lock(), share, now_secs()) {
        tracing::warn!("[share] couldn't note an open: {err}");
    }
    let place = Place::in_book(share, book, recipe.id);
    html_page(state, TEMPLATE, |t| render(t, recipe, &place, origin))
}

/// A template filled by `fill`, with the CSP worked out from the template (not the filled
/// page: nothing a recipe or book puts in the page can ever authorise a script of its own).
fn html_page(state: &AppState, template: &str, fill: impl FnOnce(&str) -> String) -> Response {
    let Some(template) = state.web.html(template) else {
        return missing();
    };
    let csp = template_csp(&template);
    let body = fill(&template);
    let tag = crate::web::etag_for(body.as_bytes());
    let mut res = body.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if let Ok(v) = HeaderValue::from_str(&tag) {
        h.insert(header::ETAG, v);
    }
    if let Ok(v) = HeaderValue::from_str(&csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

/// Everything under a share's link: see the module docs.
async fn file(
    State(state): State<AppState>,
    Path((token, rest)): Path<(String, String)>,
    req: Request,
) -> Response {
    let found = match find(&state, &req, &token) {
        Ok(found) => found,
        Err(res) => return *res,
    };
    let v = req
        .uri()
        .query()
        .and_then(|q| serde_urlencoded::from_str::<HashMap<String, String>>(q).ok())
        .and_then(|q| q.get("v").cloned());
    // Nothing of the request is held across an await (its body isn't Sync)
    let ip = client_ip(&req, state.config.trust_proxy_headers);
    let origin = state.config.public_origin(req.headers());
    drop(req);
    match found {
        Found::Recipe(share, recipe) => {
            recipe_file(&state, &ip, &share, &recipe, &rest, v.as_deref()).await
        }
        Found::Book(share, book) => {
            let (head, tail) = match rest.split_once('/') {
                Some((head, tail)) => (head, Some(tail)),
                None => (rest.as_str(), None),
            };
            match (head, tail) {
                ("crumb.json", None) => {
                    let base = share_url(&origin, &token);
                    let conn = state.db.lock();
                    match recipes::cookbook_recipes(&conn, book.id, recipes::MAX_BOOK_RECIPES) {
                        Ok(list) => {
                            let list: Vec<Recipe> = list.into_iter().map(shown).collect();
                            let doc = recipes::export_book(
                                &book,
                                &list,
                                recipes::BookExport::Shared {
                                    base: &base,
                                    include_notes: share.include_notes,
                                },
                            );
                            json_file(&doc, &book.name)
                        }
                        Err(err) => err.into_response(),
                    }
                }
                ("og.jpg", None) => {
                    let cover = book_listing(&state.db.lock(), book.id)
                        .ok()
                        .and_then(|l| cover(&l).map(|r| r.id));
                    match cover {
                        // `v` is the cover's photo key, so a new cover gets a new URL
                        Some(id) => photo(&state, id, Variant::Preview, v.as_deref()).await,
                        None => missing(),
                    }
                }
                (id, tail) => {
                    let Some(recipe) = book_member(&state, book.id, id) else {
                        return miss(&state, &ip);
                    };
                    match tail {
                        None => book_recipe_page(&state, &origin, &share, &book, &recipe),
                        Some(tail) => {
                            recipe_file(&state, &ip, &share, &recipe, tail, v.as_deref()).await
                        }
                    }
                }
            }
        }
    }
}

/// A recipe in the book, by its id as written in the path: None unless it's in the book now.
fn book_member(state: &AppState, book_id: i64, raw: &str) -> Option<Recipe> {
    let id = raw
        .parse::<i64>()
        .ok()
        .filter(|i| *i > 0 && i.to_string() == raw)?;
    let conn = state.db.lock();
    if !recipes::in_cookbook(&conn, book_id, id).ok()? {
        return None;
    }
    recipes::get_recipe(&conn, id).ok().flatten().map(shown)
}

/// `crumb.json`, `og.jpg` and `img/{width}` for one shared recipe (alone or in a book).
async fn recipe_file(
    state: &AppState,
    ip: &str,
    share: &Share,
    recipe: &Recipe,
    rest: &str,
    v: Option<&str>,
) -> Response {
    let variant = match rest {
        "crumb.json" => {
            return json_file(
                &recipes::export_shared(recipe, share.include_notes),
                &recipe.title,
            );
        }
        "og.jpg" => Variant::Preview,
        other => match other.strip_prefix("img/").and_then(exact_width) {
            Some(width) => Variant::Webp(width),
            None => return miss(state, ip),
        },
    };
    photo(state, recipe.id, variant, v).await
}

async fn photo(state: &AppState, id: i64, variant: Variant, v: Option<&str>) -> Response {
    let res = images::serve_photo(state, id, variant, v, Caching::Public).await;
    if res.status() == StatusCode::NOT_FOUND {
        return missing();
    }
    res
}

/// Only the widths the resizer makes, as written.
fn exact_width(raw: &str) -> Option<u32> {
    images::parse_width(raw).filter(|w| w.to_string() == raw)
}

fn json_file(doc: &Value, name: &str) -> Response {
    let body = serde_json::to_string_pretty(doc).unwrap_or_else(|_| "{}".into());
    let mut res = body.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(v) = crate::api::attachment(name, "json") {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    res
}

/// The recipes a shared cookbook's page lists, in the book's order (up to 500).
fn book_listing(conn: &Connection, id: i64) -> AppResult<Vec<RecipeSummary>> {
    let mut list = recipes::get_cookbook(conn, id)?.recipes;
    list.truncate(recipes::MAX_BOOK_RECIPES);
    for r in &mut list {
        r.recipe_category = crate::categories::for_import(r.recipe_category.as_deref());
    }
    Ok(list)
}

/// The book's cover for link previews: its first recipe with a photo.
fn cover(list: &[RecipeSummary]) -> Option<&RecipeSummary> {
    list.iter()
        .find(|r| r.image.as_deref().is_some_and(|i| !i.trim().is_empty()))
}

// ─── Who's asking ───────────────────────────────────────────────────────────

/// The client's address for the miss limit (see [`client_ip_from`]).
pub fn client_ip(req: &Request, trust_proxy_headers: bool) -> String {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    client_ip_from(req.headers(), peer, trust_proxy_headers)
}

/// Behind Railway's proxy: `X-Real-IP`, which the proxy sets itself, else the last
/// `X-Forwarded-For` entry (the one the proxy appended; earlier ones are whatever the client
/// sent, so trusting them would let anyone dodge the limit or get someone else blocked).
/// Otherwise the connection's peer. "unknown" when there's neither (tests).
pub fn client_ip_from(headers: &HeaderMap, peer: Option<IpAddr>, trust_proxy: bool) -> String {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let forwarded = trust_proxy
        .then(|| {
            header("x-real-ip")
                .and_then(|v| v.trim().parse::<IpAddr>().ok())
                .or_else(|| {
                    header("x-forwarded-for")
                        .and_then(|v| v.rsplit(',').next())
                        .and_then(|v| v.trim().parse::<IpAddr>().ok())
                })
        })
        .flatten();
    forwarded
        .or(peer)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Misses (404s) per client address under /s/, in fixed one-minute windows. Bounded: at
/// most [`MAX_TRACKED`] addresses are remembered; past that, stale windows are dropped, and
/// if that isn't enough, the address whose window started longest ago. A flood of new
/// addresses never resets everyone else's count.
#[derive(Default)]
pub struct Misses {
    seen: Mutex<HashMap<String, (Instant, u32)>>,
}

pub const MISS_LIMIT: u32 = 30;
const MISS_WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED: usize = 4096;

impl Misses {
    fn locked(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Instant, u32)>> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn blocked(&self, ip: &str) -> bool {
        self.locked()
            .get(ip)
            .is_some_and(|(at, n)| at.elapsed() < MISS_WINDOW && *n >= MISS_LIMIT)
    }

    pub fn miss(&self, ip: &str) {
        let mut seen = self.locked();
        if seen.len() >= MAX_TRACKED && !seen.contains_key(ip) {
            seen.retain(|_, (at, _)| at.elapsed() < MISS_WINDOW);
            if seen.len() >= MAX_TRACKED
                && let Some(oldest) = seen
                    .iter()
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(k, _)| k.clone())
            {
                seen.remove(&oldest);
            }
        }
        let entry = seen.entry(ip.to_string()).or_insert((Instant::now(), 0));
        if entry.0.elapsed() >= MISS_WINDOW {
            *entry = (Instant::now(), 0);
        }
        entry.1 = entry.1.saturating_add(1);
    }
}

// ─── The page ───────────────────────────────────────────────────────────────

/// Text for HTML content and attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn host_of(url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_string();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

fn present(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Where the recipe came from, as a link the page may show: its original source when it was
/// saved from a share, else its own link. Only http(s): older rows may hold anything.
fn source_url(r: &Recipe) -> Option<&str> {
    [&r.original_url, &r.url]
        .into_iter()
        .filter_map(present)
        .find(|u| crate::model::is_valid_url(u))
}

/// At most `max` characters, cut at a word and marked with an ellipsis.
fn clip(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        return s;
    }
    let cut: String = s.chars().take(max).collect();
    let at = cut.rfind(' ').unwrap_or(cut.len());
    format!("{}…", cut[..at].trim_end_matches([',', '.', ';', ':']))
}

static DURATION_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(\d+(?:[.,]\d+)?)\s*(days?|d|hours?|hrs?|h|minutes?|mins?|m)\b[\s,]*(?:and\s+)?",
    )
    .unwrap()
});

/// A stored time ("1h 10m", "45 mins", "1 hour 30 minutes", or ISO already) as an ISO 8601
/// duration, when it's one of those shapes.
pub fn iso_duration(text: &str) -> Option<String> {
    let text = text.trim();
    let minutes = match crate::scraper::iso_duration_minutes(text) {
        Some(m) => m,
        None => {
            let mut rest = text;
            let mut total = 0.0;
            while !rest.is_empty() {
                let c = DURATION_PART.captures(rest)?;
                let n: f64 = c[1].replace(',', ".").parse().ok()?;
                total += n * match c[2].to_ascii_lowercase().chars().next() {
                    Some('d') => 1440.0,
                    Some('h') => 60.0,
                    _ => 1.0,
                };
                rest = &rest[c[0].len()..];
            }
            total
        }
    };
    let minutes = minutes.round() as i64;
    if minutes <= 0 {
        return None;
    }
    let (h, m) = (minutes / 60, minutes % 60);
    Some(match (h, m) {
        (0, m) => format!("PT{m}M"),
        (h, 0) => format!("PT{h}H"),
        (h, m) => format!("PT{h}H{m}M"),
    })
}

const NUTRITION_LABELS: [(&str, &str); 12] = [
    ("calories", "Calories"),
    ("fatContent", "Fat"),
    ("saturatedFatContent", "Saturated fat"),
    ("unsaturatedFatContent", "Unsaturated fat"),
    ("transFatContent", "Trans fat"),
    ("carbohydrateContent", "Carbs"),
    ("sugarContent", "Sugar"),
    ("fiberContent", "Fiber"),
    ("proteinContent", "Protein"),
    ("cholesterolContent", "Cholesterol"),
    ("sodiumContent", "Sodium"),
    ("servingSize", "Serving size"),
];

fn nutrition_entries(recipe: &Recipe) -> Vec<(String, String)> {
    let Some(Value::Object(map)) = &recipe.nutrition else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(k, v)| {
            let value = match v {
                Value::String(s) => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            (!value.is_empty()).then(|| (k.clone(), value))
        })
        .collect()
}

/// Lucide icons (ISC licence) for the page's meta row, drawn inline like `Icon.astro`.
fn icon(name: &str) -> String {
    let body = match name {
        "timer" => {
            r#"<line x1="10" x2="14" y1="2" y2="2"/><line x1="12" x2="15" y1="14" y2="11"/><circle cx="12" cy="14" r="8"/>"#
        }
        "flame" => {
            r#"<path d="M12 3q1 4 4 6.5t3 5.5a1 1 0 0 1-14 0 5 5 0 0 1 1-3 1 1 0 0 0 5 0c0-2-1.5-3-1.5-5q0-2 2.5-4"/>"#
        }
        "snowflake" => {
            r#"<path d="m10 20-1.25-2.5L6 18"/><path d="M10 4 8.75 6.5 6 6"/><path d="m14 20 1.25-2.5L18 18"/><path d="m14 4 1.25 2.5L18 6"/><path d="m17 21-3-6h-4"/><path d="m17 3-3 6 1.5 3"/><path d="M2 12h6.5L10 9"/><path d="m20 10-1.5 2 1.5 2"/><path d="M22 12h-6.5L14 15"/><path d="m4 10 1.5 2L4 14"/><path d="m7 21 3-6-1.5-3"/><path d="m7 3 3 6h4"/>"#
        }
        "clock" => r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#,
        "users" => {
            r#"<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><path d="M16 3.128a4 4 0 0 1 0 7.744"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><circle cx="9" cy="7" r="4"/>"#
        }
        "note" => {
            r#"<path d="M21 9a2.4 2.4 0 0 0-.706-1.706l-3.588-3.588A2.4 2.4 0 0 0 15 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2z"/><path d="M15 3v5a1 1 0 0 0 1 1h5"/>"#
        }
        "back" => r#"<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>"#,
        "pot" => {
            r#"<path d="M2 12h20"/><path d="M20 12v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8"/><path d="m4 8 16-4"/><path d="m8.86 6.78-.45-1.81a2 2 0 0 1 1.45-2.43l1.94-.48a2 2 0 0 1 2.43 1.46l.45 1.8"/>"#
        }
        "external" => {
            r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"#
        }
        _ => "",
    };
    format!(
        r#"<svg class="share-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{body}</svg>"#
    )
}

/// Where a recipe's page sits: its path under /s/, whether it shows the notes, and the
/// shared cookbook it's in (its name and page, for the link back).
pub struct Place {
    pub path: String,
    pub include_notes: bool,
    pub book: Option<(String, String)>,
}

impl Place {
    /// A recipe shared on its own: `/s/{token}`.
    pub fn recipe(share: &Share) -> Self {
        Self {
            path: format!("/s/{}", share.token),
            include_notes: share.include_notes,
            book: None,
        }
    }

    /// A recipe in a shared cookbook: `/s/{token}/{recipeId}`.
    pub fn in_book(share: &Share, book: &Cookbook, recipe_id: i64) -> Self {
        let book_path = format!("/s/{}", share.token);
        Self {
            path: format!("{book_path}/{recipe_id}"),
            include_notes: share.include_notes,
            book: Some((book.name.clone(), book_path)),
        }
    }
}

/// Everything the page shows, worked out once.
struct View<'a> {
    recipe: &'a Recipe,
    notes: Option<&'a str>,
    page_url: String,
    export_url: String,
    /// `/s/{token}/img` (relative, for the page) and the absolute preview image.
    img_base: String,
    image_key: Option<String>,
    preview_url: Option<String>,
    description: String,
}

impl<'a> View<'a> {
    fn new(recipe: &'a Recipe, place: &Place, origin: &str) -> Self {
        let page_url = format!("{origin}{}", place.path);
        let image_key = present(&recipe.image).map(images::image_key);
        let preview_url = image_key
            .as_ref()
            .map(|k| format!("{page_url}/og.jpg?v={k}"));
        let description = present(&recipe.description)
            .map(|d| clip(d, 200))
            .unwrap_or_else(|| {
                let n = crate::model::count_items(&recipe.ingredients);
                match n {
                    0 => "A recipe shared from Crumb.".to_string(),
                    1 => "A recipe shared from Crumb: 1 ingredient.".to_string(),
                    n => format!("A recipe shared from Crumb: {n} ingredients."),
                }
            });
        Self {
            recipe,
            notes: present(&recipe.notes).filter(|_| place.include_notes),
            export_url: format!("{page_url}/crumb.json"),
            img_base: format!("{}/img", place.path),
            page_url,
            image_key,
            preview_url,
            description,
        }
    }

    fn head(&self) -> String {
        let r = self.recipe;
        let mut out = String::new();
        let mut meta = |attr: &str, name: &str, content: &str| {
            out.push_str(&format!(
                "<meta {attr}=\"{name}\" content=\"{}\">",
                escape(content)
            ));
        };
        meta("name", "description", &self.description);
        meta("property", "og:type", "article");
        meta("property", "og:site_name", "Crumb");
        meta("property", "og:title", &r.title);
        meta("property", "og:description", &self.description);
        meta("property", "og:url", &self.page_url);
        if let Some(preview) = &self.preview_url {
            meta("property", "og:image", preview);
            meta(
                "property",
                "og:image:width",
                &images::PREVIEW_SIZE.0.to_string(),
            );
            meta(
                "property",
                "og:image:height",
                &images::PREVIEW_SIZE.1.to_string(),
            );
            meta("property", "og:image:alt", &r.title);
            meta("name", "twitter:card", "summary_large_image");
        } else {
            meta("name", "twitter:card", "summary");
        }
        meta("name", "twitter:title", &r.title);
        meta("name", "twitter:description", &self.description);
        if let Some(preview) = &self.preview_url {
            meta("name", "twitter:image", preview);
        }
        out.push_str(&format!(
            "<link rel=\"alternate\" type=\"{}\" href=\"{}\">",
            crate::scraper::CRUMB_JSON_TYPE,
            escape(&self.export_url)
        ));
        if let Some(key) = &self.image_key {
            out.push_str(&format!(
                "<link rel=\"preload\" as=\"image\" fetchpriority=\"high\" imagesrcset=\"{b}/768?v={key} 768w, {b}/1200?v={key} 1200w\" imagesizes=\"(min-width: 1024px) 560px, 100vw\">",
                b = escape(&self.img_base),
            ));
        }
        out.push_str(&format!(
            "<script type=\"application/ld+json\">{}</script>",
            crate::web::inline_json(&self.json_ld())
        ));
        out
    }

    /// schema.org Recipe, for link previews and other recipe apps.
    fn json_ld(&self) -> Value {
        let r = self.recipe;
        let mut ld = Map::new();
        ld.insert("@context".into(), json!("https://schema.org"));
        ld.insert("@type".into(), json!("Recipe"));
        ld.insert("name".into(), json!(r.title));
        ld.insert("description".into(), json!(self.description));
        if let Some(preview) = &self.preview_url {
            ld.insert("image".into(), json!([preview]));
        }
        if let Some(author) = present(&r.author) {
            ld.insert("author".into(), json!({"@type": "Person", "name": author}));
        }
        for (key, value) in [
            ("prepTime", &r.prep_time),
            ("cookTime", &r.cook_time),
            ("totalTime", &r.total_time),
        ] {
            if let Some(v) = present(value) {
                // Written as it is when it isn't a time we can read
                ld.insert(
                    key.into(),
                    json!(iso_duration(v).unwrap_or_else(|| v.into())),
                );
            }
        }
        for (key, value) in [
            ("recipeYield", &r.recipe_yield),
            ("recipeCategory", &r.recipe_category),
            ("recipeCuisine", &r.recipe_cuisine),
        ] {
            if let Some(v) = present(value) {
                ld.insert(key.into(), json!(v));
            }
        }
        let flat: Vec<&String> = r.ingredients.iter().flat_map(|s| &s.items).collect();
        ld.insert("recipeIngredient".into(), json!(flat));
        let step = |text: &String| json!({"@type": "HowToStep", "text": text});
        let named = r.instructions.iter().any(|s| s.name.is_some());
        let steps: Vec<Value> = if named {
            r.instructions
                .iter()
                .flat_map(|s| match &s.name {
                    Some(name) => vec![json!({
                        "@type": "HowToSection",
                        "name": name,
                        "itemListElement": s.items.iter().map(step).collect::<Vec<_>>(),
                    })],
                    None => s.items.iter().map(step).collect(),
                })
                .collect()
        } else {
            r.instructions
                .iter()
                .flat_map(|s| s.items.iter().map(step))
                .collect()
        };
        ld.insert("recipeInstructions".into(), json!(steps));
        let nutrition = nutrition_entries(r);
        if !nutrition.is_empty() {
            let mut n = Map::new();
            n.insert("@type".into(), json!("NutritionInformation"));
            for (k, v) in nutrition {
                n.insert(k, json!(v));
            }
            ld.insert("nutrition".into(), Value::Object(n));
        }
        if let Some(source) = source_url(r) {
            ld.insert("url".into(), json!(source));
            ld.insert("isBasedOn".into(), json!(source));
        }
        Value::Object(ld)
    }

    fn photo(&self) -> String {
        let Some(key) = &self.image_key else {
            return String::new();
        };
        let b = escape(&self.img_base);
        format!(
            "<img class=\"share-photo\" src=\"{b}/768?v={key}\" srcset=\"{b}/768?v={key} 768w, {b}/1200?v={key} 1200w\" sizes=\"(min-width: 1024px) 560px, 100vw\" alt=\"{}\" fetchpriority=\"high\" decoding=\"async\">",
            escape(&self.recipe.title)
        )
    }

    fn intro(&self) -> String {
        let r = self.recipe;
        let mut out = String::new();
        let kicker: Vec<&str> = [present(&r.recipe_category), present(&r.recipe_cuisine)]
            .into_iter()
            .flatten()
            .collect();
        if !kicker.is_empty() {
            out.push_str(&format!(
                "<p class=\"kicker\">{}</p>",
                escape(&kicker.join(" · "))
            ));
        }
        out.push_str(&format!(
            "<h1 class=\"page-title share-title\">{}</h1>",
            escape(&r.title)
        ));
        if let Some(author) = present(&r.author) {
            out.push_str(&format!("<p class=\"share-by\">By {}</p>", escape(author)));
        }
        if let Some(d) = present(&r.description) {
            out.push_str(&format!("<p class=\"share-desc\">{}</p>", escape(d)));
        }
        let mut cells = String::new();
        for (name, label, value) in [
            ("timer", "Prep", &r.prep_time),
            ("flame", "Cook", &r.cook_time),
            ("snowflake", "Extra", &r.freeze_time),
            ("clock", "Total", &r.total_time),
        ] {
            if let Some(v) = present(value) {
                cells.push_str(&format!(
                    "<div><dt class=\"meta\">{}{label}</dt><dd>{}</dd></div>",
                    icon(name),
                    escape(v)
                ));
            }
        }
        if let Some(y) = present(&r.recipe_yield) {
            cells.push_str(&format!(
                "<div><dt class=\"meta\">{}Serves</dt><dd data-scale-raw=\"{e}\">{e}</dd></div>",
                icon("users"),
                e = escape(y)
            ));
        }
        if !cells.is_empty() {
            out.push_str(&format!("<dl class=\"card share-meta\">{cells}</dl>"));
        }
        out
    }

    fn ingredients(&self) -> String {
        let sections = &self.recipe.ingredients;
        if crate::model::count_items(sections) == 0 {
            return "<p class=\"meta\">No ingredients listed.</p>".into();
        }
        // Headings are what keep the groups for a scraper reading this page; an unnamed
        // first group among named ones gets a hidden one so its lines aren't dropped
        let named = sections.iter().any(|s| s.name.is_some());
        let mut out = String::from("<div class=\"share-ingredients recipe-ingredients\">");
        for s in sections.iter().filter(|s| !s.items.is_empty()) {
            out.push_str("<div class=\"share-group\">");
            match &s.name {
                Some(name) => out.push_str(&format!("<h3 class=\"kicker\">{}</h3>", escape(name))),
                None if named => out.push_str("<h3 class=\"kicker\" hidden>Ingredients</h3>"),
                None => {}
            }
            out.push_str("<ul class=\"list-card share-list\">");
            for item in &s.items {
                out.push_str(&format!(
                    "<li data-scale-raw=\"{e}\">{e}</li>",
                    e = escape(item)
                ));
            }
            out.push_str("</ul></div>");
        }
        out.push_str("</div>");
        out
    }

    fn method(&self) -> String {
        let sections = &self.recipe.instructions;
        let mut out = String::new();
        if crate::model::count_items(sections) == 0 {
            out.push_str("<p class=\"meta\">No steps listed.</p>");
        } else {
            out.push_str("<div class=\"share-steps\">");
            let mut n = 0;
            for s in sections.iter().filter(|s| !s.items.is_empty()) {
                out.push_str("<div class=\"share-group\">");
                if let Some(name) = &s.name {
                    out.push_str(&format!("<h3 class=\"kicker\">{}</h3>", escape(name)));
                }
                out.push_str(&format!(
                    "<ol class=\"share-step-list\" start=\"{}\">",
                    n + 1
                ));
                for step in &s.items {
                    n += 1;
                    out.push_str(&format!(
                        "<li><span class=\"share-num\" aria-hidden=\"true\">{n}</span><p>{}</p></li>",
                        escape(step)
                    ));
                }
                out.push_str("</ol></div>");
            }
            out.push_str("</div>");
        }
        if let Some(notes) = self.notes {
            out.push_str(&format!(
                "<div class=\"card share-notes\"><h2>{}Notes</h2><p class=\"hand\">{}</p></div>",
                icon("note"),
                escape(notes)
            ));
        }
        let nutrition = nutrition_entries(self.recipe);
        if !nutrition.is_empty() {
            out.push_str(
                "<div class=\"share-nutrition\"><h2 class=\"section-title\">Nutrition</h2><dl>",
            );
            for (key, value) in nutrition {
                let label = NUTRITION_LABELS
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, l)| l.to_string())
                    .unwrap_or_else(|| key.trim_end_matches("Content").to_string());
                out.push_str(&format!(
                    "<div class=\"card\"><dt class=\"meta\">{}</dt><dd>{}</dd></div>",
                    escape(&label),
                    escape(&value)
                ));
            }
            out.push_str("</dl></div>");
        }
        out
    }

    fn source(&self) -> String {
        let Some(url) = source_url(self.recipe) else {
            return String::new();
        };
        let host = host_of(url).unwrap_or_else(|| url.to_string());
        format!(
            "<p class=\"share-source\">Original recipe: <a class=\"link\" href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">{}{}</a></p>",
            escape(url),
            escape(&host),
            icon("external")
        )
    }

    /// For the islands: the Save and Download buttons.
    fn page_data(&self) -> Value {
        json!({
            "share": {
                "title": self.recipe.title,
                "url": self.page_url,
                "exportUrl": self.export_url,
            }
        })
    }
}

/// Swaps `marker` for `html` once; without the marker, nothing changes.
fn fill(page: &mut String, marker: &str, html: &str) {
    if let Some(at) = page.find(marker) {
        page.replace_range(at..at + marker.len(), html);
    }
}

/// The template with its title, head tags and page data set.
fn start_page(template: &str, title: &str, head: &str, data: &Value) -> String {
    let mut page = template.to_string();
    let title = format!("<title>{} · Crumb</title>", escape(title));
    match (page.find("<title>"), page.find("</title>")) {
        (Some(a), Some(b)) if a < b => page.replace_range(a..b + "</title>".len(), &title),
        _ => fill(&mut page, "<head>", &format!("<head>{title}")),
    }
    match page.find("</head>") {
        Some(at) => page.insert_str(at, head),
        None => page.insert_str(0, head),
    }
    let data = crate::web::MARKER.replace("null", &crate::web::inline_json(data));
    fill(&mut page, crate::web::MARKER, &data);
    page
}

/// The template with this recipe in it.
pub fn render(template: &str, recipe: &Recipe, place: &Place, origin: &str) -> String {
    let view = View::new(recipe, place, origin);
    let mut page = start_page(template, &recipe.title, &view.head(), &view.page_data());
    let back = place
        .book
        .as_ref()
        .map_or_else(String::new, |(name, path)| {
            format!(
                "<a class=\"btn btn-ghost share-back no-print\" href=\"{}\">{}{}</a>",
                escape(path),
                icon("back"),
                escape(name)
            )
        });
    fill(&mut page, "<!--share:back-->", &back);
    fill(&mut page, "<!--share:photo-->", &view.photo());
    fill(&mut page, "<!--share:intro-->", &view.intro());
    fill(&mut page, "<!--share:ingredients-->", &view.ingredients());
    fill(&mut page, "<!--share:method-->", &view.method());
    fill(&mut page, "<!--share:source-->", &view.source());
    page
}

/// The CSP for a template, worked out once per template text (templates are cached in
/// release builds and re-read in debug ones, so a changed template gets a fresh policy).
/// Holds the few templates there are (recipe, cookbook) and a spare.
fn template_csp(template: &str) -> String {
    static CACHE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, csp)) = cache.iter().find(|(t, _)| t == template) {
        return csp.clone();
    }
    let csp = content_security_policy(template);
    if cache.len() >= 3 {
        cache.remove(0);
    }
    cache.push((template.to_string(), csp.clone()));
    csp
}

/// A shared cookbook's page: the book's spine, name and recipe cards, each linking to that
/// recipe's page under the book's link.
pub fn render_book(
    template: &str,
    book: &Cookbook,
    list: &[RecipeSummary],
    share: &Share,
    origin: &str,
) -> String {
    let path = format!("/s/{}", share.token);
    let page_url = format!("{origin}{path}");
    let export_url = format!("{page_url}/crumb.json");
    let count = match list.len() {
        0 => "No recipes yet".to_string(),
        1 => "1 recipe".to_string(),
        n => format!("{n} recipes"),
    };
    let description = present(&book.description)
        .map(|d| clip(d, 200))
        .unwrap_or_else(|| match list.len() {
            0 => "A cookbook shared from Crumb.".to_string(),
            _ => format!("A cookbook shared from Crumb: {}.", count.to_lowercase()),
        });
    let preview = cover(list)
        .and_then(|r| r.image.as_deref())
        .map(|image| format!("{page_url}/og.jpg?v={}", images::image_key(image)));

    let mut head = String::new();
    let mut meta = |attr: &str, name: &str, content: &str| {
        head.push_str(&format!(
            "<meta {attr}=\"{name}\" content=\"{}\">",
            escape(content)
        ));
    };
    meta("name", "description", &description);
    meta("property", "og:type", "website");
    meta("property", "og:site_name", "Crumb");
    meta("property", "og:title", &book.name);
    meta("property", "og:description", &description);
    meta("property", "og:url", &page_url);
    match &preview {
        Some(preview) => {
            meta("property", "og:image", preview);
            meta(
                "property",
                "og:image:width",
                &images::PREVIEW_SIZE.0.to_string(),
            );
            meta(
                "property",
                "og:image:height",
                &images::PREVIEW_SIZE.1.to_string(),
            );
            meta("property", "og:image:alt", &book.name);
            meta("name", "twitter:card", "summary_large_image");
            meta("name", "twitter:image", preview);
        }
        None => meta("name", "twitter:card", "summary"),
    }
    meta("name", "twitter:title", &book.name);
    meta("name", "twitter:description", &description);
    head.push_str(&format!(
        "<link rel=\"alternate\" type=\"{}\" href=\"{}\">",
        crate::scraper::CRUMB_JSON_TYPE,
        escape(&export_url)
    ));
    let data = json!({
        "share": {
            "kind": "cookbook",
            "title": book.name,
            "url": page_url,
            "exportUrl": export_url,
        }
    });
    let mut page = start_page(template, &book.name, &head, &data);

    let color = book
        .color
        .as_deref()
        .and_then(crate::model::book_color)
        .unwrap_or("tile");
    let name = escape(&book.name);
    let mut intro = format!(
        "<div class=\"share-book-head\"><div class=\"share-book-spine\" aria-hidden=\"true\">\
         <div class=\"share-spine\" data-color=\"{color}\"><span>{name}</span></div>\
         <div class=\"share-plank\"></div></div><div class=\"share-book-name\">\
         <p class=\"kicker\">Cookbook</p><h1 class=\"page-title share-title\">{name}</h1>\
         <p class=\"meta\">{count}</p></div></div>"
    );
    if let Some(d) = present(&book.description) {
        intro.push_str(&format!("<p class=\"share-desc\">{}</p>", escape(d)));
    }
    fill(&mut page, "<!--share:book-->", &intro);

    let cards = if list.is_empty() {
        "<p class=\"card share-empty meta\">Nothing in this cookbook yet. Recipes added to it show up here.</p>".to_string()
    } else {
        let mut out = String::from("<ul class=\"share-cards\">");
        for (i, r) in list.iter().enumerate() {
            let href = format!("{path}/{}", r.id);
            let photo = match r.image.as_deref().filter(|i| !i.trim().is_empty()) {
                Some(image) => {
                    let key = images::image_key(image);
                    let src = |w: u32| format!("{href}/img/{w}?v={key}");
                    format!(
                        "<img class=\"share-card-photo\" src=\"{}\" srcset=\"{} 320w, {} 480w, {} 768w\" sizes=\"(min-width: 1280px) 14rem, (min-width: 1024px) 22vw, (min-width: 640px) 31vw, 46vw\" alt=\"\" loading=\"{}\" decoding=\"async\">",
                        escape(&src(320)),
                        escape(&src(320)),
                        escape(&src(480)),
                        escape(&src(768)),
                        if i < 4 { "eager" } else { "lazy" },
                    )
                }
                None => format!(
                    "<div class=\"share-card-photo photo-empty\">{}</div>",
                    icon("pot")
                ),
            };
            let sub: Vec<&str> = [
                present(&r.recipe_category),
                present(&r.recipe_cuisine),
                present(&r.total_time),
            ]
            .into_iter()
            .flatten()
            .collect();
            let sub = if sub.is_empty() {
                String::new()
            } else {
                format!(
                    "<p class=\"meta share-card-meta\">{}</p>",
                    escape(&sub.join(" · "))
                )
            };
            out.push_str(&format!(
                "<li><a class=\"share-card\" href=\"{}\">{photo}<h2 class=\"share-card-title\">{}</h2>{sub}</a></li>",
                escape(&href),
                escape(&r.title),
            ));
        }
        out.push_str("</ul>");
        out
    };
    fill(&mut page, "<!--share:recipes-->", &cards);
    page
}

static SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script\b([^>]*)>(.*?)</script>").unwrap());
static SCRIPT_TYPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\btype\s*=\s*["']?([^"'\s>]+)"#).unwrap());

/// A page's CSP: scripts only from this origin, plus each inline script the template
/// runs (the theme boot, Astro's island loader) by its hash. Data blocks (page data,
/// JSON-LD) don't run, so they need none.
pub fn content_security_policy(page: &str) -> String {
    let mut hashes = Vec::new();
    for c in SCRIPT.captures_iter(page) {
        let attrs = &c[1];
        if attrs.to_ascii_lowercase().contains("src=") {
            continue;
        }
        let runs = SCRIPT_TYPE.captures(attrs).is_none_or(|t| {
            matches!(
                t[1].to_ascii_lowercase().as_str(),
                "module" | "text/javascript" | "application/javascript"
            )
        });
        if runs {
            let digest = sha2::Sha256::digest(c[2].as_bytes());
            let hash = format!(
                "'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(digest)
            );
            if !hashes.contains(&hash) {
                hashes.push(hash);
            }
        }
    }
    let scripts = std::iter::once("'self'".to_string())
        .chain(hashes)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "default-src 'none'; script-src {scripts}; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; \
         form-action 'self'; frame-ancestors 'none'"
    )
}

// ─── Saving another Crumb's share ───────────────────────────────────────────

/// The client for exports: redirects are followed only within the export's own origin
/// (scheme, host and port), so a share page can't bounce the fetch to another address.
static EXPORT_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        let same = attempt
            .previous()
            .first()
            .is_some_and(|first| crate::scraper::same_origin(attempt.url(), first));
        if same && attempt.previous().len() < 5 {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(policy)
        .build()
        .expect("HTTP client")
});

/// What another Crumb's share export holds.
pub enum Export {
    Recipe(Box<RecipeFields>),
    Book(SharedBook),
}

/// A shared cookbook's export: its name, colour and recipes (each with its own share link).
pub struct SharedBook {
    pub name: String,
    pub color: Option<String>,
    pub recipes: Vec<ImportedRecipe>,
}

/// Fetches another Crumb's export (`crumb.json` from a share page) and reads it: one recipe,
/// or a cookbook of them. None when it can't be had or isn't a Crumb export; the caller falls
/// back to the page. Log lines name only the host: the URL holds that share's token.
pub async fn fetch_export(url: &str) -> Option<Export> {
    let host = crate::telemetry::host_of(url);
    let res = EXPORT_CLIENT
        .get(url)
        .timeout(EXPORT_TIMEOUT)
        .header(header::ACCEPT, "application/json")
        .header(header::USER_AGENT, crate::scraper::USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            let e = e.without_url();
            tracing::info!("[share] {host}: export fetch failed: {e}")
        })
        .ok()?;
    if !res.status().is_success()
        || res
            .content_length()
            .is_some_and(|n| n > MAX_EXPORT_BYTES as u64)
    {
        tracing::info!("[share] {host}: export answered {}", res.status());
        return None;
    }
    let mut res = res;
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.ok()? {
        if body.len() + chunk.len() > MAX_EXPORT_BYTES {
            tracing::info!("[share] {host}: export too large");
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    let doc: Value = serde_json::from_slice(&body).ok()?;
    let export = read_export(&doc)?;
    tracing::info!("[share] {host}: saved from a Crumb share");
    Some(export)
}

/// A Crumb share export, read. A cookbook's says `"kind": "cookbook"` and names the book;
/// a recipe's is one recipe. Cook logs and other cookbooks in it are never taken.
pub fn read_export(doc: &Value) -> Option<Export> {
    if doc.get("format").and_then(Value::as_str) != Some("crumb") {
        return None;
    }
    let found = crate::importers::from_json_value(doc)
        .into_iter()
        .filter(|r| r.restored);
    if doc.get("kind").and_then(Value::as_str) == Some("cookbook") {
        let info = doc.get("cookbooks").and_then(|b| b.get(0));
        let text = |key: &str| {
            info.and_then(|b| b.get(key))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        return Some(Export::Book(SharedBook {
            name: text("name")?,
            color: text("color"),
            recipes: found.take(recipes::MAX_BOOK_RECIPES).collect(),
        }));
    }
    let mut found = found;
    found.next().map(|r| Export::Recipe(Box::new(r.fields)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_become_iso_durations() {
        assert_eq!(iso_duration("1h 10m").as_deref(), Some("PT1H10M"));
        assert_eq!(iso_duration("45 mins").as_deref(), Some("PT45M"));
        assert_eq!(
            iso_duration("1 hour 30 minutes").as_deref(),
            Some("PT1H30M")
        );
        assert_eq!(iso_duration("2 hrs").as_deref(), Some("PT2H"));
        assert_eq!(iso_duration("1 day").as_deref(), Some("PT24H"));
        assert_eq!(iso_duration("1.5 hours").as_deref(), Some("PT1H30M"));
        assert_eq!(iso_duration("PT20M").as_deref(), Some("PT20M"));
        assert_eq!(iso_duration("overnight"), None);
        assert_eq!(iso_duration("10-15 minutes"), None);
        assert_eq!(iso_duration("0m"), None);
    }

    #[test]
    fn escapes_markup_and_quotes() {
        assert_eq!(
            escape(r#"<a href="x">Tom's & Co</a>"#),
            "&lt;a href=&quot;x&quot;&gt;Tom&#39;s &amp; Co&lt;/a&gt;"
        );
        assert_eq!(clip("one two three four", 9), "one two…");
        assert_eq!(clip("short", 9), "short");
    }

    #[test]
    fn misses_are_limited_per_address() {
        let m = Misses::default();
        for _ in 0..MISS_LIMIT - 1 {
            m.miss("a");
        }
        assert!(!m.blocked("a"));
        m.miss("a");
        assert!(m.blocked("a"));
        assert!(!m.blocked("b"));
        for i in 0..MAX_TRACKED + 10 {
            m.miss(&format!("ip{i}"));
        }
        assert!(m.locked().len() <= MAX_TRACKED);
    }

    #[test]
    fn a_flood_of_new_addresses_evicts_the_oldest_not_everyone() {
        let m = Misses::default();
        // A stale window is dropped first
        m.locked().insert(
            "stale".into(),
            (Instant::now() - MISS_WINDOW - Duration::from_secs(1), 3),
        );
        for _ in 0..MISS_LIMIT {
            m.miss("victim");
        }
        for i in 0..MAX_TRACKED - 2 {
            m.miss(&format!("ip{i}"));
        }
        assert_eq!(m.locked().len(), MAX_TRACKED);
        m.miss("new-1");
        assert!(!m.locked().contains_key("stale"));
        assert!(m.blocked("victim"));
        // Full of live windows: only the oldest goes, and the blocked address stays blocked
        // for as long as it isn't the oldest
        m.locked().insert(
            "oldest".into(),
            (Instant::now() - Duration::from_secs(30), 1),
        );
        m.locked().remove("ip0");
        m.miss("new-2");
        assert!(!m.locked().contains_key("oldest"));
        assert!(m.locked().contains_key("new-1") && m.locked().contains_key("new-2"));
        assert!(m.blocked("victim"));
        assert_eq!(m.locked().len(), MAX_TRACKED);
    }

    #[test]
    fn the_client_address_is_the_one_the_proxy_saw() {
        let peer = Some("10.0.0.9".parse().unwrap());
        let headers = |pairs: &[(&'static str, &str)]| {
            let mut h = HeaderMap::new();
            for (k, v) in pairs {
                h.insert(*k, HeaderValue::from_str(v).unwrap());
            }
            h
        };
        let spoofed = headers(&[("x-forwarded-for", "6.6.6.6, 203.0.113.7")]);
        assert_eq!(client_ip_from(&spoofed, peer, true), "203.0.113.7");
        let both = headers(&[
            ("x-forwarded-for", "6.6.6.6, 203.0.113.7"),
            ("x-real-ip", "198.51.100.2"),
        ]);
        assert_eq!(client_ip_from(&both, peer, true), "198.51.100.2");
        let junk = headers(&[("x-real-ip", "nope"), ("x-forwarded-for", "1.1.1.1, bad")]);
        assert_eq!(client_ip_from(&junk, peer, true), "10.0.0.9");
        // Not behind the proxy: headers are ignored
        assert_eq!(client_ip_from(&both, peer, false), "10.0.0.9");
        assert_eq!(client_ip_from(&HeaderMap::new(), None, true), "unknown");
    }

    #[test]
    fn csp_hashes_only_scripts_that_run() {
        let page = r#"<script>a()</script><script type="module">b()</script>
            <script type="application/json" id="page-data">{"x":1}</script>
            <script type="application/ld+json">{}</script><script type="module" src="/x.js"></script>"#;
        let csp = content_security_policy(page);
        let hash = |s: &str| {
            format!(
                "'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(s))
            )
        };
        assert!(csp.contains(&format!(
            "script-src 'self' {} {};",
            hash("a()"),
            hash("b()")
        )));
        assert!(!csp.contains(&hash(r#"{"x":1}"#)));
        assert!(csp.starts_with("default-src 'none';"));
    }

    #[test]
    fn tokens_are_checked_before_any_lookup() {
        assert!(plausible_token(&crate::auth::random_token(TOKEN_BYTES)));
        assert!(!plausible_token("short"));
        assert!(!plausible_token("abcdefghijklmnop/.."));
        assert!(!plausible_token(&"a".repeat(65)));
    }
}
