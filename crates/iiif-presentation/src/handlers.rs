use axum::extract::{Path, State};
use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE, VARY,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tracing::{error, info, warn};

use iiif_core::error::IiifError;
use iiif_core::state::AppState;

use crate::builder;
use crate::sidecar::Sidecar;
use crate::types::Standalone;

const CONTENT_TYPE_JSONLD: &str =
    "application/ld+json;profile=\"http://iiif.io/api/presentation/3/context.json\"";
const CONTENT_TYPE_JSON: &str = "application/json";

/// Content-negotiation outcome for Presentation responses.
enum NegotiatedType {
    JsonLd,
    Json,
}

/// Per IIIF Presentation 3 §6.1: default to `application/ld+json` with the
/// profile parameter. If the client explicitly accepts only `application/json`
/// (and not `ld+json` / `*/*`), serve that instead. If the Accept header is
/// present but accepts neither — return 406 from the caller.
fn negotiate(headers: &HeaderMap) -> Result<NegotiatedType, IiifError> {
    let accept = headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if accept.is_empty()
        || accept.contains("application/ld+json")
        || accept.contains("*/*")
    {
        return Ok(NegotiatedType::JsonLd);
    }
    if accept.contains("application/json") {
        return Ok(NegotiatedType::Json);
    }
    Err(IiifError::NotAcceptable(format!(
        "Unsupported Accept header: `{accept}`. \
         Supported: application/ld+json, application/json."
    )))
}

/// Build the axum router for IIIF Presentation API 3.0 endpoints.
///
/// Child resources (Canvas/AnnotationPage/Annotation/Range) are dereferenceable
/// via the routes mintead by `build_manifest_for_image`. Range is registered
/// for completeness but auto-generated manifests carry no `structures`, so it
/// always 404s — that's the correct behaviour for a non-existent Range.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manifest/{id}", get(manifest_handler))
        .route("/collection/{id}", get(collection_handler))
        .route("/canvas/{id}/{cid}", get(canvas_handler))
        .route("/annotation-page/{id}/{pid}", get(annotation_page_handler))
        .route("/annotation/{id}/{aid}", get(annotation_handler))
        .route("/range/{id}/{rid}", get(range_handler))
}

async fn read_dimensions(
    state: &AppState,
    id: &str,
) -> Result<(u32, u32), IiifError> {
    let bytes = state.storage.read_image(id).await.map_err(|e| {
        warn!(identifier = %id, error = %e, "Image not found");
        e
    })?;
    tokio::task::spawn_blocking(move || {
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| IiifError::ImageProcessing(format!("Failed to guess format: {e}")))?;
        reader
            .into_dimensions()
            .map_err(|e| IiifError::ImageProcessing(format!("Failed to read dimensions: {e}")))
    })
    .await
    .map_err(|e| IiifError::Internal(format!("Task join error: {e}")))?
}

fn is_protected(state: &AppState, id: &str) -> bool {
    if !state.config.auth.enabled {
        return false;
    }
    state
        .storage
        .containing_directory(id)
        .map(|dir| state.config.auth.protected_dirs.iter().any(|p| p == &dir))
        .unwrap_or(false)
}

async fn load_sidecar(state: &AppState, id: &str) -> Option<Sidecar> {
    state
        .storage
        .read_sidecar(id)
        .await
        .and_then(|bytes| Sidecar::from_toml_bytes(&bytes))
}

async fn manifest_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let neg = negotiate(&req_headers)?;
    let (width, height) = read_dimensions(&state, &id).await?;
    let sidecar = load_sidecar(&state, &id).await;
    let manifest = builder::build_manifest_for_image(
        &id,
        width,
        height,
        &state.config,
        is_protected(&state, &id),
        sidecar.as_ref(),
    );
    let json = serde_json::to_string(&manifest)
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;
    info!(identifier = %id, "Served manifest");
    Ok((StatusCode::OK, response_headers(neg), json).into_response())
}

async fn collection_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let neg = negotiate(&req_headers)?;
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
    info!(collection = %id, count = images.len(), "Served collection");
    Ok((StatusCode::OK, response_headers(neg), json).into_response())
}

