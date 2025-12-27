//! API error types and response formatting.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for API operations.
pub type ApiResult<T> = Result<T, ApiError>;

/// API error type with HTTP status code mapping.
#[derive(Error, Debug)]
pub enum ApiError {
    /// Authentication failed (401)
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    /// Access denied (403)
    #[error("Access denied: {0}")]
    Forbidden(String),

    /// Resource not found (404)
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid request (400)
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Conflict (409)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Validation error (422)
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Internal server error (500)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Service unavailable (503)
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Job execution error
    #[error("Job execution failed: {0}")]
    JobExecution(String),

    /// Inventory error
    #[error("Inventory error: {0}")]
    Inventory(String),

    /// Playbook error
    #[error("Playbook error: {0}")]
    Playbook(String),
}

impl ApiError {
    /// Get the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::JobExecution(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Inventory(_) => StatusCode::BAD_REQUEST,
            ApiError::Playbook(_) => StatusCode::BAD_REQUEST,
        }
    }

    /// Get the error code for machine parsing.
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::Unauthorized(_) => "UNAUTHORIZED",
            ApiError::Forbidden(_) => "FORBIDDEN",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::Conflict(_) => "CONFLICT",
            ApiError::ValidationError(_) => "VALIDATION_ERROR",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            ApiError::JobExecution(_) => "JOB_EXECUTION_ERROR",
            ApiError::Inventory(_) => "INVENTORY_ERROR",
            ApiError::Playbook(_) => "PLAYBOOK_ERROR",
        }
    }
}

/// Error response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code for machine parsing
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// Additional details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ErrorResponse {
            error: self.error_code().to_string(),
            message: self.to_string(),
            details: None,
        };

        (status, Json(body)).into_response()
    }
}

impl From<crate::error::Error> for ApiError {
    fn from(err: crate::error::Error) -> Self {
        match err {
            crate::error::Error::HostNotFound(h) => ApiError::NotFound(format!("Host not found: {}", h)),
            crate::error::Error::GroupNotFound(g) => ApiError::NotFound(format!("Group not found: {}", g)),
            crate::error::Error::PlaybookParse { path, message, .. } => {
                ApiError::Playbook(format!("Failed to parse {}: {}", path.display(), message))
            }
            crate::error::Error::PlaybookValidation(msg) => ApiError::Playbook(msg),
            crate::error::Error::InventoryLoad { path, message } => {
                ApiError::Inventory(format!("Failed to load {}: {}", path.display(), message))
            }
            _ => ApiError::Internal(err.to_string()),
        }
    }
}

impl From<crate::inventory::InventoryError> for ApiError {
    fn from(err: crate::inventory::InventoryError) -> Self {
        match err {
            crate::inventory::InventoryError::HostNotFound(h) => {
                ApiError::NotFound(format!("Host not found: {}", h))
            }
            crate::inventory::InventoryError::GroupNotFound(g) => {
                ApiError::NotFound(format!("Group not found: {}", g))
            }
            _ => ApiError::Inventory(err.to_string()),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::BadRequest(format!("JSON error: {}", err))
    }
}

impl From<jsonwebtoken::errors::Error> for ApiError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        ApiError::Unauthorized(format!("Invalid token: {}", err))
    }
}
