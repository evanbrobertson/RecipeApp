//! Minimal OAuth 2.1 authorization server (dynamic client registration + PKCE)
//! so the app can be added to Claude as a custom connector.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Json, Router, routing};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::AppState;
use crate::auth::{check_password, is_logged_in, login_cookie, random_token, sha256_hex};
use crate::error::AppError;
use crate::model::now_secs;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

const MINUTE: i64 = 60;
const DAY: i64 = 24 * 60 * MINUTE;

#[derive(Clone, Copy)]
enum Kind {
    Code,
    Access,
    Refresh,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Access => "access",
            Kind::Refresh => "refresh",
        }
    }

    fn lifetime(self) -> i64 {
        match self {
            Kind::Code => 5 * MINUTE,
            Kind::Access => 30 * DAY,
            Kind::Refresh => 365 * DAY,
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            routing::get(auth_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            routing::get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            routing::get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp/chatgpt",
            routing::get(protected_resource_metadata_chatgpt),
        )
        .route(
            "/.well-known/openai-apps-challenge",
            routing::get(apps_challenge),
        )
        .route(
            "/oauth/authorize",
            routing::get(authorize_get).post(authorize_post),
        )
        .route(
            "/oauth/register",
            routing::post(register).options(preflight),
        )
        .route("/oauth/token", routing::post(token).options(preflight))
        .route("/oauth/revoke", routing::post(revoke).options(preflight))
        .route("/api/connections", routing::get(connections))
        .route("/api/connections/{client}", routing::delete(disconnect))
}

fn cors(mut res: Response) -> Response {
    res.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    res
}

async fn preflight() -> Response {
    let mut res = cors(StatusCode::NO_CONTENT.into_response());
    let h = res.headers_mut();
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, OPTIONS"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type, authorization"),
    );
    res
}

pub fn auth_server_metadata_json(origin: &str) -> Value {
    json!({
        "issuer": origin,
        "authorization_endpoint": format!("{origin}/oauth/authorize"),
        "token_endpoint": format!("{origin}/oauth/token"),
        "registration_endpoint": format!("{origin}/oauth/register"),
        "revocation_endpoint": format!("{origin}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["recipes"],
        "authorization_response_iss_parameter_supported": true,
    })
}

async fn auth_server_metadata(State(state): State<AppState>, headers: HeaderMap) -> Response {
    cors(
        Json(auth_server_metadata_json(
            &state.config.public_origin(&headers),
        ))
        .into_response(),
    )
}

async fn protected_resource_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    resource_metadata(&state, &headers, "/mcp")
}

async fn protected_resource_metadata_chatgpt(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    resource_metadata(&state, &headers, "/mcp/chatgpt")
}

/// What OpenAI asks for to verify the domain before a plugin can be submitted.
async fn apps_challenge(State(state): State<AppState>) -> Response {
    match state.config.openai_apps_challenge.as_deref() {
        Some(token) => (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            token.to_string(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn resource_metadata(state: &AppState, headers: &HeaderMap, path: &str) -> Response {
    let origin = state.config.public_origin(headers);
    cors(
        Json(json!({
            "resource": format!("{origin}{path}"),
            "authorization_servers": [origin],
            "scopes_supported": ["recipes"],
            "bearer_methods_supported": ["header"],
            "resource_name": "Crumb",
        }))
        .into_response(),
    )
}

pub fn is_allowed_redirect_uri(uri: &str) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => true,
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    }
}

/// h3's readBody: form-encoded or JSON depending on the content type.
fn read_body(headers: &HeaderMap, body: &Bytes) -> Option<Map<String, Value>> {
    let ctype = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let form = || -> Option<Map<String, Value>> {
        let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(body).ok()?;
        Some(
            pairs
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect(),
        )
    };
    if ctype.contains("application/x-www-form-urlencoded") {
        return form();
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(m)) => Some(m),
        Ok(_) => None,
        Err(_) if !ctype.contains("json") => form(),
        Err(_) => None,
    }
}

fn get_str<'a>(m: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(Value::as_str)
}

/// Registrations one client address may make per hour, and all addresses together.
const REGISTRATIONS_PER_IP: u32 = 10;
const REGISTRATIONS_TOTAL: u32 = 120;
/// Registered clients kept. Registration is open by design (Claude registers itself), so
/// clients nobody has connected are dropped after 90 days, and sooner when the table is full.
const MAX_CLIENTS: i64 = 1000;
const UNUSED_CLIENT_TTL: i64 = 90 * DAY;