/// GET `/canvas/{id}/{cid}` — standalone Canvas. Auto-generated manifests
/// only ever mint `cid = "p1"`, so other values 404.
async fn canvas_handler(
    State(state): State<AppState>,
    Path((id, cid)): Path<(String, String)>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let neg = negotiate(&req_headers)?;
    if cid != "p1" {
        return Err(IiifError::NotFound(format!(
            "Canvas not found: {id}/{cid}"
        )));
    }
    let (width, height) = read_dimensions(&state, &id).await?;
    let sidecar = load_sidecar(&state, &id).await;
    let manifest = builder::build_manifest_for_image(
        &id,
        width,
        height,
        &state.config,
        is_protected(&state, &id),
        sidecar.as_ref(),
    );
    let canvas = manifest
        .items
        .into_iter()
        .next()
        .ok_or_else(|| IiifError::Internal("Manifest produced no canvases".to_string()))?;
    let json = serde_json::to_string(&Standalone::new(canvas))
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;
    info!(identifier = %id, canvas = %cid, "Served canvas");
    Ok((StatusCode::OK, response_headers(neg), json).into_response())
}

/// GET `/annotation-page/{id}/{pid}` — standalone AnnotationPage.
async fn annotation_page_handler(
    State(state): State<AppState>,
    Path((id, pid)): Path<(String, String)>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let neg = negotiate(&req_headers)?;
    if pid != "p1" {
        return Err(IiifError::NotFound(format!(
            "AnnotationPage not found: {id}/{pid}"
        )));
    }
    let (width, height) = read_dimensions(&state, &id).await?;
    let sidecar = load_sidecar(&state, &id).await;
    let manifest = builder::build_manifest_for_image(
        &id,
        width,
        height,
        &state.config,
        is_protected(&state, &id),
        sidecar.as_ref(),
    );
    let page = manifest
        .items
        .into_iter()
        .next()
        .and_then(|c| c.items)
        .and_then(|pages| pages.into_iter().next())
        .ok_or_else(|| IiifError::Internal("Manifest produced no annotation pages".to_string()))?;
    let json = serde_json::to_string(&Standalone::new(page))
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;
    info!(identifier = %id, page = %pid, "Served annotation page");
    Ok((StatusCode::OK, response_headers(neg), json).into_response())
}

/// GET `/annotation/{id}/{aid}` — standalone Annotation.
async fn annotation_handler(
    State(state): State<AppState>,
    Path((id, aid)): Path<(String, String)>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let neg = negotiate(&req_headers)?;
    if aid != "p1-image" {
        return Err(IiifError::NotFound(format!(
            "Annotation not found: {id}/{aid}"
        )));
    }
    let (width, height) = read_dimensions(&state, &id).await?;
    let sidecar = load_sidecar(&state, &id).await;
    let manifest = builder::build_manifest_for_image(
        &id,
        width,
        height,
        &state.config,
        is_protected(&state, &id),
        sidecar.as_ref(),
    );
    let annotation = manifest
        .items
        .into_iter()
        .next()
        .and_then(|c| c.items)
        .and_then(|pages| pages.into_iter().next())
        .and_then(|p| p.items)
        .and_then(|annos| annos.into_iter().next())
        .ok_or_else(|| IiifError::Internal("Manifest produced no annotations".to_string()))?;
    let json = serde_json::to_string(&Standalone::new(annotation))
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;
    info!(identifier = %id, annotation = %aid, "Served annotation");
    Ok((StatusCode::OK, response_headers(neg), json).into_response())
}

/// GET `/range/{id}/{rid}` — auto-generated manifests have no `structures`,
/// so every Range request 404s. Registered so the path is reserved and
/// returns a typed not-found body rather than the default 404 page.
async fn range_handler(
    Path((id, rid)): Path<(String, String)>,
) -> Result<Response, IiifError> {
    Err(IiifError::NotFound(format!("Range not found: {id}/{rid}")))
}

fn response_headers(negotiated: NegotiatedType) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let ct = match negotiated {
        NegotiatedType::JsonLd => CONTENT_TYPE_JSONLD,
        NegotiatedType::Json => CONTENT_TYPE_JSON,
    };
    headers.insert(CONTENT_TYPE, ct.parse().expect("valid header value"));
    headers.insert(
        ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("valid header value"),
    );
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=3600".parse().expect("valid header value"),
    );
    // Caches must key on Accept since we vary the body by content type.
    headers.insert(VARY, "accept".parse().expect("valid header value"));
    headers
}
