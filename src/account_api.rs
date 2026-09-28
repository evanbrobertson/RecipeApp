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
        .route("/api/account/export", routing::get(export_account))
        .route("/api/account/delete", routing::post(delete_account))
        .route("/api/account/email", routing::post(change_email))
        .route("/api/account/email/confirm", routing::post(confirm_email))
}

/// How recent a sign-in must be to stand in for a password (an account made with Google or
/// Apple has none), as Better Auth's `freshAge` does for the hosted edition.
const FRESH_SECS: i64 = 60 * 60 * 24;

/// Accounts, when this server keeps them itself (`AUTH_MODE=accounts`). Hosted, Better
/// Auth answers for them through the proxy instead (see `crate::hosted`).
pub fn accounts(state: &AppState) -> AppResult<&Accounts> {
    if state.hosted.is_some() {
        return Err(AppError::not_found("Not found"));
    }
    state
        .accounts
        .as_deref()
        .ok_or_else(|| AppError::not_found("Accounts aren't turned on here"))
}

pub fn text<'a>(body: &'a Value, key: &str, label: &str) -> AppResult<&'a str> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::bad_request(format!("{key}: {label} is required")))
}

pub fn json_body(body: &Bytes) -> AppResult<Value> {
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
            "providers": hosted.providers().await,
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
        "providers": crate::social::provider_ids(&state),
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
    let mut res = Json(json!({"ok": true})).into_response();
    if let Some(cookie) = new_session(accounts, user, household, headers)? {
        res.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(res)
}

/// Starts a session for `user` in `household`: the `Set-Cookie` that carries it.
pub fn new_session(
    accounts: &Accounts,
    user: i64,
    household: i64,
    headers: &HeaderMap,
) -> AppResult<Option<header::HeaderValue>> {
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let token = accounts.start_session(user, household, agent)?;
    Ok(auth::session_cookie(&token, headers))
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

/// "Download my data": the account (who, how they sign in, their devices), the apps they
/// connected, and every household they're in with its recipes in the backup format.
async fn export_account(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    headers: HeaderMap,
) -> AppResult<Response> {
    let Some(accounts) = &state.accounts else {
        return Err(AppError::not_found("Accounts aren't turned on here"));
    };
    let mut data = match &state.hosted {
        Some(hosted) => {
            let mut data = hosted.account("export", &headers, json!({})).await?;
            // Better Auth's organizations, as the households (and recipe files) they are here
            if let Some(list) = data["households"].as_array_mut() {
                for h in list {
                    let local = h["externalId"]
                        .as_str()
                        .map(|ext| accounts.find_hosted_household(ext))
                        .transpose()?
                        .flatten();
                    h["id"] = json!(local);
                }
            }
            data
        }
        None => accounts.export(signed.user_id)?,
    };
    if let Some(list) = data["households"].as_array_mut() {
        for h in list {
            if let Some(id) = h["id"].as_i64() {
                let box_ = state.for_household(id)?;
                h["recipes"] = crate::recipes::export_backup(&box_.db.lock())?;
            }
        }
    }
    data["connectedApps"] = json!(crate::oauth::connected_apps(&state, Some(signed.user_id))?);
    data["exportedAt"] = json!(crate::model::now_secs());
    let body = serde_json::to_string_pretty(&data).map_err(AppError::internal)?;
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let mut res = body.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    h.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(v) = header::HeaderValue::from_str(&format!(
        "attachment; filename=\"crumb-account-{date}.json\""
    )) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

/// Changes the signed-in account's email: `{email, password}`, the password when it has one
/// (else a sign-in from the last day). Answers `{email, pending}`.
///
/// Self-hosted Crumb sends no email, so the change is made at once and other devices are
/// signed out; here the address is only what people sign in with, never how they get back in.
/// Hosted, with email, `pending` is true: the new address gets a link to confirm it first, and
/// once confirmed the old one gets a link to undo it (see `auth/src/email.ts`).
async fn change_email(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let Some(accounts) = &state.accounts else {
        return Err(AppError::not_found("Accounts aren't turned on here"));
    };
    let body = json_body(&body)?;
    let email = text(&body, "email", "Email")?;
    let password = body.get("password").and_then(Value::as_str).unwrap_or("");
    if let Some(hosted) = &state.hosted {
        let reply = hosted
            .account(
                "change-email",
                &headers,
                json!({"email": email, "password": password}),
            )
            .await?;
        hosted.forget();
        return Ok(Json(reply));
    }
    let (has_password, _) = accounts.sign_in_methods(signed.user_id)?;
    if has_password {
        if password.is_empty() || !accounts.check_password(signed.user_id, password).await? {
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            return Err(AppError::new(401, "password: Incorrect password"));
        }
    } else {
        let started = accounts.session_started(signed.id)?.unwrap_or(0);
        if crate::model::now_secs() - started > FRESH_SECS {
            return Err(AppError::new(
                403,
                "For your safety, sign out and in again, then change your email.",
            ));
        }
    }
    let email = accounts.change_email(signed.user_id, email)?;
    accounts.revoke_others(&signed)?;
    tracing::info!("[auth] an account changed its email");
    Ok(Json(json!({"ok": true, "pending": false, "email": email})))
}

/// Public: a hosted change-email link's `{token}`, from whichever browser opened the email.
/// Answers `{done: "changed" | "reverted", email}`.
async fn confirm_email(State(state): State<AppState>, body: Bytes) -> AppResult<Json<Value>> {
    let Some(hosted) = &state.hosted else {
        return Err(AppError::not_found("Not found"));
    };
    let body = json_body(&body)?;
    let token = text(&body, "token", "Link")?;
    let reply = hosted
        .account("confirm-email", &HeaderMap::new(), json!({"token": token}))
        .await?;
    hosted.forget();
    Ok(Json(reply))
}

/// Deletes the signed-in account: `{password}` when it has one, else `{confirm: email}`.
/// Households it shares pass to whoever joined first; ones it had to itself are deleted,
/// recipes and all. Connectors it approved stop working.
async fn delete_account(
    State(state): State<AppState>,
    SignedIn(signed): SignedIn,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let Some(accounts) = &state.accounts else {
        return Err(AppError::not_found("Accounts aren't turned on here"));
    };
    let body = if body.is_empty() {
        json!({})
    } else {
        json_body(&body)?
    };
    let password = body.get("password").and_then(Value::as_str).unwrap_or("");
    let deleted = match &state.hosted {
        Some(hosted) => {
            let reply = hosted
                .account("delete-account", &headers, json!({"password": password}))
                .await?;
            hosted.forget();
            let mut gone = Vec::new();
            for ext in reply["deletedHouseholds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if let Some(id) = accounts.find_hosted_household(ext)? {
                    gone.push(id);
                }
            }
            accounts.forget_hosted(signed.user_id, &gone)?
        }
        None => {
            let (has_password, _) = accounts.sign_in_methods(signed.user_id)?;
            if has_password {
                if password.is_empty() || !accounts.check_password(signed.user_id, password).await?
                {
                    tokio::time::sleep(std::time::Duration::from_millis(750)).await;
                    return Err(AppError::new(401, "password: Incorrect password"));
                }
            } else {
                let confirm = body.get("confirm").and_then(Value::as_str).unwrap_or("");
                if !confirm.trim().eq_ignore_ascii_case(&signed.email) {
                    return Err(AppError::bad_request(
                        "confirm: Type your email address to confirm",
                    ));
                }
            }
            accounts.delete_account(signed.user_id)?
        }
    };
    crate::oauth::forget_user(&state, signed.user_id)?;
    for &id in &deleted.households {
        crate::oauth::forget_household(&state, id)?;
        state.households.remove(id)?;
        state.images.forget_household(id);
    }
    if deleted.home {
        crate::oauth::forget_household(&state, crate::households::HOME)?;
        crate::db::wipe_box(&mut state.households.home().db.lock())?;
        state.images.forget_household(crate::households::HOME);
    }
    tracing::info!(
        "[auth] an account was deleted, with {} household(s){}",
        deleted.households.len(),
        if deleted.home {
            " and the home box's contents"
        } else {
            ""
        }
    );
    let mut res = Json(json!({"ok": true})).into_response();
    res.headers_mut()
        .append(header::SET_COOKIE, auth::logout_cookie(&headers));
    Ok(res)
}