/// Drops clients that never got a token. Returns how many clients are left.
fn prune_clients(conn: &rusqlite::Connection, older_than: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "DELETE FROM oauth_clients WHERE created_at < ?1
           AND id NOT IN (SELECT client_id FROM oauth_tokens)",
        [older_than],
    )?;
    conn.query_row("SELECT count(*) FROM oauth_clients", [], |r| r.get(0))
}

async fn register(
    State(state): State<AppState>,
    axum::Extension(ip): axum::Extension<crate::throttle::ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let hour = std::time::Duration::from_secs(60 * MINUTE as u64);
    if !state.rates.hit(
        &format!("oauth-register:{}", ip.0),
        REGISTRATIONS_PER_IP,
        hour,
    ) || !state
        .rates
        .hit("oauth-register:all", REGISTRATIONS_TOTAL, hour)
    {
        return cors(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": "temporarily_unavailable",
                    "error_description": "Too many registrations. Try again later.",
                })),
            )
                .into_response(),
        );
    }
    let invalid = || {
        cors(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_redirect_uri",
                    "error_description": "redirect_uris must be https (or http://localhost) URLs",
                })),
            )
                .into_response(),
        )
    };
    let Some(m) = read_body(&headers, &body) else {
        return invalid();
    };
    let client_name = match m.get("client_name") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.chars().count() <= 200 => Some(s.clone()),
        _ => return invalid(),
    };
    let Some(Value::Array(uris)) = m.get("redirect_uris") else {
        return invalid();
    };
    let uris: Option<Vec<String>> = uris.iter().map(|u| u.as_str().map(String::from)).collect();
    let Some(uris) = uris.filter(|u| (1..=10).contains(&u.len())) else {
        return invalid();
    };
    if !uris.iter().all(|u| is_allowed_redirect_uri(u)) {
        return invalid();
    }

    let id = random_token(16);
    let saved = {
        let conn = state.db.lock();
        let now = now_secs();
        let mut count = prune_clients(&conn, now - UNUSED_CLIENT_TTL).unwrap_or(0);
        if count >= MAX_CLIENTS {
            // Full: make room from the ones nobody has used yet, however new
            count = prune_clients(&conn, now - MINUTE).unwrap_or(count);
        }
        if count >= MAX_CLIENTS {
            drop(conn);
            return cors(
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": "temporarily_unavailable",
                        "error_description": "Too many apps are registered. Try again later.",
                    })),
                )
                    .into_response(),
            );
        }
        conn.execute(
            "INSERT INTO oauth_clients (id, name, redirect_uris, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                id,
                client_name,
                serde_json::to_string(&uris).unwrap_or_default(),
                now
            ],
        )
    };
    if let Err(err) = saved {
        return AppError::internal(err).into_response();
    }
    let mut out = json!({
        "client_id": id,
        "client_id_issued_at": now_secs(),
        "redirect_uris": uris,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    if let Some(name) = client_name {
        out["client_name"] = json!(name);
    }
    cors((StatusCode::CREATED, Json(out)).into_response())
}

struct Client {
    id: String,
    name: Option<String>,
    redirect_uris: Vec<String>,
}

fn get_client(state: &AppState, id: &str) -> Option<Client> {
    state
        .db
        .lock()
        .query_row(
            "SELECT id, name, redirect_uris FROM oauth_clients WHERE id = ?1",
            [id],
            |r| {
                let uris: String = r.get(2)?;
                Ok(Client {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    redirect_uris: serde_json::from_str(&uris).unwrap_or_default(),
                })
            },
        )
        .optional()
        .ok()
        .flatten()
}

/// Whose connector a token is, with accounts: the person who approved it and the household
/// it works on. None with one password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner {
    pub user_id: i64,
    pub household_id: crate::households::HouseholdId,
}

/// What a token is for: who approved it, when, and which endpoint it opens.
#[derive(Clone, Copy)]
struct Grant<'a> {
    owner: Option<Owner>,
    granted: i64,
    resource: Option<&'a str>,
}

