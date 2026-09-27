//! `/api/auth/*` for accounts (`AUTH_MODE=accounts`): status, first-run setup, sign-up,
//! sign-in and out, the sessions a person can see and revoke, and their household: its
//! members, invite links, leaving and switching. With one password these answer as that
//! mode does (status) or refuse (the rest).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing};
use serde_json::{Value, json};

use crate::AppState;
use crate::accounts::Accounts;
use crate::auth::{self, SignedIn};
use crate::error::{AppError, AppResult};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/status", routing::get(status))
        .route("/api/auth/setup", routing::post(set_up))
        .route("/api/auth/signup", routing::post(sign_up))
        .route("/api/auth/sessions", routing::get(sessions))
        .route("/api/auth/sessions/{id}", routing::delete(revoke))
        .route(
            "/api/auth/sessions/revoke-others",
            routing::post(revoke_others),
        )
        .route(
            "/api/auth/household",
            routing::get(household).patch(rename_household),
        )
        .route(
            "/api/auth/household/switch",
            routing::post(switch_household),
        )
        .route("/api/auth/household/leave", routing::post(leave))
        .route("/api/auth/members/{user}", routing::delete(remove_member))
        .route("/api/auth/invites", routing::post(create_invite))
        .route("/api/auth/invites/{id}", routing::delete(cancel_invite))
        .route("/api/auth/invite/preview", routing::post(preview_invite))
        .route("/api/auth/invite/accept", routing::post(accept_invite))
}

/// Accounts, when this server keeps them itself (`AUTH_MODE=accounts`). Hosted, Better
/// Auth answers for them through the proxy instead (see `crate::hosted`).
fn accounts(state: &AppState) -> AppResult<&Accounts> {
    if state.hosted.is_some() {
        return Err(AppError::not_found("Not found"));
    }
    state
        .accounts
        .as_deref()
        .ok_or_else(|| AppError::not_found("Accounts aren't turned on here"))
}

fn text<'a>(body: &'a Value, key: &str, label: &str) -> AppResult<&'a str> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::bad_request(format!("{key}: {label} is required")))
}

fn json_body(body: &Bytes) -> AppResult<Value> {
    serde_json::from_slice(body).map_err(|_| AppError::bad_request("Invalid JSON body"))
}

/// How this install signs people in, and who's signed in: public, so the login page
/// knows which form to show.
async fn status(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let Some(accounts) = &state.accounts else {
        return Ok(Json(json!({
            "mode": "password",
            "passwordRequired": state.config.app_password.is_some(),
            "signedIn": auth::is_logged_in(&state.config, &headers),
        })));
    };
    if let Some(hosted) = &state.hosted {
        let who = hosted.who(accounts, &headers).await?;
        let signed = who.as_ref().and_then(|w| w.2.as_ref());
        return Ok(Json(json!({
            "mode": "hosted",
            "setupNeeded": false,
            "signupOpen": true,
            "signedIn": who.is_some(),
            "user": who.as_ref().map(|(name, email, _)| json!({"name": name, "email": email})),
            "household": signed.map(|s| json!({
                "id": s.household_id, "name": s.household_name, "role": s.role,
            })),
        })));
    }
    let signed = auth::session(&state, &headers).await;
    let setup_needed = accounts.needs_setup()?;
    Ok(Json(json!({
        "mode": "accounts",
        "setupNeeded": setup_needed,
        // The setup asks for the app password when there is one, so a stranger can't
        // claim the box that's already here
        "setupNeedsAppPassword": setup_needed && state.config.app_password.is_some(),
        "signupOpen": state.config.open_signup && !setup_needed,
        "signedIn": signed.is_some(),
        "user": signed.as_ref().map(|s| json!({"name": s.name, "email": s.email})),
        "household": signed.as_ref().map(|s| json!({
            "id": s.household_id, "name": s.household_name, "role": s.role,
        })),
    })))
}

