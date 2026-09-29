//! The hosted edition (`AUTH_MODE=hosted`): people, sessions and households live in Better
//! Auth, the small `auth/` service beside this server, with households as its
//! organizations.
//!
//! The browser only ever talks to this server: `/api/auth/*` routes it doesn't answer itself
//! are proxied to the service unchanged (its cookies are first-party). To learn who a
//! request is signed in as, the server forwards the session cookie to the service's
//! `/internal/session` and maps the answer onto the ids everything else uses: a local user
//! id (for connector tokens) and a local household id (for the recipe file,
//! `households/{id}/recipes.db`), both kept in `accounts.db` (see
//! [`Accounts::hosted_household`]). Answers are cached briefly so a page's burst of requests
//! costs one lookup; anything sent through the proxy that could change them clears the cache.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;
use crate::accounts::{Accounts, Session};
use crate::auth::sha256_hex;
use crate::error::{AppError, AppResult};
use crate::households::HouseholdId;

/// How long a session lookup is trusted.
const SESSION_TTL: Duration = Duration::from_secs(30);
/// How long a connector's membership check is trusted.
const MEMBER_TTL: Duration = Duration::from_secs(60);
/// Cache entries kept before the cache is emptied (it only holds live sessions' hashes).
const CACHE_CAP: usize = 10_000;
/// Better Auth's session cookie (`advanced.cookiePrefix` is `crumb`), with or without the
/// `__Secure-` prefix it gets over HTTPS.
const SESSION_COOKIE: &str = "crumb.session_token";
/// Request headers passed through to the service; nothing else (no forwarded-for chain the
/// client could have written) goes along.
const FORWARD: [HeaderName; 6] = [
    header::COOKIE,
    header::CONTENT_TYPE,
    header::ACCEPT,
    header::ORIGIN,
    header::REFERER,
    header::USER_AGENT,
];

#[derive(Deserialize)]
struct SessionReply {
    session: Option<Remote>,
}

#[derive(Deserialize)]
struct Remote {
    user: RemoteUser,
    household: Option<RemoteHousehold>,
}

#[derive(Deserialize)]
struct RemoteUser {
    id: String,
    email: String,
    name: String,
}

#[derive(Deserialize)]
struct RemoteHousehold {
    id: String,
    name: String,
    role: String,
}

#[derive(Deserialize)]
struct MemberReply {
    member: bool,
}

pub struct Hosted {
    base: String,
    secret: String,
    http: reqwest::Client,
    home_owner: Option<String>,
    sessions: Mutex<HashMap<String, (Instant, Option<Session>)>>,
    members: Mutex<HashMap<(i64, HouseholdId), (Instant, bool)>>,
    /// The social sign-ins the service offers (its config: asked once).
    providers: Mutex<Option<Vec<String>>>,
}

#[derive(Deserialize)]
struct ProvidersReply {
    providers: Vec<String>,
}

