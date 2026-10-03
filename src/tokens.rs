//! API tokens: long-lived bearer tokens for scripts, agents and the `crumb` CLI, which have no
//! browser to hold a session cookie. A cook makes one on the account page (signed in with a
//! cookie, never with a token), it's shown once and kept hashed, and it works on one household
//! with a scope: `read` (GET only) or `write`. Like the connector's rule, a token can never
//! destroy what a recipe's text could talk a client into destroying: it can't empty or purge the
//! Trash, delete the account, touch sign-in, sessions, members or connected apps, or make tokens.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, Request, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::{SignedIn, random_token, sha256_hex};
use crate::error::AppError;
use crate::households::{HOME, HouseholdId};
use crate::model::now_secs;

/// Every token starts with this, so a bearer header can be told from the connector's.
pub const PREFIX: &str = "crumb_pat_";
const DAY: i64 = 24 * 60 * 60;
/// `last_used_at` is written at most this often, so reads stay reads.
const TOUCH_EVERY: i64 = 60 * 60;
const MAX_TOKENS: i64 = 50;
const MAX_NAME: usize = 80;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/tokens", routing::get(list).post(create))
        .route("/api/tokens/{id}", routing::delete(revoke))
}

/// The API token in a request's `Authorization` header, if it carries one.
pub fn bearer(headers: &HeaderMap) -> Option<String> {
    let header = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = header.split_once(char::is_whitespace)?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && token.starts_with(PREFIX)).then(|| token.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Read,
    Write,
}

impl Scope {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

struct Found {
    id: i64,
    scope: Scope,
    user: Option<i64>,
    household: Option<HouseholdId>,
    last_used: Option<i64>,
}

fn find(state: &AppState, token: &str) -> Option<Found> {
    let conn = state.households.home().db.lock();
    let found = conn
        .query_row(
            "SELECT id, scope, user_id, household_id, last_used_at FROM api_tokens
             WHERE hash = ?1 AND (expires_at IS NULL OR expires_at > ?2)",
            params![sha256_hex(token), now_secs()],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten()?;
    Some(Found {
        id: found.0,
        scope: Scope::parse(&found.1)?,
        user: found.2,
        household: found.3,
        last_used: found.4,
    })
}

/// Paths a token never reaches, whatever its scope.
fn refused(method: &Method, path: &str) -> bool {
    let path = path.trim_end_matches('/');
    let under = |p: &str| path == p || path.starts_with(&format!("{p}/"));
    under("/api/tokens")
        || under("/api/auth")
        || under("/api/account")
        || under("/api/connections")
        || (method == Method::DELETE && under("/api/trash"))
}

fn deny(status: u16, message: &str) -> Response {
    AppError::new(status, message).into_response()
}

/// Runs a request as the owner of an API token: their household's box, and, with accounts,
/// their sign-in for handlers that ask who it is.
pub async fn serve(state: &AppState, token: &str, mut req: Request<Body>, next: Next) -> Response {
    let Some(found) = find(state, token) else {
        return deny(
            401,
            "That token isn't valid, or it has expired or been revoked",
        );
    };
    if refused(req.method(), req.uri().path()) {
        return deny(403, "An API token can't do that. Use Crumb in the browser");
    }
    let reads = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if found.scope == Scope::Read && !reads {
        return deny(403, "This token can only read. Make one that can write");
    }
    if let Some(accounts) = &state.accounts {
        let (Some(user), Some(household)) = (found.user, found.household) else {
            return deny(
                401,
                "That token isn't valid, or it has expired or been revoked",
            );
        };
        let member = match &state.hosted {
            Some(hosted) => hosted.is_member(accounts, user, household).await,
            None => accounts.is_member(user, household),
        };
        let session = match (member, accounts.member_session(user, household)) {
            (Ok(true), Ok(Some(session))) => session,
            (Ok(_), Ok(_)) => {
                return deny(
                    401,
                    "That token isn't valid, or it has expired or been revoked",
                );
            }
            (Err(err), _) | (_, Err(err)) => return err.into_response(),
        };
        let scoped = match state.for_household(household) {
            Ok(scoped) => scoped,
            Err(err) => return err.into_response(),
        };
        req.extensions_mut().insert(crate::Scoped(scoped));
        req.extensions_mut().insert(SignedIn(session));
    }
    let now = now_secs();
    if found.last_used.is_none_or(|t| now - t >= TOUCH_EVERY) {
        let _ = state.households.home().db.lock().execute(
            "UPDATE api_tokens SET last_used_at = ?1 WHERE id = ?2",
            params![now, found.id],
        );
    }
    next.run(req).await
}

/// Whose tokens a request manages: a person's with accounts, the box's with one password.
fn owner(
    state: &AppState,
    signed: Option<&axum::Extension<SignedIn>>,
) -> Result<(Option<i64>, HouseholdId), AppError> {
    match (&state.accounts, signed) {
        (None, _) => Ok((None, HOME)),
        (Some(_), Some(s)) => Ok((Some(s.0.0.user_id), s.0.0.household_id)),
        (Some(_), None) => Err(AppError::new(401, "Not signed in")),
    }
}

/// The signed-in person's tokens, newest first. Never the token itself.
async fn list(
    State(state): State<AppState>,
    signed: Option<axum::Extension<SignedIn>>,
) -> Result<Json<Value>, AppError> {
    let (user, _) = owner(&state, signed.as_ref())?;
    type Row = (
        i64,
        String,
        String,
        Option<i64>,
        i64,
        Option<i64>,
        Option<i64>,
    );
    let rows: Vec<Row> = {
        let conn = state.households.home().db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, scope, household_id, created_at, last_used_at, expires_at
             FROM api_tokens WHERE user_id IS ?1 ORDER BY created_at DESC, id DESC",
        )?;
        stmt.query_map([user], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    let now = now_secs();
    Ok(Json(json!(
        rows.into_iter()
            .map(|(id, name, scope, household, created, used, expires)| {
                let household = household.map(|h| {
                    let name = state
                        .accounts
                        .as_ref()
                        .and_then(|a| a.household_name(h).ok().flatten());
                    json!({"id": h, "name": name})
                });
                json!({
                    "id": id,
                    "name": name,
                    "scope": scope,
                    "household": household,
                    "createdAt": created,
                    "lastUsedAt": used,
                    "expiresAt": expires,
                    "expired": expires.is_some_and(|e| e <= now),
                })
            })
            .collect::<Vec<_>>()
    )))
}

/// Makes a token: `{name, scope: "read" | "write", expiresInDays?}`. The token is in the
/// reply once and nowhere else. Only a signed-in person (a cookie) can ask.
async fn create(
    State(state): State<AppState>,
    signed: Option<axum::Extension<SignedIn>>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let (user, household) = owner(&state, signed.as_ref())?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(AppError::bad_request(format!(
            "name: Give the token a name of 1 to {MAX_NAME} characters"
        )));
    }
    let scope = body
        .get("scope")
        .and_then(Value::as_str)
        .and_then(Scope::parse)
        .ok_or_else(|| AppError::bad_request("scope: Use \"read\" or \"write\""))?;
    let now = now_secs();
    let expires = match body.get("expiresInDays") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let days = v
                .as_i64()
                .filter(|d| (1..=3650).contains(d))
                .ok_or_else(|| {
                    AppError::bad_request("expiresInDays: Use 1 to 3650, or leave it out")
                })?;
            Some(now + days * DAY)
        }
    };
    let token = format!("{PREFIX}{}", random_token(32));
    let conn = state.households.home().db.lock();
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM api_tokens WHERE user_id IS ?1",
        [user],
        |r| r.get(0),
    )?;
    if count >= MAX_TOKENS {
        return Err(AppError::bad_request(
            "Too many tokens. Revoke ones you no longer use",
        ));
    }
    conn.execute(
        "INSERT INTO api_tokens (hash, name, scope, user_id, household_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            sha256_hex(&token),
            name,
            scope.as_str(),
            user,
            household,
            now,
            expires
        ],
    )?;
    Ok(Json(json!({
        "id": conn.last_insert_rowid(),
        "name": name,
        "scope": scope.as_str(),
        "token": token,
        "createdAt": now,
        "expiresAt": expires,
    })))
}

