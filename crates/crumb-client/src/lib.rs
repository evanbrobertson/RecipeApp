//! A typed HTTP client for the Crumb REST API. No UI, no Qt: every `/api` route, over
//! `reqwest`, with a cookie jar for the app-password session. Routes are grouped by area:
//! recipes here and in `recipes`, then `cookbooks`, `checks` (Wee Chef) and `shares`.

use std::sync::Arc;

use reqwest::cookie::{CookieStore, Jar};
use reqwest::{Client as HttpClient, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

pub use crumb_core::model::{
    Cookbook, CookbookListItem, CookbookWithRecipes, Recipe, RecipeSummary, Section,
};
pub use crumb_core::staples::{Staple, Staples};

mod accounts;
mod checks;
mod cookbooks;
mod error;
mod recipes;
mod shares;
mod types;
pub use accounts::*;
pub use error::Error;
pub use types::*;

/// Name of the session cookie the server sets (`src/auth.rs`). The desktop app keeps
/// this value in the keyring so a signed-in session survives a restart.
pub const SESSION_COOKIE: &str = "crumb_session";

/// Better Auth's session cookie for the hosted edition (`advanced.cookiePrefix` is
/// `crumb` in `auth/src/auth.ts`); over HTTPS it gets a `__Secure-` prefix.
const HOSTED_COOKIE: &str = "crumb.session_token";
const SECURE_PREFIX: &str = "__Secure-";

/// What to save: a link or pasted text, sent to `POST /api/recipes/import`.
#[derive(Debug, Clone)]
pub enum ImportInput {
    Url(String),
    Text(String),
}

/// A completed import: the saved recipe and whether it was new (the API's `isNew`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub recipe: Recipe,
    pub is_new: bool,
    /// A cooking video Wee Chef watched in the server's queue.
    pub from_video: bool,
    /// The page's photo link was dead, so it was saved without one.
    pub dropped_photo: bool,
    /// Another Crumb's shared cookbook: where its recipes went. `recipe` is the first new one.
    pub cookbook: Option<ImportedCookbook>,
}

/// A shared cookbook saved from another Crumb ("Added 12 recipes to Weeknights").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedCookbook {
    pub id: i64,
    pub name: String,
    pub added: usize,
    #[serde(default)]
    pub duplicates: usize,
    #[serde(default)]
    pub skipped: usize,
}

/// A cooking video's place in the server's queue (`GET /api/import/jobs/{id}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    /// `queued`, `running`, `done` or `failed`.
    pub status: String,
    /// In the queue: 1 = next.
    pub position: Option<u32>,
    /// Done: `{id, title, isNew}`.
    pub recipe: Option<JobRecipe>,
    /// Failed: the error's status and message.
    pub status_code: Option<u16>,
    pub message: Option<String>,
}

/// The recipe a finished video job saved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecipe {
    pub id: i64,
    pub title: String,
    pub is_new: bool,
}

/// How often [`Client::import_with_progress`] asks after a video job, as the web does.
pub const JOB_POLL: std::time::Duration = std::time::Duration::from_secs(2);

/// The `{statusCode, statusMessage, message}` body the server sends on errors.
#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
    code: Option<String>,
    site: Option<String>,
}

/// The `{id, title, isNew}` body the import route echoes back. A shared-cookbook link
/// adds a `cookbook` object instead of being a single recipe; a cooking video answers 202
/// with `{jobId, status, position?}` instead.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    /// A recipe's id, or a video job's (a string).
    id: Option<serde_json::Value>,
    #[serde(default)]
    is_new: bool,
    #[serde(default)]
    dropped_photo: bool,
    cookbook: Option<ImportedCookbook>,
    job_id: Option<String>,
    #[serde(flatten)]
    job: Option<ImportJob>,
}

/// A Crumb server as a client: base address plus the cookie jar holding the session.
#[derive(Clone)]
pub struct Client {
    base: String,
    url: Url,
    http: HttpClient,
    jar: Arc<Jar>,
}

impl Client {
    /// Points a client at `base_url` (an http or https origin). A trailing slash is
    /// dropped, so endpoints are joined consistently.
    pub fn new(base_url: &str) -> Result<Self, Error> {
        let (base, url) = parse_base(base_url)?;
        let jar = Arc::new(Jar::default());
        let http = HttpClient::builder()
            .cookie_provider(jar.clone())
            .build()
            .map_err(|err| Error::Network(err.to_string()))?;
        Ok(Self {
            base,
            url,
            http,
            jar,
        })
    }

    /// Restores a session from a previously saved [`Client::session_cookie`] value: a bare
    /// `crumb_session` value, or `name=value` for the hosted edition's cookie.
    pub fn with_session(base_url: &str, cookie: &str) -> Result<Self, Error> {
        let client = Self::new(base_url)?;
        client.set_session(cookie);
        Ok(client)
    }