impl Hosted {
    /// `base` is the service's internal URL; `secret` its `AUTH_INTERNAL_SECRET`.
    pub fn new(base: &str, secret: &str, home_owner: Option<String>) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            secret: secret.to_string(),
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                // Verification links answer with redirects meant for the browser
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client"),
            home_owner,
            sessions: Mutex::default(),
            members: Mutex::default(),
            providers: Mutex::default(),
        }
    }

    /// The signed-in person and household for a request's cookies, if any. Someone in no
    /// household yet gets a kitchen of their own made (see `auth/src/kitchen.ts`).
    pub async fn session(
        &self,
        accounts: &Accounts,
        headers: &HeaderMap,
    ) -> AppResult<Option<Session>> {
        let Some(key) = session_key(headers) else {
            return Ok(None);
        };
        if let Some((at, found)) = lock(&self.sessions).get(&key)
            && at.elapsed() < SESSION_TTL
        {
            return Ok(found.clone());
        }
        let found = match self.ask(headers, true).await? {
            Some(remote) => self.local(accounts, remote)?,
            None => None,
        };
        self.remember(key, found.clone());
        Ok(found)
    }

    /// Who's signed in, for `/api/auth/status`: their name and email, and their session
    /// when they're in a household. Unlike [`Self::session`] this never makes a kitchen, so
    /// someone who signed up to accept an invite joins that household and nothing else.
    pub async fn who(
        &self,
        accounts: &Accounts,
        headers: &HeaderMap,
    ) -> AppResult<Option<(String, String, Option<Session>)>> {
        let Some(key) = session_key(headers) else {
            return Ok(None);
        };
        if let Some((at, Some(s))) = lock(&self.sessions).get(&key)
            && at.elapsed() < SESSION_TTL
        {
            return Ok(Some((s.name.clone(), s.email.clone(), Some(s.clone()))));
        }
        let Some(remote) = self.ask(headers, false).await? else {
            return Ok(None);
        };
        let (name, email) = (remote.user.name.clone(), remote.user.email.clone());
        let found = self.local(accounts, remote)?;
        if found.is_some() {
            self.remember(key, found.clone());
        }
        Ok(Some((name, email, found)))
    }

    /// Asks the service about a request's session cookies.
    async fn ask(&self, headers: &HeaderMap, create: bool) -> AppResult<Option<Remote>> {
        let cookies = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join("; ");
        let reply: SessionReply = self
            .http
            .get(format!("{}/internal/session", self.base))
            .query(&[("create", if create { "1" } else { "0" })])
            .header("x-crumb-internal", &self.secret)
            .header(header::COOKIE, cookies)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(unavailable)?
            .json()
            .await
            .map_err(unavailable)?;
        Ok(reply.session)
    }

    fn remember(&self, key: String, found: Option<Session>) {
        let mut cache = lock(&self.sessions);
        if cache.len() >= CACHE_CAP {
            cache.clear();
        }
        cache.insert(key, (Instant::now(), found));
    }

    /// The service's answer in local ids (None while they're in no household).
    fn local(&self, accounts: &Accounts, remote: Remote) -> AppResult<Option<Session>> {
        let Some(household) = remote.household else {
            return Ok(None);
        };
        let claim_home = household.role == "owner"
            && self
                .home_owner
                .as_deref()
                .is_some_and(|owner| owner.eq_ignore_ascii_case(&remote.user.email));
        Ok(Some(Session {
            id: 0,
            user_id: accounts.hosted_user(&remote.user.id)?,
            email: remote.user.email,
            name: remote.user.name,
            household_id: accounts.hosted_household(&household.id, &household.name, claim_home)?,
            household_name: household.name,
            role: household.role,
        }))
    }

    /// Whether a connector's owner is still in the household it was approved for.
    pub async fn is_member(
        &self,
        accounts: &Accounts,
        user: i64,
        household: HouseholdId,
    ) -> AppResult<bool> {
        if let Some((at, member)) = lock(&self.members).get(&(user, household))
            && at.elapsed() < MEMBER_TTL
        {
            return Ok(*member);
        }
        let Some((user_ext, household_ext)) = accounts.hosted_ids(user, household)? else {
            return Ok(false);
        };
        let reply: MemberReply = self
            .http
            .get(format!("{}/internal/member", self.base))
            .query(&[("user", &user_ext), ("household", &household_ext)])
            .header("x-crumb-internal", &self.secret)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(unavailable)?
            .json()
            .await
            .map_err(unavailable)?;
        let mut cache = lock(&self.members);
        if cache.len() >= CACHE_CAP {
            cache.clear();
        }
        cache.insert((user, household), (Instant::now(), reply.member));
        Ok(reply.member)
    }

    /// Asks the service about the signed-in person (by the request's cookies): `export` for
    /// everything it keeps about them, `delete-account` to delete them. Its refusals (a
    /// wrong password, say) come back as they are.
    pub async fn account(
        &self,
        action: &str,
        headers: &HeaderMap,
        body: serde_json::Value,
    ) -> AppResult<serde_json::Value> {
        let cookies = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join("; ");
        let res = self
            .http
            .post(format!("{}/internal/{action}", self.base))
            .header("x-crumb-internal", &self.secret)
            .header(header::COOKIE, cookies)
            .json(&body)
            .send()
            .await
            .map_err(unavailable)?;
        let status = res.status().as_u16();
        let reply: serde_json::Value = res.json().await.map_err(unavailable)?;
        if (400..500).contains(&status) {
            let message = reply["message"].as_str().unwrap_or("That didn't work");
            return Err(AppError::new(status, message.to_string()));
        }
        if status >= 500 {
            return Err(AppError::new(
                503,
                "Sign-in is unavailable right now. Try again in a moment.",
            ));
        }
        Ok(reply)
    }

    /// Which of Google and Apple people can sign in with (none while the service can't say).
    pub async fn providers(&self) -> Vec<String> {
        if let Some(known) = lock(&self.providers).clone() {
            return known;
        }
        let reply = self
            .http
            .get(format!("{}/internal/providers", self.base))
            .header("x-crumb-internal", &self.secret)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        let Ok(reply) = reply else { return Vec::new() };
        let Ok(reply) = reply.json::<ProvidersReply>().await else {
            return Vec::new();
        };
        *lock(&self.providers) = Some(reply.providers.clone());
        reply.providers
    }

    /// Forgets every cached answer: something may have signed out, switched household or
    /// changed who's in one.
    pub fn forget(&self) {
        lock(&self.sessions).clear();
        lock(&self.members).clear();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn unavailable(err: reqwest::Error) -> AppError {
    tracing::warn!("[hosted] the auth service didn't answer: {err}");
    AppError::new(
        503,
        "Sign-in is unavailable right now. Try again in a moment.",
    )
}

/// The cache key for a request's session: a hash of its session cookie(s), or None when
/// it has none (and so can't be signed in).
fn session_key(headers: &HeaderMap) -> Option<String> {
    let values: Vec<&str> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(k, v)| k.ends_with(SESSION_COOKIE) && !v.is_empty())
        .map(|(_, v)| v)
        .collect();
    (!values.is_empty()).then(|| sha256_hex(&values.join(";")))
}

