use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use percent_encoding::percent_decode_str;

use iiif_core::config::AuthConfig;
use iiif_core::storage::ImageStorage;

/// Middleware that checks the access cookie for protected image resources.
///
/// Protection is directory-based: images in subdirectories listed in
/// `auth.protected_dirs` (e.g., `restricted/`) require a valid session cookie.
/// Images in the root or other directories are served without authentication.
pub async fn check_access(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Skip non-image routes
    if segments.first().is_some_and(|s| {
        *s == "auth"
            || *s == "manifest"
            || *s == "collection"
            || *s == "search"
            || *s == "autocomplete"
            || *s == "content-state"
            || *s == "activity"
    }) {
        return next.run(request).await;
    }

    let identifier = match segments.first() {
        Some(id) => *id,
        None => return next.run(request).await,
    };

    // Get auth config and storage from extensions
    let auth_config = request.extensions().get::<AuthConfig>();
    let cookie_name = request.extensions().get::<CookieName>();
    let storage = request.extensions().get::<Arc<dyn ImageStorage>>();

    let (auth_config, cookie_name, storage) = match (auth_config, cookie_name, storage) {
        (Some(ac), Some(cn), Some(st)) => (ac, &cn.0, st),
        _ => return next.run(request).await,
    };

    if !auth_config.enabled {
        return next.run(request).await;
    }

    // Check which directory the image is in
    let subdir = storage.access_zone(identifier);
    let is_protected = match &subdir {
        Some(dir) => auth_config.protected_dirs.iter().any(|d| d == dir),
        None => false,
    };

    if !is_protected {
        return next.run(request).await;
    }

    // Tiered access: requests for the configured substitute size pass through
    // without auth. The `substitute[]` URI advertised in the probe response
    // points at this very route, so it MUST be reachable per spec §6.
    //
    // Match the full IIIF image-request shape (5 segments:
    // {id}/{region}/{size}/{rotation}/{quality.format}). The size segment is
    // percent-decoded before comparison so clients that escape `^`/`,` still
    // hit the substitute path.
    if !auth_config.substitute_size.is_empty()
        && segments.len() == 5
        && segments.get(1) == Some(&"full")
    {
        let raw = segments[2];
        let decoded = percent_decode_str(raw).decode_utf8_lossy();
        if decoded == auth_config.substitute_size.as_str() {
            return next.run(request).await;
        }
    }

    // Protected resource — check for valid session cookie
    let session_valid = check_session_cookie(&request, cookie_name);

    if session_valid {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        serde_json::json!({
            "error": "Authentication required",
            "status": 401
        })
        .to_string(),
    )
        .into_response()
}

/// Validate the session cookie against the auth store.
fn check_session_cookie(request: &Request, cookie_name: &str) -> bool {
    let auth_store = request.extensions().get::<Arc<crate::AuthStore>>();

    let cookie_val = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|pair| {
                let pair = pair.trim();
                let (name, value) = pair.split_once('=')?;
                if name.trim() == cookie_name {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            })
        });

    match (auth_store, cookie_val) {
        (Some(store), Some(sid)) => store.validate_session(&sid).is_some(),
        _ => false,
    }
}

/// Newtype wrapper so we can insert the cookie name into axum extensions.
#[derive(Clone)]
pub struct CookieName(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(substitute_size: &str) -> AuthConfig {
        AuthConfig {
            enabled: true,
            pattern: "active".to_string(),
            cookie_name: "iiif_access".to_string(),
            token_ttl: 3600,
            protected_dirs: vec!["restricted".to_string()],
            users: vec![],
            allowed_origins: vec![],
            token_sweep_interval_secs: 0,
            substitute_size: substitute_size.to_string(),
        }
    }

    #[test]
    fn substitute_size_matches_after_decode() {
        let auth_config = cfg("^200,");
        let cases = [
            ("^200,", true),
            ("%5E200,", true),
            ("%5E200%2C", true),
            ("200,", false),
            ("^300,", false),
        ];
        for (segment, expect_match) in cases {
            let decoded = percent_decode_str(segment).decode_utf8_lossy();
            let matches = decoded == auth_config.substitute_size.as_str();
            assert_eq!(
                matches, expect_match,
                "segment {segment:?} should match={expect_match}"
            );
        }
    }

    #[test]
    fn empty_substitute_size_disables_exemption() {
        let auth_config = cfg("");
        // No string equality possible; the gating code short-circuits on empty.
        assert!(auth_config.substitute_size.is_empty());
    }
}
