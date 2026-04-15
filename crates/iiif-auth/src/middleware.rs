use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use iiif_core::config::AuthConfig;

use crate::types::is_protected;

/// Middleware that checks the access cookie for protected image resources.
///
/// If the requested image identifier matches a protected pattern and no valid
/// session cookie is present, the request is rejected with 401.
///
/// This middleware should be applied to Image API routes.
pub async fn check_access(request: Request, next: Next) -> Response {
    // Extract identifier from path: /{identifier}/... or /{identifier}
    let path = request.uri().path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Skip non-image routes (auth, manifest, collection)
    if segments
        .first()
        .is_some_and(|s| *s == "auth" || *s == "manifest" || *s == "collection")
    {
        return next.run(request).await;
    }

    let identifier = match segments.first() {
        Some(id) => *id,
        None => return next.run(request).await,
    };

    // Get auth config from extensions (set by the server)
    let auth_config = request.extensions().get::<AuthConfig>();
    let cookie_name = request.extensions().get::<CookieName>();

    let (auth_config, cookie_name) = match (auth_config, cookie_name) {
        (Some(ac), Some(cn)) => (ac, &cn.0),
        _ => return next.run(request).await,
    };

    if !auth_config.enabled || !is_protected(identifier, &auth_config.protected) {
        return next.run(request).await;
    }

    // Check for valid session cookie
    let has_valid_cookie = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|cookies| {
            cookies.split(';').any(|pair| {
                let pair = pair.trim();
                if let Some((name, value)) = pair.split_once('=') {
                    name.trim() == cookie_name && !value.trim().is_empty()
                } else {
                    false
                }
            })
        })
        .unwrap_or(false);

    if has_valid_cookie {
        // Cookie present — validate session via the auth store
        let auth_store = request
            .extensions()
            .get::<std::sync::Arc<crate::AuthStore>>();
        let session_valid = auth_store.is_some_and(|store| {
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
            cookie_val
                .map(|sid| store.validate_session(&sid).is_some())
                .unwrap_or(false)
        });

        if session_valid {
            return next.run(request).await;
        }
    }

    // No valid session — return 401
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

/// Newtype wrapper so we can insert the cookie name into axum extensions.
#[derive(Clone)]
pub struct CookieName(pub String);
