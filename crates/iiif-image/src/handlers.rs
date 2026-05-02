use std::sync::Arc;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use axum::extract::{Extension, Path, State};
use axum::http::header::{
    self, ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE,
    IF_NONE_MATCH, LAST_MODIFIED, LINK, LOCATION,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tracing::{error, info, warn};

use iiif_core::config::ImageConfig;
use iiif_core::error::IiifError;
use iiif_core::identifier::ImageIdentifier;
use iiif_core::state::AppState;

use moka::sync::Cache;

use crate::info::ImageInfo;
use crate::params::{parse_quality_format, OutputFormat, Quality, Region, Rotation, Size};
use crate::pipeline;

/// Type alias for the processed image cache.
pub type ImageCache = Cache<String, Arc<Vec<u8>>>;

const PROFILE_URI: &str = "http://iiif.io/api/image/3/level2.json";

/// Build the axum router for IIIF Image API 3.0 endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{identifier}", get(base_redirect_handler))
        .route("/{identifier}/info.json", get(info_handler))
        .route(
            "/{identifier}/{region}/{size}/{rotation}/{quality_format}",
            get(image_handler),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET `/{identifier}` → 303 redirect to info.json
async fn base_redirect_handler(
    State(state): State<AppState>,
    Path(raw_identifier): Path<String>,
) -> Result<Response, IiifError> {
    let _identifier = ImageIdentifier::from_encoded(&raw_identifier)?;
    let location = format!(
        "{}/{}/info.json",
        state.config.server.base_url, raw_identifier
    );

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, location.parse().expect("valid header value"));
    headers.insert(
        ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("valid header value"),
    );

    info!(identifier = %raw_identifier, "Redirect to info.json");
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

/// GET `/{identifier}/info.json`
async fn info_handler(
    State(state): State<AppState>,
    Path(raw_identifier): Path<String>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let identifier = ImageIdentifier::from_encoded(&raw_identifier)?;
    let id_str = identifier.as_str().to_string();

    // Get last-modified time (async)
    let mtime = state.storage.last_modified(&id_str).await.ok();

    // Compute ETag from identifier + modification time
    let etag = compute_etag(identifier.as_str(), mtime, "info.json");

    // Check conditional request — return 304 if fresh
    if is_not_modified(&req_headers, &etag, mtime) {
        return Ok(not_modified_response(&etag, mtime));
    }

    let bytes = state.storage.read_image(&id_str).await.map_err(|e| {
        warn!(identifier = %identifier, error = %e, "Image not found");
        e
    })?;
    // Header decode is cheap but still CPU; keep on the blocking pool.
    let (width, height) = tokio::task::spawn_blocking(move || pipeline::get_dimensions(&bytes))
        .await
        .map_err(|e| IiifError::Internal(format!("Task join error: {e}")))??;

    // If the resource resides in a protected directory, embed the auth probe
    // service so clients can discover the auth flow.
    let auth_service = if state.config.auth.enabled {
        let in_protected = state
            .storage
            .access_zone(identifier.as_str())
            .map(|dir| state.config.auth.protected_dirs.iter().any(|p| p == &dir))
            .unwrap_or(false);
        if in_protected {
            Some(iiif_auth::build_probe_service_descriptor(
                &state.config.server.base_url,
                identifier.as_str(),
                iiif_auth::AuthPattern::from_config(&state.config.auth.pattern),
            ))
        } else {
            None
        }
    } else {
        None
    };

    let info = ImageInfo::build(
        &state.config.server.base_url,
        identifier.as_str(),
        width,
        height,
        &state.config.image,
        auth_service,
    );

    let json = serde_json::to_string(&info)
        .map_err(|e| IiifError::Internal(format!("JSON serialization error: {e}")))?;

    // Content negotiation: application/ld+json (default) vs application/json
    let accept = req_headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let content_type =
        if accept.contains("application/ld+json") || !accept.contains("application/json") {
            "application/ld+json;profile=\"http://iiif.io/api/image/3/context.json\""
        } else {
            "application/json"
        };

    let mut headers = common_headers(&etag, mtime);
    headers.insert(
        CONTENT_TYPE,
        content_type.parse().expect("valid header value"),
    );
    headers.insert(
        LINK,
        format!("<{PROFILE_URI}>;rel=\"profile\"")
            .parse()
            .expect("valid header value"),
    );

    info!(identifier = %identifier, width, height, "Served info.json");
    Ok((StatusCode::OK, headers, json).into_response())
}

/// GET `/{identifier}/{region}/{size}/{rotation}/{quality}.{format}`
async fn image_handler(
    State(state): State<AppState>,
    image_cache: Option<Extension<Arc<ImageCache>>>,
    Path((raw_identifier, raw_region, raw_size, raw_rotation, raw_quality_format)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
    req_headers: HeaderMap,
) -> Result<Response, IiifError> {
    let identifier = ImageIdentifier::from_encoded(&raw_identifier)?;
    let region: Region = raw_region.parse()?;
    let size: Size = raw_size.parse()?;
    let rotation: Rotation = raw_rotation.parse()?;
    let (quality, format) = parse_quality_format(&raw_quality_format)?;

    let id_str = identifier.as_str().to_string();

    let mtime = state.storage.last_modified(&id_str).await.ok();

    // ETag includes request parameters — different params = different output
    let params_key = format!("{raw_region}/{raw_size}/{raw_rotation}/{raw_quality_format}");
    let etag = compute_etag(identifier.as_str(), mtime, &params_key);

    if is_not_modified(&req_headers, &etag, mtime) {
        return Ok(not_modified_response(&etag, mtime));
    }

    // Build cache key from request params + file mtime
    let cache_key = etag.clone();

    // Memory cache (Extension<Arc<ImageCache>>) is optional — None disables caching.
    let image_cache = image_cache.as_ref().map(|Extension(c)| c.as_ref());

    // 1. Memory cache
    if let Some(cached) = image_cache.and_then(|c| c.get(&cache_key)) {
        tracing::debug!("Memory cache hit: {cache_key}");
        return Ok(build_image_response(
            &cached,
            &etag,
            mtime,
            &format,
            &state,
            &raw_identifier,
            &region,
            &size,
            &rotation,
            &quality,
            None,
        ));
    }

    // 2. Disk tile cache
    let tile_cache_dir = state.config.performance.tile_cache_dir.as_deref();
    if let Some(dir) = tile_cache_dir {
        let disk_path = disk_cache_path(dir, &cache_key, &format);
        let read_result = {
            let path = disk_path.clone();
            tokio::task::spawn_blocking(move || std::fs::read(&path)).await
        };
        if let Ok(Ok(bytes)) = read_result {
            tracing::debug!("Disk cache hit: {}", disk_path.display());
            let bytes = Arc::new(bytes);
            if let Some(cache) = image_cache {
                cache.insert(cache_key.clone(), Arc::clone(&bytes));
            }
            return Ok(build_image_response(
                &bytes,
                &etag,
                mtime,
                &format,
                &state,
                &raw_identifier,
                &region,
                &size,
                &rotation,
                &quality,
                None,
            ));
        }
    }

    let bytes = state.storage.read_image(&id_str).await.map_err(|e| {
        warn!(identifier = %identifier, error = %e, "Image not found");
        e
    })?;
    // Move-and-return — header decode is cheap but kept on the blocking pool
    // for consistency with the heavy pipeline below; we hand the bytes back so
    // we never clone a 5-50 MB Vec.
    let (source, src_w, src_h) = tokio::task::spawn_blocking(move || {
        let (w, h) = pipeline::get_dimensions(&bytes)?;
        Ok::<_, IiifError>((bytes, w, h))
    })
    .await
    .map_err(|e| IiifError::Internal(format!("Task join error: {e}")))??;

    let config = state.config.image.clone();
    let region_c = region.clone();
    let size_c = size.clone();
    let rotation_c = rotation.clone();
    let output = tokio::task::spawn_blocking(move || {
        pipeline::process_image(
            &source,
            &region_c,
            &size_c,
            &rotation_c,
            &quality,
            &format,
            &config,
        )
    })
    .await
    .map_err(|e| IiifError::Internal(format!("Task join error: {e}")))?
    .map_err(|e| {
        error!(identifier = %identifier, error = %e, "Image processing failed");
        e
    })?;

    let output = Arc::new(output);
    if let Some(cache) = image_cache {
        cache.insert(cache_key.clone(), Arc::clone(&output));
    }

    if let Some(dir) = tile_cache_dir {
        let disk_path = disk_cache_path(dir, &cache_key, &format);
        let bytes = Arc::clone(&output);
        let _ = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            if let Some(parent) = disk_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&disk_path, bytes.as_ref())
        })
        .await;
    }

    info!(
        identifier = %identifier,
        region = %raw_region,
        size = %raw_size,
        rotation = %raw_rotation,
        quality_format = %raw_quality_format,
        output_bytes = output.len(),
        cached = false,
        "Served image"
    );

    Ok(build_image_response(
        &output,
        &etag,
        mtime,
        &format,
        &state,
        &raw_identifier,
        &region,
        &size,
        &rotation,
        &quality,
        Some((src_w, src_h)),
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_image_response(
    output: &[u8],
    etag: &str,
    mtime: Option<SystemTime>,
    format: &OutputFormat,
    state: &AppState,
    raw_identifier: &str,
    region: &Region,
    size: &Size,
    rotation: &Rotation,
    quality: &Quality,
    source_dims: Option<(u32, u32)>,
) -> Response {
    let mut headers = common_headers(etag, mtime);
    headers.insert(
        CONTENT_TYPE,
        format.content_type().parse().expect("valid header value"),
    );

    // Canonical Link is MAY per spec — emit only when we have the source
    // dimensions needed to resolve `pct:n` and `!w,h` to absolute pixel sizes.
    // On cache hits we don't decode the source, so we omit the header rather
    // than guess.
    let link_value = match source_dims {
        Some((img_w, img_h)) => {
            let canonical = build_canonical_uri(&CanonicalParams {
                base_url: &state.config.server.base_url,
                identifier: raw_identifier,
                region,
                size,
                rotation,
                quality,
                format,
                img_w,
                img_h,
                config: &state.config.image,
            });
            format!("<{PROFILE_URI}>;rel=\"profile\",<{canonical}>;rel=\"canonical\"")
        }
        None => format!("<{PROFILE_URI}>;rel=\"profile\""),
    };
    headers.insert(LINK, link_value.parse().expect("valid header value"));

    (StatusCode::OK, headers, output.to_vec()).into_response()
}

/// Build path for disk tile cache: `{dir}/{hash}.{ext}`
///
/// Uses SHA-256 truncated to 16 hex chars (64 bits) — deterministic across
/// Rust versions, builds, and platforms, so cache files survive recompiles.
fn disk_cache_path(dir: &str, cache_key: &str, format: &OutputFormat) -> std::path::PathBuf {
    let digest = Sha256::digest(cache_key.as_bytes());
    let hash: String = digest
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    std::path::PathBuf::from(dir).join(format!("{hash}.{format}"))
}

// ---------------------------------------------------------------------------
// HTTP caching helpers
// ---------------------------------------------------------------------------

/// Compute a deterministic ETag from identifier, modification time, and request params.
///
/// Uses SHA-256 truncated to 16 hex chars (64 bits) — deterministic across
/// Rust versions, builds, and platforms.
fn compute_etag(identifier: &str, mtime: Option<SystemTime>, params: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(identifier.as_bytes());
    hasher.update(b"\0");
    if let Some(t) = mtime {
        let secs = t
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        hasher.update(secs.to_le_bytes());
    }
    hasher.update(b"\0");
    hasher.update(params.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("\"{hex}\"")
}

/// Check If-None-Match and If-Modified-Since against the current ETag/mtime.
fn is_not_modified(req_headers: &HeaderMap, etag: &str, mtime: Option<SystemTime>) -> bool {
    // If-None-Match takes priority per HTTP spec
    if let Some(inm) = req_headers.get(IF_NONE_MATCH) {
        if let Ok(value) = inm.to_str() {
            if value == etag || value == "*" {
                return true;
            }
            // Handle comma-separated list of ETags
            if value.split(',').any(|t| t.trim() == etag) {
                return true;
            }
        }
    }

    // If-Modified-Since
    if let (Some(ims), Some(mtime)) = (req_headers.get(IF_MODIFIED_SINCE), mtime) {
        if let Ok(value) = ims.to_str() {
            if let Ok(since) = httpdate::parse_http_date(value) {
                return mtime <= since;
            }
        }
    }

    false
}

/// Build a 304 Not Modified response with caching headers.
fn not_modified_response(etag: &str, mtime: Option<SystemTime>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(ETAG, etag.parse().expect("valid header value"));
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=86400".parse().expect("valid header value"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("valid header value"),
    );
    if let Some(t) = mtime {
        headers.insert(
            LAST_MODIFIED,
            httpdate::fmt_http_date(t)
                .parse()
                .expect("valid header value"),
        );
    }

    (StatusCode::NOT_MODIFIED, headers).into_response()
}

/// Build common response headers shared by info.json and image responses.
fn common_headers(etag: &str, mtime: Option<SystemTime>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("valid header value"),
    );
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=86400".parse().expect("valid header value"),
    );
    headers.insert(ETAG, etag.parse().expect("valid header value"));
    if let Some(t) = mtime {
        headers.insert(
            LAST_MODIFIED,
            httpdate::fmt_http_date(t)
                .parse()
                .expect("valid header value"),
        );
    }
    headers
}

// ---------------------------------------------------------------------------
// Canonical URI
// ---------------------------------------------------------------------------

struct CanonicalParams<'a> {
    base_url: &'a str,
    identifier: &'a str,
    region: &'a Region,
    size: &'a Size,
    rotation: &'a Rotation,
    quality: &'a Quality,
    format: &'a OutputFormat,
    img_w: u32,
    img_h: u32,
    config: &'a ImageConfig,
}

fn build_canonical_uri(p: &CanonicalParams<'_>) -> String {
    let (base_url, identifier, img_w, img_h) = (p.base_url, p.identifier, p.img_w, p.img_h);
    let (region, size, rotation, quality, format) =
        (p.region, p.size, p.rotation, p.quality, p.format);

    let (rx, ry, rw, rh) = region.resolve(img_w, img_h).unwrap_or((0, 0, img_w, img_h));
    let region_str = if rx == 0 && ry == 0 && rw == img_w && rh == img_h {
        "full".to_string()
    } else {
        format!("{rx},{ry},{rw},{rh}")
    };

    let (sw, sh) = size
        .resolve(
            rw,
            rh,
            p.config.max_width,
            p.config.max_height,
            p.config.max_area,
        )
        .unwrap_or((rw, rh));
    let size_str = if sw == rw && sh == rh && !size.upscale {
        "max".to_string()
    } else if size.upscale && (sw > rw || sh > rh) {
        format!("^{sw},{sh}")
    } else {
        format!("{sw},{sh}")
    };

    format!("{base_url}/{identifier}/{region_str}/{size_str}/{rotation}/{quality}.{format}")
}

