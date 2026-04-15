use thiserror::Error;

/// All errors produced by the IIIF server, mapped to HTTP status codes.
#[derive(Debug, Error)]
pub enum IiifError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IiifError {
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::NotImplemented(_) => 501,
            Self::ServiceUnavailable(_) => 503,
            Self::Internal(_) | Self::ImageProcessing(_) | Self::Storage(_) | Self::Io(_) => 500,
        }
    }
}
