use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An error that becomes an h3-style JSON body:
/// `{"statusCode":404,"statusMessage":"Recipe not found","message":"Recipe not found"}`.
#[derive(Debug, Clone)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: message.into(),
        }
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

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = json!({
            "statusCode": self.status.as_u16(),
            "statusMessage": self.message,
            "message": self.message,
        });
        (self.status, Json(body)).into_response()
    }
}
