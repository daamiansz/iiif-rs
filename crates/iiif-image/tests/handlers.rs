//! Integration tests for the IIIF Image API 3.0 router.
//!
//! Covers the response shapes that don't require decoding a real source image:
//! - base URI 303 redirect to info.json
//! - identifier validation (UTF-8, path traversal)
//! - parameter parse errors (400 vs 501)

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use async_trait::async_trait;
use iiif_core::config::AppConfig;
use iiif_core::error::IiifError;
use iiif_core::state::AppState;
use iiif_core::storage::ImageStorage;

struct StubStorage;

#[async_trait]
impl ImageStorage for StubStorage {
    async fn exists(&self, _id: &str) -> Result<bool, IiifError> {
        Ok(false)
    }
    async fn read_image(&self, _id: &str) -> Result<Vec<u8>, IiifError> {
        Err(IiifError::NotFound("stub".into()))
    }
    async fn resolve_path(&self, _id: &str) -> Result<std::path::PathBuf, IiifError> {
        Err(IiifError::NotFound("stub".into()))
    }
    async fn last_modified(&self, _id: &str) -> Result<std::time::SystemTime, IiifError> {
        Err(IiifError::NotFound("stub".into()))
    }
    fn containing_directory(&self, _id: &str) -> Option<String> {
        None
    }
}

fn build_state() -> AppState {
    AppState {
        config: Arc::new(AppConfig::default()),
        storage: Arc::new(StubStorage),
    }
}

#[tokio::test]
async fn base_uri_redirects_303_to_info_json() {
    let app = iiif_image::router().with_state(build_state());

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/some-image")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get("location")
        .expect("Location header")
        .to_str()
        .unwrap();
    assert_eq!(location, "http://localhost:8080/some-image/info.json");
}

#[tokio::test]
async fn invalid_region_returns_400() {
    let app = iiif_image::router().with_state(build_state());

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/img/notaregion/max/0/default.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn path_traversal_identifier_returns_400() {
    let app = iiif_image::router().with_state(build_state());

    // `..%2Fpasswd` decodes to `../passwd` and must be rejected.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/..%2Fpasswd/info.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_utf8_percent_sequence_returns_400() {
    let app = iiif_image::router().with_state(build_state());

    // `%C3` alone is an incomplete UTF-8 sequence.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bad%C3/info.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
