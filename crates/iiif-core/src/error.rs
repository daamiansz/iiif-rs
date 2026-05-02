use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
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

    #[error("Not acceptable: {0}")]
    NotAcceptable(String),

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
    /// HTTP status code for this error variant.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::NotAcceptable(_) => StatusCode::NOT_ACCEPTABLE,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) | Self::ImageProcessing(_) | Self::Storage(_) | Self::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Numeric HTTP status code (kept for backward compatibility).
    pub fn http_status_code(&self) -> u16 {
        self.status_code().as_u16()
    }
}

impl IntoResponse for IiifError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = serde_json::json!({
            "error": self.to_string(),
            "status": status.as_u16(),
        });

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/json".parse().expect("valid header value"),
        );
        headers.insert(
            ACCESS_CONTROL_ALLOW_ORIGIN,
            "*".parse().expect("valid header value"),
        );

        (status, headers, body.to_string()).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_map_correctly() {
        assert_eq!(IiifError::BadRequest("x".into()).status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(IiifError::NotFound("x".into()).status_code(), StatusCode::NOT_FOUND);
        assert_eq!(
            IiifError::NotAcceptable("x".into()).status_code(),
            StatusCode::NOT_ACCEPTABLE
        );
        assert_eq!(
            IiifError::NotImplemented("x".into()).status_code(),
            StatusCode::NOT_IMPLEMENTED
        );
        assert_eq!(
            IiifError::ServiceUnavailable("x".into()).status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            IiifError::Internal("x".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn into_response_emits_json_with_status() {
        let resp = IiifError::BadRequest("oops".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.headers().get(CONTENT_TYPE).unwrap(),
            "application/json"
        );

        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], 400);
        assert!(body["error"].as_str().unwrap().contains("oops"));
    }
}
