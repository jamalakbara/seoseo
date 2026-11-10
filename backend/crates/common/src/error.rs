/// Application error. The API layer maps each variant to an RFC 9457 problem response.
/// Messages are user/agent facing: phrase them as instructions where possible.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("sign in required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn internal<E: Into<anyhow::Error>>(e: E) -> Self {
        AppError::Internal(e.into())
    }
}