/// Issues a token. `granted` is when the person approved the connection (now, for a code),
/// carried from token to token so the connected apps list can say since when.
fn issue(
    state: &AppState,
    kind: Kind,
    client_id: &str,
    challenge: Option<&str>,
    redirect: Option<&str>,
    grant: Grant,
) -> Result<String, AppError> {
    let token = random_token(32);
    let now = now_secs();
    state.db.lock().execute(
        "INSERT INTO oauth_tokens (hash, kind, client_id, code_challenge, redirect_uri, expires_at,
           created_at, user_id, household_id, granted_at, resource)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            sha256_hex(&token),
            kind.as_str(),
            client_id,
            challenge,
            redirect,
            now + kind.lifetime(),
            now,
            grant.owner.map(|o| o.user_id),
            grant.owner.map(|o| o.household_id),
            grant.granted,
            grant.resource
        ],
    )?;
    Ok(token)
}

/// With accounts set up for the first time, the connector tokens the one-password box
/// issued become its owner's (household 1), so nobody has to reconnect.
pub fn adopt_tokens(state: &AppState, owner: i64) -> Result<(), AppError> {
    state.households.home().db.lock().execute(
        "UPDATE oauth_tokens SET user_id = ?1, household_id = ?2 WHERE household_id IS NULL",
        params![owner, crate::households::HOME],
    )?;
    Ok(())
}

fn owner_of(user_id: Option<i64>, household_id: Option<i64>) -> Option<Owner> {
    Some(Owner {
        user_id: user_id?,
        household_id: household_id?,
    })
}

struct TokenRow {
    hash: String,
    client_id: String,
    code_challenge: Option<String>,
    redirect_uri: Option<String>,
    owner: Option<Owner>,
    granted: i64,
    resource: Option<String>,
}