/// Revokes a token: it stops working at once.
async fn revoke(
    State(state): State<AppState>,
    signed: Option<axum::Extension<SignedIn>>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let (user, _) = owner(&state, signed.as_ref())?;
    let n = state.households.home().db.lock().execute(
        "DELETE FROM api_tokens WHERE id = ?1 AND user_id IS ?2",
        params![id, user],
    )?;
    if n == 0 {
        return Err(AppError::not_found("There's no such token"));
    }
    Ok(Json(json!({"ok": true})))
}

/// With accounts set up for the first time, the box's tokens become its owner's.
pub fn adopt(state: &AppState, owner: i64) -> Result<(), AppError> {
    state.households.home().db.lock().execute(
        "UPDATE api_tokens SET user_id = ?1, household_id = ?2 WHERE household_id IS NULL OR user_id IS NULL",
        params![owner, HOME],
    )?;
    Ok(())
}

/// Deletes every token a person made (their account is going).
pub fn forget_user(state: &AppState, user: i64) -> Result<(), AppError> {
    state
        .households
        .home()
        .db
        .lock()
        .execute("DELETE FROM api_tokens WHERE user_id = ?1", [user])?;
    Ok(())
}

/// Deletes the tokens that work on a household (it's being deleted).
pub fn forget_household(state: &AppState, household: HouseholdId) -> Result<(), AppError> {
    state.households.home().db.lock().execute(
        "DELETE FROM api_tokens WHERE household_id = ?1",
        [household],
    )?;
    Ok(())
}
