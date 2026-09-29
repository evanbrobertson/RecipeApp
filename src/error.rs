use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An error that becomes an h3-style JSON body:
/// `{"statusCode":404,"statusMessage":"Recipe not found","message":"Recipe not found"}`.
/// An error a client can act on also carries a `code` (`"site_blocked"`, `"site_terms"`); the field is
/// left out otherwise. A `site_terms` error also carries `site`, the listed site's display name
/// ("Allrecipes"), so a client can say whose terms without guessing from the link's host.
#[derive(Debug, Clone, PartialEq)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
    pub code: Option<&'static str>,
    /// With `site_terms`: the site's display name.
    pub site: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: message.into(),
            code: None,
            site: None,
        }
    }

    pub fn with_site(mut self, site: impl Into<String>) -> Self {
        self.site = Some(site.into());
        self
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!("internal error: {err}");
        Self::new(500, "Something went wrong")
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::internal(err)
    }
}

impl From<crumb_core::error::ValidationError> for AppError {
    fn from(err: crumb_core::error::ValidationError) -> Self {
        Self::bad_request(err.0)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut body = json!({
            "statusCode": self.status.as_u16(),
            "statusMessage": self.message,
            "message": self.message,
        });
        if let Some(code) = self.code {
            body["code"] = code.into();
        }
        if let Some(site) = &self.site {
            body["site"] = site.as_str().into();
        }
        (self.status, Json(body)).into_response()
    }
}