/// Whether a path has no dot segments, plain or percent-encoded, that a URL parser would
/// resolve to somewhere outside `/api/auth/`.
fn stays_in_auth(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    !lower.contains("%2e")
        && !lower.contains("%2f")
        && !lower.contains('\\')
        && path.split('/').all(|seg| seg != ".." && seg != ".")
}

/// `/api/auth/*` that this server doesn't answer itself: passed to the auth service as is.
pub async fn proxy(State(state): State<AppState>, req: Request) -> Response {
    let Some(hosted) = state.hosted.clone() else {
        return AppError::not_found("Not found").into_response();
    };
    // Only ever Better Auth's routes: no climbing out to the service's /internal/*
    if !stays_in_auth(req.uri().path()) {
        return AppError::not_found("Not found").into_response();
    }
    let ip = crate::share::client_ip(&req, state.config.trust_proxy_headers);
    // Password guesses count against the client's address here too, on top of Better Auth's own
    // limit; a successful sign-in clears it
    let sign_in = req.method() == Method::POST && req.uri().path().starts_with("/api/auth/sign-in");
    let throttle_key = format!("ip:{ip}");
    if sign_in
        && let Err(err) = state
            .logins
            .attempt(&[(&throttle_key, crate::throttle::IP_FREE)])
    {
        return err.into_response();
    }
    let (parts, body) = req.into_parts();
    let path = parts
        .uri
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let Ok(body) = axum::body::to_bytes(body, 1 << 20).await else {
        return AppError::new(413, "Request too large").into_response();
    };
    let mut out = hosted
        .http
        .request(parts.method.clone(), format!("{}{path}", hosted.base))
        .body(body);
    for name in FORWARD {
        for value in parts.headers.get_all(&name) {
            out = out.header(&name, value);
        }
    }
    if ip != "unknown" {
        out = out.header("x-real-ip", ip);
    }
    let res = match out.send().await {
        Ok(res) => res,
        Err(err) => return unavailable(err).into_response(),
    };
    if parts.method != Method::GET {
        hosted.forget();
    }
    if sign_in && res.status().is_success() {
        state.logins.clear(&[&throttle_key]);
    }
    relay(res).await
}

/// The service's response, minus hop-by-hop headers.
async fn relay(res: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut headers = HeaderMap::new();
    for (name, value) in res.headers() {
        if matches!(
            name.as_str(),
            "connection" | "transfer-encoding" | "content-length" | "keep-alive"
        ) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            headers.append(name, value);
        }
    }
    let body: Bytes = match res.bytes().await {
        Ok(b) => b,
        Err(err) => return unavailable(err).into_response(),
    };
    let mut out = Response::new(Body::from(body));
    *out.status_mut() = status;
    *out.headers_mut() = headers;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_proxy_stays_under_api_auth() {
        assert!(stays_in_auth("/api/auth/sign-in/email"));
        assert!(stays_in_auth("/api/auth/organization/get-invitation"));
        for bad in [
            "/api/auth/../internal/session",
            "/api/auth/./x",
            "/api/auth/%2e%2e/internal/session",
            "/api/auth/%2E%2E%2Finternal",
            "/api/auth/..\\internal",
        ] {
            assert!(!stays_in_auth(bad), "{bad}");
        }
    }

    #[test]
    fn only_the_session_cookie_keys_the_cache() {
        let mut h = HeaderMap::new();
        assert_eq!(session_key(&h), None);
        h.insert(header::COOKIE, HeaderValue::from_static("tz=Europe/Paris"));
        assert_eq!(session_key(&h), None);
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("tz=Europe/Paris; __Secure-crumb.session_token=abc.def"),
        );
        let a = session_key(&h).unwrap();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("__Secure-crumb.session_token=abc.def; tz=UTC"),
        );
        assert_eq!(session_key(&h).unwrap(), a, "other cookies don't matter");
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("crumb.session_token=other"),
        );
        assert_ne!(session_key(&h).unwrap(), a);
    }
}
