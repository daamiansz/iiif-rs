//! Integration tests for the IIIF Presentation API 3.0 router.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

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

fn build_app() -> axum::Router {
    let state = AppState {
        config: Arc::new(AppConfig::default()),
        storage: Arc::new(StubStorage),
    };
    iiif_presentation::router().with_state(state)
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn range_endpoint_returns_typed_404_json() {
    let app = build_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/range/anything/r1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["status"], 404);
    assert!(body["error"].as_str().unwrap().contains("Range not found"));
}

#[tokio::test]
async fn unsupported_accept_returns_406() {
    let app = build_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/collection/top")
                .header("accept", "text/plain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn collection_unknown_id_404() {
    let app = build_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/collection/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn canvas_with_unknown_cid_404() {
    let app = build_app();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/canvas/anything/p999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