    fn set_session(&self, saved: &str) {
        let (name, value) = split_session(saved);
        let secure = if name.starts_with(SECURE_PREFIX) {
            "; Secure"
        } else {
            ""
        };
        self.jar
            .add_cookie_str(&format!("{name}={value}; Path=/{secure}"), &self.url);
    }

    /// The session to persist in the keyring: the `crumb_session` value on its own, or
    /// `name=value` when the server is hosted and Better Auth set the cookie.
    pub fn session_cookie(&self) -> Option<String> {
        let header = self.jar.cookies(&self.url)?;
        let cookies = header.to_str().ok()?;
        cookies.split(';').find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            if name == SESSION_COOKIE {
                Some(value.to_string())
            } else {
                is_hosted_cookie(name).then(|| format!("{name}={value}"))
            }
        })
    }

    /// The normalized base address (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// `GET /api/health`. Succeeds without a password.
    pub async fn health(&self) -> Result<(), Error> {
        let res = self
            .send(self.http.get(self.endpoint("api/health")))
            .await?;
        self.expect_ok(res, false).await
    }

    /// `GET /api/connector`: what this server can do (Wee Chef, photos, checks).
    pub async fn connector(&self) -> Result<Connector, Error> {
        self.fetch(self.http.get(self.endpoint("api/connector")))
            .await
    }

    /// `POST /api/auth/login`; a wrong password is an [`Error::Api`] with status 401.
    pub async fn login(&self, password: &str) -> Result<(), Error> {
        let res = self
            .send(
                self.http
                    .post(self.endpoint("api/auth/login"))
                    .json(&json!({ "password": password })),
            )
            .await?;
        self.expect_ok(res, true).await
    }

    /// `POST /api/auth/logout`, clearing the session cookie from the jar.
    pub async fn logout(&self) -> Result<(), Error> {
        let res = self
            .send(self.http.post(self.endpoint("api/auth/logout")))
            .await?;
        self.expect_ok(res, true).await
    }

    /// `GET /api/recipes?q=&limit=`.
    pub async fn recipes(
        &self,
        query: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<RecipeSummary>, Error> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(query) = query {
            params.push(("q", query.to_string()));
        }
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        let res = self
            .send(self.http.get(self.endpoint("api/recipes")).query(&params))
            .await?;
        self.json(res, false).await
    }

    /// `GET /api/recipes/{id}`.
    pub async fn recipe(&self, id: i64) -> Result<Recipe, Error> {
        let res = self
            .send(self.http.get(self.endpoint(&format!("api/recipes/{id}"))))
            .await?;
        self.json(res, false).await
    }

    /// `POST /api/recipes/import`: saves a link or pasted text. A cooking video waits in
    /// the server's queue; see [`Client::import_with_progress`] to hear how it's going.
    pub async fn import(&self, input: ImportInput) -> Result<Imported, Error> {
        self.import_with_progress(input, |_| {}).await
    }

    /// Like [`Client::import`], and while a cooking video waits or is watched, `progress`
    /// gets the Add box's short lines ("Queued (2nd)…", `crumb_core::add::job_progress`).
    pub async fn import_with_progress(
        &self,
        input: ImportInput,
        mut progress: impl FnMut(&str),
    ) -> Result<Imported, Error> {
        let body = match input {
            ImportInput::Url(url) => json!({ "url": url }),
            ImportInput::Text(text) => json!({ "text": text }),
        };
        let res = self
            .send(
                self.http
                    .post(self.endpoint("api/recipes/import"))
                    .json(&body),
            )
            .await?;
        let imported: ImportResponse = self.json(res, false).await?;
        if let (Some(job_id), Some(mut job)) = (imported.job_id, imported.job) {
            loop {
                match job.status.as_str() {
                    "done" => {
                        let done = job.recipe.ok_or_else(|| {
                            Error::Decode("a finished video job without its recipe".into())
                        })?;
                        return Ok(Imported {
                            recipe: self.recipe(done.id).await?,
                            is_new: done.is_new,
                            from_video: true,
                            dropped_photo: false,
                            cookbook: None,
                        });
                    }
                    "failed" => {
                        return Err(Error::Api {
                            status: job.status_code.unwrap_or(422),
                            message: job
                                .message
                                .filter(|m| !m.is_empty())
                                .unwrap_or_else(|| "Couldn't read that video".into()),
                            code: None,
                            site: None,
                        });
                    }
                    status => progress(&crumb_core::add::job_progress(status, job.position)),
                }
                tokio::time::sleep(JOB_POLL).await;
                job = self.import_job(&job_id).await?;
            }
        }
        let id = imported
            .id
            .and_then(|id| id.as_i64())
            .ok_or_else(|| Error::Decode("an import without a recipe id".into()))?;
        // A shared-cookbook link saves a whole book, not one recipe. When it added no
        // recipes, `id` is the cookbook's id, which is not a recipe id: report it rather
        // than fetch whichever unrelated recipe happens to share that number.
        if imported
            .cookbook
            .as_ref()
            .is_some_and(|book| book.added == 0)
        {
            return Err(Error::Api {
                status: 200,
                message: "That shared cookbook had no new recipes to save.".into(),
                code: None,
                site: None,
            });
        }
        Ok(Imported {
            recipe: self.recipe(id).await?,
            is_new: imported.is_new,
            from_video: false,
            dropped_photo: imported.dropped_photo,
            cookbook: imported.cookbook,
        })
    }

    /// `GET /api/import/jobs/{id}`: where a cooking video's import is.
    pub async fn import_job(&self, id: &str) -> Result<ImportJob, Error> {
        let mut url = self.endpoint("api/import/jobs/");
        url.push_str(&percent_encode_segment(id));
        self.fetch(self.http.get(url)).await
    }

    /// `POST /api/recipes/{id}/viewed` (204 on success).
    pub async fn viewed(&self, id: i64) -> Result<(), Error> {
        let res = self
            .send(
                self.http
                    .post(self.endpoint(&format!("api/recipes/{id}/viewed"))),
            )
            .await?;
        self.expect_ok(res, false).await
    }

    /// `POST /api/recipes/{id}/cooked`: the new stats, and the event [`Client::undo_cooked`]
    /// takes (None when this cook was already logged).
    pub async fn cooked(&self, id: i64) -> Result<Cooked, Error> {
        let res = self
            .send(
                self.http
                    .post(self.endpoint(&format!("api/recipes/{id}/cooked"))),
            )
            .await?;
        self.json(res, false).await
    }

    /// `GET /api/cookbooks`.
    pub async fn cookbooks(&self) -> Result<Vec<CookbookListItem>, Error> {
        let res = self
            .send(self.http.get(self.endpoint("api/cookbooks")))
            .await?;
        self.json(res, false).await
    }

    /// A sized photo URL: `/img/{id}/{width}?v={fnv1a(image)}`, matching the web.
    pub fn image_url(&self, recipe_id: i64, width: u32, image: &str) -> String {
        format!("{}/img/{recipe_id}/{width}?v={}", self.base, fnv1a(image))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base, path)
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<Response, Error> {
        req.send()
            .await
            .map_err(|err| Error::Network(err.to_string()))
    }

    /// Maps a failed response to an [`Error`], reading the server's error body.
    async fn error_for(&self, res: Response, login: bool) -> Error {
        let status = res.status().as_u16();
        if status == 401 && !login {
            return Error::Unauthorized;
        }
        let body = res.text().await.unwrap_or_default();
        let parsed = serde_json::from_str::<ErrorBody>(&body).ok();
        let code = parsed.as_ref().and_then(|body| body.code.clone());
        let site = parsed.as_ref().and_then(|body| body.site.clone());
        let message = parsed
            .and_then(|body| body.message)
            .filter(|message| !message.is_empty())
            .unwrap_or(body);
        Error::Api {
            status,
            message,
            code,
            site,
        }
    }

    async fn expect_ok(&self, res: Response, login: bool) -> Result<(), Error> {
        if res.status().is_success() {
            Ok(())
        } else {
            Err(self.error_for(res, login).await)
        }
    }

    async fn json<T: DeserializeOwned>(&self, res: Response, login: bool) -> Result<T, Error> {
        if !res.status().is_success() {
            return Err(self.error_for(res, login).await);
        }
        res.json::<T>()
            .await
            .map_err(|err| Error::Decode(err.to_string()))
    }

    /// Sends `req` and decodes a successful JSON answer.
    async fn fetch<T: DeserializeOwned>(&self, req: reqwest::RequestBuilder) -> Result<T, Error> {
        let res = self.send(req).await?;
        self.json(res, false).await
    }

    /// Sends `req` and only checks that it succeeded.
    async fn call(&self, req: reqwest::RequestBuilder) -> Result<(), Error> {
        let res = self.send(req).await?;
        self.expect_ok(res, false).await
    }

    /// Sends `req` and keeps the answer as a file.
    async fn download(&self, req: reqwest::RequestBuilder) -> Result<Download, Error> {
        let res = self.send(req).await?;
        if !res.status().is_success() {
            return Err(self.error_for(res, false).await);
        }
        let header = |name| {
            res.headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(String::from)
        };
        let content_type = header(reqwest::header::CONTENT_TYPE).unwrap_or_default();
        let file_name = header(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|value| attachment_name(&value))
            .unwrap_or_else(|| "crumb-export".into());
        let bytes = res
            .bytes()
            .await
            .map_err(|err| Error::Network(err.to_string()))?
            .to_vec();
        Ok(Download {
            file_name,
            content_type,
            bytes,
        })
    }
}

