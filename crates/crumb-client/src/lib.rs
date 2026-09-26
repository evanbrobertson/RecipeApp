//! A typed HTTP client for the Crumb REST API. No UI, no Qt: just the routes the
//! desktop app needs, over `reqwest`, with a cookie jar for the app-password session.

use std::sync::Arc;

use reqwest::cookie::{CookieStore, Jar};
use reqwest::{Client as HttpClient, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use url::Url;

pub use crumb_core::model::{CookbookListItem, Recipe, RecipeSummary, Section};

mod error;
pub use error::Error;

/// Name of the session cookie the server sets (`src/auth.rs`). The desktop app keeps
/// this value in the keyring so a signed-in session survives a restart.
pub const SESSION_COOKIE: &str = "crumb_session";

/// What to save: a link or pasted text, sent to `POST /api/recipes/import`.
#[derive(Debug, Clone)]
pub enum ImportInput {
    Url(String),
    Text(String),
}

/// A completed import: the saved recipe and whether it was new (the API's `isNew`).
#[derive(Debug, Clone)]
pub struct Imported {
    pub recipe: Recipe,
    pub is_new: bool,
}

/// The `{statusCode, statusMessage, message}` body the server sends on errors.
#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

/// The `{id, title, isNew}` body the import route echoes back. A shared-cookbook link
/// adds a `cookbook` object instead of being a single recipe.
#[derive(Deserialize)]
struct ImportResponse {
    id: i64,
    #[serde(rename = "isNew")]
    is_new: bool,
    cookbook: Option<ImportedBook>,
}

/// How many recipes a shared-cookbook import actually added.
#[derive(Deserialize)]
struct ImportedBook {
    added: usize,
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

    /// Restores a session from a previously saved [`Client::session_cookie`] value.
    pub fn with_session(base_url: &str, cookie: &str) -> Result<Self, Error> {
        let client = Self::new(base_url)?;
        client.set_session(cookie);
        Ok(client)
    }

    fn set_session(&self, cookie: &str) {
        self.jar
            .add_cookie_str(&format!("{SESSION_COOKIE}={cookie}; Path=/"), &self.url);
    }

    /// The current `crumb_session` value, to persist in the keyring.
    pub fn session_cookie(&self) -> Option<String> {
        let header = self.jar.cookies(&self.url)?;
        let cookies = header.to_str().ok()?;
        cookies.split(';').find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == SESSION_COOKIE).then(|| value.to_string())
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

    /// `POST /api/recipes/import`. The route echoes only `id`/`title`/`isNew`, so the
    /// full recipe is fetched straight after.
    pub async fn import(&self, input: ImportInput) -> Result<Imported, Error> {
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
        // A shared-cookbook link saves a whole book, not one recipe. When it added no
        // recipes, `id` is the cookbook's id, which is not a recipe id: report it rather
        // than fetch whichever unrelated recipe happens to share that number.
        if imported.cookbook.is_some_and(|book| book.added == 0) {
            return Err(Error::Api {
                status: 200,
                message: "That shared cookbook had no new recipes to save.".into(),
            });
        }
        let recipe = self.recipe(imported.id).await?;
        Ok(Imported {
            recipe,
            is_new: imported.is_new,
        })
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

    /// `POST /api/recipes/{id}/cooked`.
    pub async fn cooked(&self, id: i64) -> Result<(), Error> {
        let res = self
            .send(
                self.http
                    .post(self.endpoint(&format!("api/recipes/{id}/cooked"))),
            )
            .await?;
        self.expect_ok(res, false).await
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
        let message = serde_json::from_str::<ErrorBody>(&body)
            .ok()
            .and_then(|body| body.message)
            .filter(|message| !message.is_empty())
            .unwrap_or(body);
        Error::Api { status, message }
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