/// Looks up an unexpired token of the given kind and deletes it (single use).
fn consume(state: &AppState, kind: Kind, token: &str) -> Option<TokenRow> {
    let conn = state.db.lock();
    let row = conn
        .query_row(
            "SELECT hash, client_id, code_challenge, redirect_uri, user_id, household_id,
                    coalesce(granted_at, created_at), resource
             FROM oauth_tokens WHERE hash = ?1 AND kind = ?2 AND expires_at > ?3",
            params![sha256_hex(token), kind.as_str(), now_secs()],
            |r| {
                Ok(TokenRow {
                    hash: r.get(0)?,
                    client_id: r.get(1)?,
                    code_challenge: r.get(2)?,
                    redirect_uri: r.get(3)?,
                    owner: owner_of(r.get(4)?, r.get(5)?),
                    granted: r.get(6)?,
                    resource: r.get(7)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()?;
    let _ = conn.execute("DELETE FROM oauth_tokens WHERE hash = ?1", [&row.hash]);
    Some(row)
}

/// The household an MCP request's bearer token works on, or None when it has no valid
/// token. Without a password (and without accounts) every request works on the one box.
/// With accounts, a token must belong to someone who is still in that household.
pub async fn access_household(
    state: &AppState,
    headers: &HeaderMap,
    endpoint: &str,
) -> Option<crate::households::HouseholdId> {
    if !state.config.auth_enabled() {
        return Some(crate::households::HOME);
    }
    let header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (scheme, token) = header.split_once(char::is_whitespace)?;
    let token = token.trim();
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return None;
    }
    let found: Option<(Option<i64>, Option<i64>, Option<String>)> = state
        .households
        .home()
        .db
        .lock()
        .query_row(
            "SELECT user_id, household_id, resource FROM oauth_tokens
             WHERE hash = ?1 AND kind = 'access' AND expires_at > ?2",
            params![sha256_hex(token), now_secs()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();
    let (user, household, resource) = found?;
    // A token works on the endpoint it was issued for; those from before tokens were bound
    // (no resource) are Claude's, on /mcp
    if resource.as_deref().unwrap_or("/mcp") != endpoint {
        return None;
    }
    let Some(accounts) = &state.accounts else {
        return Some(crate::households::HOME);
    };
    let owner = owner_of(user, household)?;
    let member = match &state.hosted {
        Some(hosted) => {
            hosted
                .is_member(accounts, owner.user_id, owner.household_id)
                .await
        }
        None => accounts.is_member(owner.user_id, owner.household_id),
    };
    member.unwrap_or(false).then_some(owner.household_id)
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(json!({"error": error, "error_description": description})),
    )
        .into_response()
}

fn token_response(state: &AppState, client_id: &str, grant: Grant) -> Result<Value, AppError> {
    // Opportunistically purge expired rows
    state.db.lock().execute(
        "DELETE FROM oauth_tokens WHERE expires_at < ?1",
        [now_secs()],
    )?;
    Ok(json!({
        "access_token": issue(state, Kind::Access, client_id, None, None, grant)?,
        "token_type": "Bearer",
        "expires_in": Kind::Access.lifetime(),
        "refresh_token": issue(state, Kind::Refresh, client_id, None, None, grant)?,
        "scope": "recipes",
    }))
}

async fn token(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let m = read_body(&headers, &body).unwrap_or_default();
    let bad = StatusCode::BAD_REQUEST;
    let result = match get_str(&m, "grant_type") {
        Some("authorization_code") => {
            let (Some(code), Some(verifier)) = (get_str(&m, "code"), get_str(&m, "code_verifier"))
            else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_request",
                    "Missing code or code_verifier",
                )));
            };
            let Some(row) = consume(&state, Kind::Code, code) else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Authorization code is invalid or expired",
                )));
            };
            if get_str(&m, "client_id").is_some_and(|c| !c.is_empty() && c != row.client_id) {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Code was issued to another client",
                )));
            }
            if get_str(&m, "redirect_uri")
                .is_some_and(|r| !r.is_empty() && Some(r) != row.redirect_uri.as_deref())
            {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "redirect_uri does not match",
                )));
            }
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            if Some(challenge.as_str()) != row.code_challenge.as_deref() {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "PKCE verification failed",
                )));
            }
            token_response(
                &state,
                &row.client_id,
                Grant {
                    owner: row.owner,
                    granted: row.granted,
                    resource: row.resource.as_deref(),
                },
            )
        }
        Some("refresh_token") => {
            let Some(refresh) = get_str(&m, "refresh_token") else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_request",
                    "Missing refresh_token",
                )));
            };
            let Some(row) = consume(&state, Kind::Refresh, refresh) else {
                return no_store(cors(oauth_error(
                    bad,
                    "invalid_grant",
                    "Refresh token is invalid or expired",
                )));
            };
            token_response(
                &state,
                &row.client_id,
                Grant {
                    owner: row.owner,
                    granted: row.granted,
                    resource: row.resource.as_deref(),
                },
            )
        }
        _ => {
            return no_store(cors(
                (bad, Json(json!({"error": "unsupported_grant_type"}))).into_response(),
            ));
        }
    };
    match result {
        Ok(v) => no_store(cors(Json(v).into_response())),
        Err(err) => cors(err.into_response()),
    }
}