/// Signs in `user` to `household` and answers with the session cookie.
fn signed_in_response(
    accounts: &Accounts,
    user: i64,
    household: i64,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let token = accounts.start_session(user, household, agent)?;
    let mut res = Json(json!({"ok": true})).into_response();
    if let Some(cookie) = auth::session_cookie(&token, headers) {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(res)
}

/// The first account, which owns the recipe box already here (household 1). Refused once
/// anyone has an account, and without the app password when one is set.
async fn set_up(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let accounts = accounts(&state)?;
    let body = json_body(&body)?;
    if !accounts.needs_setup()? {
        return Err(AppError::new(
            409,
            "Crumb is already set up. Sign in instead.",
        ));
    }
    if state.config.app_password.is_some() {
        let given = body
            .get("appPassword")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !auth::check_password(&state.config, given) {
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            return Err(AppError::new(401, "appPassword: Incorrect app password"));
        }
    }
    let user = accounts
        .set_up(
            text(&body, "email", "Email")?,
            text(&body, "name", "Name")?,
            text(&body, "password", "Password")?,
        )
        .await?;
    // The connector's existing tokens were the box's: they're the owner's now
    crate::oauth::adopt_tokens(&state, user)?;
    signed_in_response(accounts, user, crate::households::HOME, &headers)
}

/// A new account with a recipe box of its own, when sign-up is open (`SIGNUP=open`).
async fn sign_up(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let accounts = accounts(&state)?;
    if !state.config.open_signup {
        return Err(AppError::new(
            403,
            "Sign-up isn't open here. Ask for an invite.",
        ));
    }
    let body = json_body(&body)?;
    let (user, household) = accounts
        .sign_up(
            text(&body, "email", "Email")?,
            text(&body, "name", "Name")?,
            text(&body, "password", "Password")?,
        )
        .await?;
    signed_in_response(accounts, user, household, &headers)
}

/// Accounts' `POST /api/auth/login`: `{email, password}`.
pub async fn log_in(state: &AppState, headers: &HeaderMap, body: &Value) -> AppResult<Response> {
    let accounts = accounts(state)?;
    let email = text(body, "email", "Email")?;
    let password = text(body, "password", "Password")?;
    match accounts.authenticate(email, password).await? {
        Some((user, household)) => signed_in_response(accounts, user, household, headers),
        None => {
            // Slow down guessing
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            Err(AppError::new(401, "Incorrect email or password"))
        }
    }
}

/// Accounts' sign-out: the session ends here, not just the cookie.
pub fn log_out(state: &AppState, headers: &HeaderMap) {
    if let (Some(accounts), Some(token)) =
        (&state.accounts, auth::cookie_value(headers, auth::COOKIE))
        && let Err(err) = accounts.end_session(&token)
    {
        tracing::warn!("[auth] couldn't end a session: {}", err.message);
    }
}

async fn sessions(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
) -> AppResult<Json<Value>> {
    Ok(Json(json!(accounts(&state)?.sessions(&signed)?)))
}

async fn revoke(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    if !accounts(&state)?.revoke(&signed, id)? {
        return Err(AppError::not_found("Session not found"));
    }
    Ok(Json(json!({"ok": true})))
}

async fn revoke_others(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
) -> AppResult<Json<Value>> {
    let ended = accounts(&state)?.revoke_others(&signed)?;
    Ok(Json(json!({"ok": true, "ended": ended})))
}

/// The signed-in household: who's in it, the other households the person is in, and for
/// its owner, the invite links still waiting to be used.
async fn household(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
) -> AppResult<Json<Value>> {
    let accounts = accounts(&state)?;
    let owner = signed.role == "owner";
    Ok(Json(json!({
        "id": signed.household_id,
        "name": signed.household_name,
        "role": signed.role,
        "you": signed.user_id,
        "members": accounts.members(&signed)?,
        "invites": if owner { json!(accounts.invites(&signed)?) } else { json!([]) },
        "households": accounts.memberships(signed.user_id)?,
    })))
}

async fn rename_household(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let body = json_body(&body)?;
    let name = accounts(&state)?.rename_household(&signed, text(&body, "name", "Name")?)?;
    Ok(Json(json!({"ok": true, "name": name})))
}

async fn switch_household(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let id = json_body(&body)?
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::bad_request("id: Household is required"))?;
    accounts(&state)?.switch_household(&signed, id)?;
    Ok(Json(json!({"ok": true})))
}

async fn leave(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
) -> AppResult<Json<Value>> {
    let next = accounts(&state)?.leave(&signed)?;
    Ok(Json(json!({"ok": true, "household": next})))
}

async fn remove_member(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    Path(user): Path<i64>,
) -> AppResult<Json<Value>> {
    accounts(&state)?.remove_member(&signed, user)?;
    Ok(Json(json!({"ok": true})))
}

/// A new invite link: `{id, url, expiresAt}`. The link is only ever shown here; the
/// token is in its fragment, so it never reaches a server log or a `Referer`.
async fn create_invite(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let (token, invite) = accounts(&state)?.create_invite(&signed)?;
    let url = format!("{}/invite#{token}", state.config.public_origin(&headers));
    Ok(Json(json!({
        "id": invite.id,
        "url": url,
        "expiresAt": invite.expires_at,
    })))
}

async fn cancel_invite(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    if !accounts(&state)?.cancel_invite(&signed, id)? {
        return Err(AppError::not_found("Invite not found"));
    }
    Ok(Json(json!({"ok": true})))
}

/// Public: which household an invite link is for, so its page can say so.
async fn preview_invite(State(state): State<AppState>, body: Bytes) -> AppResult<Json<Value>> {
    let accounts = accounts(&state)?;
    let body = json_body(&body)?;
    let token = text(&body, "token", "Invite")?;
    match accounts.preview_invite(token)? {
        Some(preview) => Ok(Json(json!(preview))),
        None => {
            // Slow down guessing
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            Err(AppError::new(
                410,
                "This invite has expired or was already used. Ask for a new one.",
            ))
        }
    }
}

/// Public: joins an invite link's household. Signed in, the person joins and this session
/// moves there; otherwise `{name, email, password}` makes an account in it (even with
/// sign-up closed) and `{email, password}` signs in to an existing one and joins.
async fn accept_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let accounts = accounts(&state)?;
    let body = json_body(&body)?;
    let token = text(&body, "token", "Invite")?;
    if let Some(signed) = auth::session(&state, &headers).await {
        let household = accounts.accept_invite(token, signed.user_id)?;
        accounts.switch_household(&signed, household)?;
        return Ok(Json(json!({"ok": true})).into_response());
    }
    if body.get("name").is_some() {
        let (user, household) = accounts
            .sign_up_invited(
                token,
                text(&body, "email", "Email")?,
                text(&body, "name", "Name")?,
                text(&body, "password", "Password")?,
            )
            .await?;
        return signed_in_response(accounts, user, household, &headers);
    }
    let email = text(&body, "email", "Email")?;
    let password = text(&body, "password", "Password")?;
    if accounts.preview_invite(token)?.is_none() {
        return Err(AppError::new(
            410,
            "This invite has expired or was already used. Ask for a new one.",
        ));
    }
    let Some((user, _)) = accounts.authenticate(email, password).await? else {
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        return Err(AppError::new(401, "Incorrect email or password"));
    };
    let household = accounts.accept_invite(token, user)?;
    signed_in_response(accounts, user, household, &headers)
}
