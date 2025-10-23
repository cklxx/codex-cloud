use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("unauthorized: {0}")]
    Unauthorized(Cow<'static, str>),
    #[error("forbidden: {0}")]
    Forbidden(Cow<'static, str>),
    #[error("not found: {0}")]
    NotFound(Cow<'static, str>),
    #[error("conflict: {0}")]
    Conflict(Cow<'static, str>),
    #[error("bad request: {0}")]
    BadRequest(Cow<'static, str>),
    #[error("hashing error: {0}")]
    Hash(#[from] bcrypt::BcryptError),
    #[error("token error: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

impl AppError {
    pub fn unauthorized(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn forbidden(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::BadRequest(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Database(_) | Self::Hash(_) | Self::Io(_) | Self::Http(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message.to_string()),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, message.to_string()),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message.to_string()),
            Self::Conflict(message) => (StatusCode::CONFLICT, message.to_string()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.to_string()),
            Self::Token(err) => (StatusCode::UNAUTHORIZED, err.to_string()),
        };

        let body = Json(json!({ "detail": message }));
        (status, body).into_response()
    }
}
