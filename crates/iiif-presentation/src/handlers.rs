use axum::extract::{Path, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tracing::{error, info, warn};

use iiif_core::error::IiifError;
use iiif_core::state::AppState;

use crate::builder;

const CONTENT_TYPE_JSONLD: &str =
    "application/ld+json;profile=\"http://iiif.io/api/presentation/3/context.json\"";

/// Build the axum router for IIIF Presentation API 3.0 endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manifest/{id}", get(manifest_handler))
        .route("/collection/{id}", get(collection_handler))
}

/// GET `/manifest/{id}` — serve a Manifest for the given image identifier.
async fn manifest_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, IiifError> {
    let bytes = state.storage.read_image(&id).await.map_err(|e| {
        warn!(identifier = %id, error = %e, "Image not found for manifest");
        e
    })?;
    let (width, height) = tokio::task::spawn_blocking(move || {
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| IiifError::ImageProcessing(format!("Failed to guess format: {e}")))?;
        reader
            .into_dimensions()
            .map_err(|e| IiifError::ImageProcessing(format!("Failed to read dimensions: {e}")))
    })
    .await
    .map_err(|e| IiifError::Internal(format!("Task join error: {e}")))??;

    let is_protected = if state.config.auth.enabled {
        state
            .storage
            .containing_directory(&id)
            .map(|dir| state.config.auth.protected_dirs.iter().any(|p| p == &dir))
            .unwrap_or(false)
    } else {
        false
    };

    let manifest =
        builder::build_manifest_for_image(&id, width, height, &state.config, is_protected);

    let json = serde_json::to_string(&manifest)
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;

    let headers = response_headers();
    info!(identifier = %id, "Served manifest");
    Ok((StatusCode::OK, headers, json).into_response())
}

/// GET `/collection/{id}` — serve a Collection.
///
/// `id = "top"` returns the root collection listing all images.
async fn collection_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, IiifError> {
    if id != "top" {
        return Err(IiifError::NotFound(format!("Collection not found: {id}")));
    }

    let images = builder::scan_images(state.storage.as_ref(), &state.config.storage.root_path)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to scan images");
            e
        })?;

    let collection = builder::build_root_collection(&images, &state.config);

    let json = serde_json::to_string(&collection)
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;

    let headers = response_headers();
    info!(collection = %id, count = images.len(), "Served collection");
    Ok((StatusCode::OK, headers, json).into_response())
}

fn response_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        CONTENT_TYPE_JSONLD.parse().expect("valid header value"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("valid header value"),
    );
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=3600".parse().expect("valid header value"),
    );
    headers
}