fn no_store(mut res: Response) -> Response {
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn revoke(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let m = read_body(&headers, &body).unwrap_or_default();
    if let Some(token) = get_str(&m, "token") {
        let _ = state.db.lock().execute(
            "DELETE FROM oauth_tokens WHERE hash = ?1",
            [sha256_hex(token)],
        );
    }
    cors(Json(json!({})).into_response())
}

// ─── Connected apps ─────────────────────────────────────────────────────────

/// Which connectors a request may see and disconnect: with accounts, the signed-in
/// person's (`user_id = ?`); with one password, the box's (no owner). None: signed out.
fn whose(state: &AppState, signed: Option<&crate::auth::SignedIn>) -> Option<Option<i64>> {
    match (&state.accounts, signed) {
        (None, _) => Some(None),
        (Some(_), Some(s)) => Some(Some(s.0.user_id)),
        (Some(_), None) => None,
    }
}

/// The apps (Claude, mostly) connected to Crumb, one row per app and household: `[{id,
/// name, household: {id, name} | null, connectedAt, renewedAt}]`, newest first.
async fn connections(
    State(state): State<AppState>,
    signed: Option<axum::Extension<crate::auth::SignedIn>>,
) -> Result<Json<Value>, AppError> {
    let Some(user) = whose(&state, signed.as_deref()) else {
        return Err(AppError::new(401, "Not signed in"));
    };
    Ok(Json(json!(connected_apps(&state, user)?)))
}

/// The apps connected for a person (None: the one-password box), as [`connections`] lists them.
pub fn connected_apps(state: &AppState, user: Option<i64>) -> Result<Vec<Value>, AppError> {
    type Row = (String, Option<String>, Option<i64>, i64, i64);
    let rows: Vec<Row> = {
        let conn = state.households.home().db.lock();
        let mut stmt = conn.prepare(
            "SELECT t.client_id, c.name, t.household_id,
                    min(coalesce(t.granted_at, t.created_at)), max(t.created_at)
             FROM oauth_tokens t LEFT JOIN oauth_clients c ON c.id = t.client_id
             WHERE t.kind IN ('access', 'refresh') AND t.expires_at > ?1
               AND t.user_id IS ?2
             GROUP BY t.client_id, t.household_id
             ORDER BY 4 DESC, t.client_id",
        )?;
        stmt.query_map(params![now_secs(), user], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    Ok(rows
        .into_iter()
        .map(|(id, name, household, connected, renewed)| {
            let household = household.map(|h| {
                let name = state
                    .accounts
                    .as_ref()
                    .and_then(|a| a.household_name(h).ok().flatten());
                json!({"id": h, "name": name})
            });
            json!({
                "id": id,
                "name": name.filter(|n| !n.is_empty()).unwrap_or_else(|| "An app".into()),
                "household": household,
                "connectedAt": connected,
                "renewedAt": renewed,
            })
        })
        .collect())
}

/// Disconnects an app: every token it holds for this person (or, with one password, the
/// box) stops working at once. It has to be approved again to reconnect.
async fn disconnect(
    State(state): State<AppState>,
    signed: Option<axum::Extension<crate::auth::SignedIn>>,
    axum::extract::Path(client): axum::extract::Path<String>,
) -> Result<Json<Value>, AppError> {
    let Some(user) = whose(&state, signed.as_deref()) else {
        return Err(AppError::new(401, "Not signed in"));
    };
    let n = state.db.lock().execute(
        "DELETE FROM oauth_tokens WHERE client_id = ?1 AND user_id IS ?2",
        params![client, user],
    )?;
    if n == 0 {
        return Err(AppError::not_found("That app isn't connected"));
    }
    Ok(Json(json!({"ok": true})))
}

/// Deletes every connector token a person approved (their account is going).
pub fn forget_user(state: &AppState, user: i64) -> Result<(), AppError> {
    state
        .households
        .home()
        .db
        .lock()
        .execute("DELETE FROM oauth_tokens WHERE user_id = ?1", [user])?;
    Ok(())
}

/// Deletes the connector tokens that work on a household (it's being deleted).
pub fn forget_household(
    state: &AppState,
    household: crate::households::HouseholdId,
) -> Result<(), AppError> {
    state.households.home().db.lock().execute(
        "DELETE FROM oauth_tokens WHERE household_id = ?1",
        [household],
    )?;
    Ok(())
}

// ─── Consent screen ─────────────────────────────────────────────────────────

pub fn escape_html(s: &str) -> String {
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

fn page(title: &str, body: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<meta name="theme-color" content="#ee5a3a">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<title>{} · Crumb</title>
<style>
  :root {{ color-scheme: light dark; --bg:#fbf7f1; --card:#fffdf8; --fg:#2a211a; --muted:#7a6a5b; --line:#e6dac8; --accent:#ee5a3a; --accent-fg:#fff; --r:22px }}
  @media (prefers-color-scheme: dark) {{ :root {{ --bg:#15110d; --card:#221c16; --fg:#f6efe6; --muted:#b3a393; --line:#342b22; }} }}
  * {{ box-sizing:border-box }}
  body {{ margin:0; min-height:100vh; display:grid; place-items:center; padding:16px; background:var(--bg); color:var(--fg);
    font:16px/1.5 ui-rounded, system-ui, -apple-system, "Segoe UI", sans-serif }}
  main {{ width:100%; max-width:420px; background:var(--card); border:1px solid var(--line); border-radius:var(--r); padding:28px }}
  .brand {{ display:flex; align-items:center; gap:10px; font-weight:700; margin-bottom:18px }}
  .brand span {{ display:grid; place-items:center; width:36px; height:36px; border-radius:12px; background:var(--accent); color:var(--accent-fg); font-size:20px }}
  h1 {{ font-size:1.3rem; margin:0 0 8px; line-height:1.25 }} p {{ color:var(--muted); margin:0 0 20px }}
  label {{ display:block; font-size:.875rem; font-weight:600; margin-bottom:6px }}
  input[type=password] {{ width:100%; padding:12px 16px; border-radius:var(--r); border:1px solid var(--line); background:var(--bg); color:inherit; font:inherit; margin-bottom:16px }}
  input[type=password]:focus {{ outline:2px solid var(--accent); outline-offset:1px }}
  .row {{ display:flex; gap:10px }}
  button {{ flex:1; padding:12px 16px; border-radius:var(--r); border:1px solid var(--line); font:inherit; font-weight:600; cursor:pointer; background:transparent; color:inherit }}
  button.primary {{ background:var(--accent); border-color:var(--accent); color:var(--accent-fg) }}
  .error {{ color:#d9432a; font-size:.875rem; margin:-8px 0 12px }} code {{ font-size:.85em }}
</style></head><body><main><div class="brand"><span aria-hidden="true">✦</span>Crumb</div>{}</main></body></html>"##,
        escape_html(title),
        body
    )
}

#[derive(Default)]
struct AuthorizeParams {
    client_id: String,
    redirect_uri: String,
    state: String,
    code_challenge: String,
    code_challenge_method: String,
    response_type: String,
    resource: String,
}

impl AuthorizeParams {
    fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let v = |k: &str| get(k).unwrap_or_default();
        let or = |k: &str, d: &str| {
            get(k)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| d.to_string())
        };
        Self {
            client_id: v("client_id"),
            redirect_uri: v("redirect_uri"),
            state: v("state"),
            code_challenge: v("code_challenge"),
            code_challenge_method: or("code_challenge_method", "S256"),
            response_type: or("response_type", "code"),
            resource: v("resource"),
        }
    }

    fn pairs(&self) -> [(&'static str, &str); 7] {
        [
            ("client_id", &self.client_id),
            ("redirect_uri", &self.redirect_uri),
            ("state", &self.state),
            ("code_challenge", &self.code_challenge),
            ("code_challenge_method", &self.code_challenge_method),
            ("response_type", &self.response_type),
            ("resource", &self.resource),
        ]
    }
}

fn fail(message: &str) -> Response {
    let mut res = (
        StatusCode::BAD_REQUEST,
        Html(page(
            "Can't connect",
            &format!("<h1>Can't connect</h1><p>{}</p>", escape_html(message)),
        )),
    )
        .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

fn redirect_back(redirect_uri: &str, params: &[(&str, &str)]) -> Response {
    let Ok(mut url) = url::Url::parse(redirect_uri) else {
        return fail("Invalid redirect URI.");
    };
    let keys: Vec<&str> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| *k)
        .collect();
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !keys.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    {
        let mut q = url.query_pairs_mut();
        q.clear();
        for (k, v) in &kept {
            q.append_pair(k, v);
        }
        for (k, v) in params.iter().filter(|(_, v)| !v.is_empty()) {
            q.append_pair(k, v);
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }
    crate::auth::found(url.as_str())
}

async fn authorize_get(
    State(state): State<AppState>,
    axum::Extension(ip): axum::Extension<crate::throttle::ClientIp>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let params = AuthorizeParams::read(|k| q.get(k).cloned());
    authorize(&state, &ip, &headers, Method::GET, params, HashMap::new()).await
}

async fn authorize_post(
    State(state): State<AppState>,
    axum::Extension(ip): axum::Extension<crate::throttle::ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: HashMap<String, String> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let params = AuthorizeParams::read(|k| form.get(k).cloned());
    authorize(&state, &ip, &headers, Method::POST, params, form).await
}

async fn authorize(
    state: &AppState,
    ip: &crate::throttle::ClientIp,
    headers: &HeaderMap,
    method: Method,
    params: AuthorizeParams,
    form: HashMap<String, String>,
) -> Response {
    let client = if params.client_id.is_empty() {
        None
    } else {
        get_client(state, &params.client_id)
    };
    let Some(client) = client else {
        return fail("Unknown client. Remove the connector in the app and add it again.");
    };
    if !client.redirect_uris.contains(&params.redirect_uri) {
        return fail("The redirect URI doesn't match this client's registration.");
    }
    if params.response_type != "code" {
        return redirect_back(
            &params.redirect_uri,
            &[
                ("error", "unsupported_response_type"),
                ("state", &params.state),
            ],
        );
    }
    if params.code_challenge.is_empty() || params.code_challenge_method != "S256" {
        return redirect_back(
            &params.redirect_uri,
            &[
                ("error", "invalid_request"),
                ("error_description", "PKCE with S256 is required"),
                ("state", &params.state),
            ],
        );
    }

    // The endpoint this approval is for (RFC 8707); none means Claude's, as before
    let origin = state.config.public_origin(headers);
    let resource = if params.resource.is_empty() {
        None
    } else {
        // Only the path matters: tokens live in this server's database, and the address a
        // client was given can differ from the public origin (a proxy, a trailing slash)
        let path = url::Url::parse(&params.resource)
            .ok()
            .map(|u| u.path().trim_end_matches('/').to_string());
        match path.filter(|p| matches!(p.as_str(), "/mcp" | "/mcp/chatgpt")) {
            Some(p) => Some(p),
            None => {
                return redirect_back(
                    &params.redirect_uri,
                    &[
                        ("error", "invalid_target"),
                        ("error_description", "Unknown resource"),
                        ("state", &params.state),
                        ("iss", &origin),
                    ],
                );
            }
        }
    };

    // With accounts, the connector is approved by a signed-in person, for their household
    let session = crate::auth::session(state, headers).await;
    let accounts = state.accounts.is_some();
    let logged_in = if accounts {
        session.is_some()
    } else {
        is_logged_in(&state.config, headers)
    };
    let owner = session.as_ref().map(|s| Owner {
        user_id: s.user_id,
        household_id: s.household_id,
    });
    let mut error = "";
    if method == Method::POST {
        if form.get("action").map(String::as_str) == Some("deny") {
            return redirect_back(
                &params.redirect_uri,
                &[
                    ("error", "access_denied"),
                    ("state", &params.state),
                    ("iss", &origin),
                ],
            );
        }
        let password = form.get("password").map(String::as_str).unwrap_or("");
        // Guessing the app password here is limited like signing in
        let mut attempt = None;
        if !logged_in && !accounts {
            match state.login_attempt(ip, "password") {
                Ok(keys) => attempt = Some(keys),
                Err(err) => return fail(&err.message),
            }
        }
        let allowed = logged_in || (!accounts && check_password(&state.config, password));
        if let (true, Some(keys)) = (allowed, &attempt) {
            state.login_succeeded(keys);
        }
        if allowed {
            let code = match issue(
                state,
                Kind::Code,
                &client.id,
                Some(&params.code_challenge),
                Some(&params.redirect_uri),
                Grant {
                    owner,
                    granted: now_secs(),
                    resource: resource.as_deref(),
                },
            ) {
                Ok(code) => code,
                Err(err) => return err.into_response(),
            };
            let mut res = redirect_back(
                &params.redirect_uri,
                &[("code", &code), ("state", &params.state), ("iss", &origin)],
            );
            if !logged_in
                && !accounts
                && let Some(cookie) = login_cookie(&state.config, headers)
            {
                res.headers_mut().append(header::SET_COOKIE, cookie);
            }
            return res;
        }
        error = if accounts {
            "Sign in to Crumb first."
        } else {
            "Incorrect password."
        };
    }

    let host = url::Url::parse(&params.redirect_uri)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| match u.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_string(),
            })
        })
        .unwrap_or_default();
    let hidden: String = params
        .pairs()
        .iter()
        .map(|(k, v)| {
            format!(
                r#"<input type="hidden" name="{k}" value="{}">"#,
                escape_html(v)
            )
        })
        .collect();
    let password_field = if logged_in {
        match &session {
            Some(s) => format!(
                "<p>It will work on <strong>{}</strong>, as {}.</p>",
                escape_html(&s.household_name),
                escape_html(&s.email)
            ),
            None => String::new(),
        }
    } else if accounts {
        // Back here once signed in
        let back = format!(
            "/oauth/authorize?{}",
            serde_urlencoded::to_string(params.pairs()).unwrap_or_default()
        );
        format!(
            r#"<p><a href="/login?next={}">Sign in to Crumb</a> to connect it.</p>"#,
            escape_html(&utf8_percent_encode(&back, NON_ALPHANUMERIC).to_string())
        )
    } else {
        r#"<label for="password">App password</label>
             <input id="password" name="password" type="password" autocomplete="current-password" required autofocus>"#
            .to_string()
    };
    let error_html = if error.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="error">{}</div>"#, escape_html(error))
    };
    let heading = consent_heading(client.name.as_deref(), &host);
    let body = format!(
        r#"{heading}
    <form method="post" action="/oauth/authorize">
      {hidden}
      {password_field}
      {error_html}
      <div class="row">
        <button type="submit" name="action" value="deny" formnovalidate>Cancel</button>
        <button type="submit" name="action" value="allow" class="primary">Allow</button>
      </div>
    </form>"#
    );
    let mut res = Html(page("Connect", &body)).into_response();
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    res
}

/// Hosts whose apps may be called by the name they registered with: Claude's and ChatGPT's own.
fn is_known_host(host: &str) -> bool {
    ["claude.ai", "claude.com", "chatgpt.com"]
        .iter()
        .any(|k| host == *k || host.ends_with(&format!(".{k}")))
}

/// The consent screen's question. Anyone can register a client with any name, so the name is
/// only trusted for Claude's own hosts; for everything else the address the approval is sent
/// to comes first and the name is shown as what the app calls itself.
fn consent_heading(client_name: Option<&str>, host: &str) -> String {
    let host_html = escape_html(host);
    let name = client_name.map(str::trim).filter(|n| !n.is_empty());
    let (title, who) = match name {
        Some(n) if is_known_host(host) => {
            let n = escape_html(n);
            (
                format!("Connect {n}?"),
                format!("<strong>{n}</strong> (<code>{host_html}</code>)"),
            )
        }
        Some(n) => (
            format!("Connect an app at <code>{host_html}</code>?"),
            format!(
                "An app calling itself \u{201c}{}\u{201d}, which sends you back to <code>{host_html}</code>,",
                escape_html(n)
            ),
        ),
        None => (
            format!("Connect an app at <code>{host_html}</code>?"),
            format!("An app that sends you back to <code>{host_html}</code>"),
        ),
    };
    let warning = if is_known_host(host) {
        ""
    } else {
        "<p>Only continue if you just set this up yourself. Anyone can choose an app's name; the address above is where your approval goes.</p>"
    };
    format!(
        "<h1>{title}</h1>\n    <p>{who} wants to read, add and edit recipes in Crumb.</p>{warning}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_consent_screen_leads_with_the_host_not_the_name() {
        let known = consent_heading(Some("Claude"), "claude.ai");
        assert!(known.contains("Connect Claude?"));
        let spoof = consent_heading(Some("Claude <b>"), "attacker.example");
        assert!(!spoof.contains("Connect Claude"), "{spoof}");
        assert!(spoof.contains("Connect an app at <code>attacker.example</code>?"));
        assert!(spoof.contains("calling itself"));
        assert!(
            spoof.contains("Claude &lt;b&gt;"),
            "the name is escaped: {spoof}"
        );
        // A host that only ends in the same letters, or has claude.ai as a prefix, is not Claude's
        assert!(!is_known_host("evilclaude.ai"));
        assert!(!is_known_host("claude.ai.evil.example"));
        assert!(is_known_host("claude.com"));
        let none = consent_heading(None, "a.test");
        assert!(none.contains("Connect an app at"));
    }

    #[test]
    fn redirect_uri_rules() {
        assert!(is_allowed_redirect_uri(
            "https://claude.ai/api/mcp/auth_callback"
        ));
        assert!(is_allowed_redirect_uri("http://localhost:6274/cb"));
        assert!(!is_allowed_redirect_uri("http://evil.test/cb"));
        assert!(!is_allowed_redirect_uri("https://a.test/cb#frag"));
        assert!(!is_allowed_redirect_uri("javascript:alert(1)"));
    }

    #[test]
    fn redirect_back_sets_params() {
        let res = redirect_back(
            "https://a.test/cb?state=old&x=1",
            &[("code", "abc"), ("state", "s1")],
        );
        let loc = res.headers()[header::LOCATION].to_str().unwrap();
        assert_eq!(loc, "https://a.test/cb?x=1&code=abc&state=s1");
    }
}
