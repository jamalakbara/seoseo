use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use common::AppError;
use serde::Serialize;
use utoipa::ToSchema;

/// RFC 9457 problem details.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub title: &'static str,
    pub status: u16,
    /// Human/agent readable explanation; often tells you what to do next.
    pub detail: String,
}

pub struct ApiError(pub AppError);

impl<E: Into<AppError>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, title) = match &self.0 {
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad-request", "Bad request"),
            AppError::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation",
                "Invalid input",
            ),
            AppError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "unauthorized", "Sign in required")
            }
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", "Not allowed"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not-found", "Not found"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "conflict", "Conflict"),
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "Internal error",
                )
            }
        };
        let detail = match &self.0 {
            AppError::Internal(_) => {
                "something went wrong on the server; try again or check the logs".into()
            }
            other => other.to_string(),
        };
        let body = Problem {
            kind,
            title,
            status: status.as_u16(),
            detail,
        };
        let mut res = (status, Json(body)).into_response();
        res.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        res
    }
}