/// A saved [`Client::session_cookie`] as a `Cookie` header value, for code that makes its
/// own requests to the server (like the desktop app's photo loader).
pub fn session_header(saved: &str) -> String {
    let (name, value) = split_session(saved);
    format!("{name}={value}")
}

fn is_hosted_cookie(name: &str) -> bool {
    name == HOSTED_COOKIE || name.strip_prefix(SECURE_PREFIX) == Some(HOSTED_COOKIE)
}

/// A saved session as cookie name and value: `name=value` for the hosted edition's
/// cookie, anything else a bare `crumb_session` value (which may itself contain `=`).
fn split_session(saved: &str) -> (&str, &str) {
    match saved.split_once('=') {
        Some((name, value)) if is_hosted_cookie(name) => (name, value),
        _ => (SESSION_COOKIE, saved),
    }
}

/// The file name in a `Content-Disposition` header: the UTF-8 `filename*` when there is
/// one (RFC 6266, as `api::attachment` writes it), else the plain `filename`.
fn attachment_name(header: &str) -> Option<String> {
    let param = |key: &str| {
        header.split(';').find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            name.eq_ignore_ascii_case(key).then(|| value.trim())
        })
    };
    if let Some(name) = param("filename*")
        .and_then(|v| v.strip_prefix("UTF-8''"))
        .and_then(percent_decode)
    {
        return Some(name);
    }
    param("filename")
        .map(|v| v.trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
}

