use axum::extract::{Path, Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use iiif_core::state::AppState;

use crate::store::AuthStore;
use crate::types::AUTH_CONTEXT;

/// Build the auth router. Requires `AuthStore` in axum Extension.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page).post(login_submit))
        .route("/auth/token", get(token_handler))
        .route("/auth/probe/{resource_id}", get(probe_handler))
        .route("/auth/logout", get(logout_handler))
}

// ---------------------------------------------------------------------------
// Cookie attributes
// ---------------------------------------------------------------------------

/// Build security attributes for `Set-Cookie`. `Secure; SameSite=None` over
/// HTTPS to allow the cross-site iframe token flow; plain `SameSite=Lax` on
/// HTTP for local development convenience.
fn cookie_attrs(base_url: &str) -> &'static str {
    if base_url.starts_with("https://") {
        "Path=/; HttpOnly; Secure; SameSite=None"
    } else {
        "Path=/; HttpOnly; SameSite=Lax"
    }
}

// ---------------------------------------------------------------------------
// Login (Access Service)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LoginQuery {
    origin: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct LoginForm {
    username: String,
    password: String,
    origin: Option<String>,
}

/// GET /auth/login — render the login form.
async fn login_page(
    State(state): State<AppState>,
    Query(params): Query<LoginQuery>,
) -> Html<String> {
    let base = &state.config.server.base_url;
    let origin = params.origin.unwrap_or_default();

    Html(format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>IIIF Login</title>
<style>
body {{ font-family: sans-serif; max-width: 400px; margin: 60px auto; }}
input {{ width: 100%; padding: 8px; margin: 6px 0 16px; box-sizing: border-box; }}
button {{ padding: 10px 20px; background: #2563eb; color: white; border: none; cursor: pointer; width: 100%; }}
button:hover {{ background: #1d4ed8; }}
.error {{ color: #dc2626; }}
</style></head>
<body>
<h2>IIIF Authentication</h2>
<p>This resource requires authentication to access.</p>
<form method="POST" action="{base}/auth/login">
  <input type="hidden" name="origin" value="{origin}">
  <label>Username<input type="text" name="username" required></label>
  <label>Password<input type="password" name="password" required></label>
  <button type="submit">Login</button>
</form>
</body></html>"#
    ))
}

/// POST /auth/login — validate credentials, set cookie, close window.
async fn login_submit(
    State(state): State<AppState>,
    axum::extract::Form(form): axum::extract::Form<LoginForm>,
) -> Response {
    let auth_config = &state.config.auth;

    let valid = auth_config
        .users
        .iter()
        .any(|u| u.username == form.username && u.password == form.password);

    if !valid {
        let base = &state.config.server.base_url;
        return Html(format!(
            r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Login Failed</title>
<style>body {{ font-family: sans-serif; max-width: 400px; margin: 60px auto; }} .error {{ color: #dc2626; }}</style></head>
<body>
<h2>Login Failed</h2>
<p class="error">Invalid username or password.</p>
<p><a href="{base}/auth/login">Try again</a></p>
</body></html>"#
        ))
        .into_response();
    }

    let auth_store = state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>())
        .expect("auth store");
    let session_id = auth_store.create_session(&form.username);
    let cookie_name = &auth_config.cookie_name;

    info!(user = %form.username, "User logged in");

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!(
            "{cookie_name}={session_id}; {attrs}",
            attrs = cookie_attrs(&state.config.server.base_url)
        )
        .parse()
        .expect("valid cookie"),
    );

    let html = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Logged In</title></head>
<body>
<p>Login successful. This window will close automatically.</p>
<script>window.close();</script>
</body></html>"#;

    (StatusCode::OK, headers, Html(html.to_string())).into_response()
}

// ---------------------------------------------------------------------------
// Token Service
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TokenQuery {
    #[serde(rename = "messageId")]
    message_id: Option<String>,
    origin: Option<String>,
}

/// GET /auth/token — issue an access token based on the session cookie.
///
/// Returns HTTP 200 with an HTML page that calls `window.parent.postMessage(...)`
/// with either an `AuthAccessToken2` (success) or `AuthAccessTokenError2` (failure).
async fn token_handler(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    req_headers: HeaderMap,
) -> Response {
    let message_id = params.message_id.clone().unwrap_or_default();

    // Strict origin validation defends against XSS via `</script>` breakout in
    // the inline-script template below. We accept only `scheme://host[:port]`
    // with a conservative host charset; anything else is `invalidOrigin`.
    // Error path is the only context where targetOrigin "*" is allowed (no token leaks).
    let origin = match params.origin.as_deref() {
        Some(o) if is_valid_origin(o) => o.to_string(),
        _ => {
            return token_post_message(
                "*",
                token_error_body(
                    "invalidOrigin",
                    &message_id,
                    "Missing, empty, or malformed origin parameter.",
                ),
            );
        }
    };

    let Some(auth_store) = state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>())
    else {
        return token_post_message(
            &origin,
            token_error_body("unavailable", &message_id, "Auth is not enabled on this server."),
        );
    };

    let cookie_name = &state.config.auth.cookie_name;
    let session_id = extract_cookie(&req_headers, cookie_name);

    let body = match session_id.and_then(|sid| auth_store.issue_token(&sid)) {
        Some((token, expires_in)) => {
            info!("Issued access token");
            json!({
                "@context": AUTH_CONTEXT,
                "type": "AuthAccessToken2",
                "accessToken": token,
                "expiresIn": expires_in,
                "messageId": message_id,
            })
        }
        None => token_error_body(
            "missingAspect",
            &message_id,
            "No valid session cookie found. Please log in first.",
        ),
    };

    token_post_message(&origin, body)
}

fn token_error_body(profile: &str, message_id: &str, note_text: &str) -> serde_json::Value {
    json!({
        "@context": AUTH_CONTEXT,
        "type": "AuthAccessTokenError2",
        "profile": profile,
        "messageId": message_id,
        "heading": {"en": ["Authentication failed"]},
        "note": {"en": [note_text]},
    })
}

fn token_post_message(target_origin: &str, body: serde_json::Value) -> Response {
    // `target_origin` is either "*" (error path only) or already validated by
    // `is_valid_origin` (success path). The body-JSON, however, includes user-
    // controlled `messageId`; serde_json does not escape `/`, so a literal
    // `</script>` inside any string would break out of the inline script tag.
    //
    // Defense: escape `</` to `<\/` in the JSON. This is semantically identical
    // (`\/` is a valid JSON escape for `/`) but neutralises HTML parser breakout.
    let body_json = serde_json::to_string(&body)
        .expect("valid json")
        .replace("</", "<\\/");

    // `target_origin` has been validated, but apply the same `</` neutralisation
    // for defence-in-depth in case validation ever loosens.
    let safe_origin = target_origin.replace("</", "<\\/");

    let html = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head><body>
<script>
  window.parent.postMessage({body_json}, "{safe_origin}");
</script>
</body></html>"#
    );

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/html; charset=utf-8".parse().expect("valid"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));

    (StatusCode::OK, headers, html).into_response()
}

/// Conservative origin validator: `scheme://host[:port]` only, ASCII host charset.
/// Used to gate the inline-script `targetOrigin` substitution.
fn is_valid_origin(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    let Some(rest) = rest else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }

    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
        _ => (rest, None),
    };

    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return false;
    }

    if let Some(p) = port {
        if p.len() > 5 {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Probe Service
// ---------------------------------------------------------------------------

/// GET /auth/probe/{resource_id} — check if the bearer token grants access.
///
/// HTTP response is ALWAYS 200 per IIIF Auth Flow 2.0; the would-be access status
/// is carried in the body's `status` field.
async fn probe_handler(
    State(state): State<AppState>,
    Path(_resource_id): Path<String>,
    req_headers: HeaderMap,
) -> Response {
    let auth_store = state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>());

    let token = req_headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let (status, heading, note) = match (auth_store, token) {
        (Some(store), Some(t)) if store.validate_token(t) => (200u16, None, None),
        (Some(_), Some(_)) => (
            401,
            Some(json!({"en": ["Authentication expired"]})),
            Some(json!({"en": ["Your token is invalid or expired. Please log in again."]})),
        ),
        (Some(_), None) => (
            401,
            Some(json!({"en": ["Authentication required"]})),
            Some(json!({"en": ["Please log in to access this resource."]})),
        ),
        (None, _) => (
            503,
            Some(json!({"en": ["Authentication unavailable"]})),
            Some(json!({"en": ["The authentication service is not configured on this server."]})),
        ),
    };

    let mut body = json!({
        "@context": AUTH_CONTEXT,
        "type": "AuthProbeResult2",
        "status": status,
    });
    if let Some(h) = heading {
        body["heading"] = h;
    }
    if let Some(n) = note {
        body["note"] = n;
    }

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().expect("valid"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));

    // ALWAYS HTTP 200 — auth state lives in the body's `status` field.
    (StatusCode::OK, headers, body.to_string()).into_response()
}

// ---------------------------------------------------------------------------
// Logout
// ---------------------------------------------------------------------------

/// GET /auth/logout — clear session, cookie, and any tokens issued for it.
async fn logout_handler(State(state): State<AppState>, req_headers: HeaderMap) -> Response {
    let cookie_name = &state.config.auth.cookie_name;

    if let Some(sid) = extract_cookie(&req_headers, cookie_name) {
        if let Some(auth_store) = state
            .auth
            .as_ref()
            .and_then(|a| a.downcast_ref::<AuthStore>())
        {
            auth_store.remove_session(&sid);
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!(
            "{cookie_name}=; {attrs}; Max-Age=0",
            attrs = cookie_attrs(&state.config.server.base_url)
        )
        .parse()
        .expect("valid cookie"),
    );

    info!("User logged out");
    (StatusCode::OK, headers, "Logged out").into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers
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
        })
}
