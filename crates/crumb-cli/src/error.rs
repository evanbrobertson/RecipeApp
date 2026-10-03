//! What went wrong, as the stable exit code and the words that go with it.

use crumb_client::Error;
use serde_json::{Value, json};

pub const USAGE: i32 = 2;
pub const NOT_SIGNED_IN: i32 = 3;
pub const BLOCKED: i32 = 4;
pub const NOT_FOUND: i32 = 5;
pub const NETWORK: i32 = 6;
pub const RATE_LIMITED: i32 = 7;

#[derive(Debug)]
pub struct Failure {
    pub code: i32,
    pub status: Option<u16>,
    pub reason: Option<String>,
    pub site: Option<String>,
    pub message: String,
    /// One line saying what to do about it.
    pub hint: Option<String>,
}

impl Failure {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::plain(USAGE, message, None)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::plain(NOT_FOUND, message, None)
    }

    pub fn plain(code: i32, message: impl Into<String>, hint: Option<&str>) -> Self {
        Self {
            code,
            status: None,
            reason: None,
            site: None,
            message: message.into(),
            hint: hint.map(String::from),
        }
    }

    /// `{"error": {statusCode, code, site, message}}`, for `--json`.
    pub fn json(&self) -> Value {
        let mut error = serde_json::Map::new();
        if let Some(status) = self.status {
            error.insert("statusCode".into(), json!(status));
        }
        if let Some(reason) = &self.reason {
            error.insert("code".into(), json!(reason));
        }
        if let Some(site) = &self.site {
            error.insert("site".into(), json!(site));
        }
        error.insert("message".into(), json!(self.message));
        if let Some(hint) = &self.hint {
            error.insert("hint".into(), json!(hint));
        }
        json!({ "error": error })
    }
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        match &err {
            Error::InvalidUrl => Self::usage("That isn't an http or https address"),
            Error::Unauthorized => Self::plain(
                NOT_SIGNED_IN,
                "Not signed in, or the token was revoked or has expired",
                Some("Make a token on the Crumb account page and run: crumb login --token <token>"),
            ),
            Error::Network(message) => Self::plain(
                NETWORK,
                message.clone(),
                Some("Check the server address with: crumb doctor"),
            ),
            Error::Decode(message) => Self::plain(1, message.clone(), None),
            Error::Api {
                status,
                message,
                code,
                site,
            } => {
                let (exit, hint) = match (*status, code.as_deref()) {
                    (_, Some("site_blocked")) => (
                        BLOCKED,
                        Some(
                            "The site turned the server away. Open the recipe in a browser with the Crumb extension",
                        ),
                    ),
                    (_, Some("site_terms")) => (BLOCKED, None),
                    (401 | 403, _) => (NOT_SIGNED_IN, None),
                    (404, _) => (NOT_FOUND, None),
                    (429, _) => (RATE_LIMITED, Some("Try again in a little while")),
                    _ => (1, None),
                };
                Self {
                    code: exit,
                    status: Some(*status),
                    reason: code.clone(),
                    site: site.clone(),
                    message: message.clone(),
                    hint: hint.map(String::from),
                }
            }
        }
    }
}