/// `text` as one URL path segment: anything but unreserved characters percent-encoded.
fn percent_encode_segment(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&b, tail)) = rest.split_first() {
        if b == b'%' {
            let hex = std::str::from_utf8(tail.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(b);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

/// Validates an http(s) base URL and normalizes it to no trailing slash.
fn parse_base(base_url: &str) -> Result<(String, Url), Error> {
    let trimmed = base_url.trim();
    let url = Url::parse(trimmed).map_err(|_| Error::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::InvalidUrl);
    }
    Ok((trimmed.trim_end_matches('/').to_string(), url))
}

/// FNV-1a 32-bit over UTF-8 bytes, as 8 lowercase hex digits. A direct port of
/// `imageKey` in `web/src/lib/img.ts`, so the client and server agree on cache keys.
fn fnv1a(value: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in value.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_answers() {
        let recipe: ImportResponse =
            serde_json::from_str(r#"{"id":7,"title":"Pie","isNew":true,"droppedPhoto":true}"#)
                .unwrap();
        assert_eq!(recipe.id.and_then(|v| v.as_i64()), Some(7));
        assert!(recipe.dropped_photo && recipe.job_id.is_none());

        let job: ImportResponse =
            serde_json::from_str(r#"{"id":"ab12","status":"queued","position":2,"jobId":"ab12"}"#)
                .unwrap();
        assert_eq!(job.job_id.as_deref(), Some("ab12"));
        let job = job.job.unwrap();
        assert_eq!((job.status.as_str(), job.position), ("queued", Some(2)));

        let book: ImportResponse = serde_json::from_str(
            r#"{"id":3,"title":"Weeknights","isNew":true,"cookbook":{"id":9,"name":"Weeknights","added":2,"duplicates":1,"skipped":0}}"#,
        )
        .unwrap();
        assert_eq!(book.cookbook.unwrap().added, 2);
    }

    #[test]
    fn saved_sessions_as_headers() {
        assert_eq!(session_header("abc.d=="), "crumb_session=abc.d==");
        assert_eq!(
            session_header("__Secure-crumb.session_token=x.y"),
            "__Secure-crumb.session_token=x.y"
        );
    }

    #[test]
    fn saved_sessions_name_their_cookie() {
        assert_eq!(split_session("abc.def"), ("crumb_session", "abc.def"));
        assert_eq!(split_session("abc.d=="), ("crumb_session", "abc.d=="));
        assert_eq!(
            split_session("crumb.session_token=abc.d%3D"),
            ("crumb.session_token", "abc.d%3D")
        );
        assert_eq!(
            split_session("__Secure-crumb.session_token=abc"),
            ("__Secure-crumb.session_token", "abc")
        );
    }

    #[test]
    fn job_ids_are_one_segment() {
        assert_eq!(percent_encode_segment("a-b_c.1"), "a-b_c.1");
        assert_eq!(percent_encode_segment("a/b ?"), "a%2Fb%20%3F");
    }
}
