use axum::extract::{Path, Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tracing::info;

use iiif_core::state::AppState;

use crate::store::AuthStore;
use crate::types::ProbeResult;

/// Build the auth router. Requires `AuthStore` in axum Extension.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page).post(login_submit))
        .route("/auth/token", get(token_handler))
        .route("/auth/probe/{resource_id}", get(probe_handler))
        .route("/auth/logout", get(logout_handler))
}

// ---------------------------------------------------------------------------
// Login (Access Service)
// ---------------------------------------------------------------------------

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

/// POST /auth/login — validate credentials, set cookie, close window.
async fn login_submit(
    State(state): State<AppState>,
    axum::extract::Form(form): axum::extract::Form<LoginForm>,
) -> Response {
    let auth_config = &state.config.auth;

    // Validate credentials
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

    // Create session
    let auth_store = state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>())
        .expect("auth store");
    let session_id = auth_store.create_session(&form.username);
    let cookie_name = &auth_config.cookie_name;

    info!(user = %form.username, "User logged in");

    // Set cookie and return page that closes itself
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!("{cookie_name}={session_id}; Path=/; HttpOnly; SameSite=Lax")
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
/// Per IIIF Auth Flow 2.0, the response is a JSON object delivered via
/// `postMessage` when loaded in an iframe.
async fn token_handler(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    req_headers: HeaderMap,
) -> Response {
    let auth_store = match state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>())
    {
        Some(s) => s,
        None => return json_error("authUnavailable", "Auth is not enabled"),
    };
    let cookie_name = &state.config.auth.cookie_name;
    let message_id = params.message_id.unwrap_or_default();

    // Extract session ID from cookie
    let session_id = extract_cookie(&req_headers, cookie_name);

    let body = match session_id.and_then(|sid| auth_store.issue_token(&sid)) {
        Some((token, expires_in)) => {
            info!("Issued access token");
            serde_json::to_string(&serde_json::json!({
                "accessToken": token,
                "expiresIn": expires_in,
                "messageId": message_id,
            }))
            .expect("valid json")
        }
        None => serde_json::to_string(&serde_json::json!({
            "type": "missingCredentials",
            "description": "No valid session cookie found. Please log in first.",
            "messageId": message_id,
        }))
        .expect("valid json"),
    };

    let origin = params.origin.unwrap_or_else(|| "*".to_string());

    // Return as HTML with postMessage script for iframe communication
    let html = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head><body>
<script>
  window.parent.postMessage({body}, "{origin}");
</script>
</body></html>"#
    );

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/html".parse().expect("valid"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));

    (StatusCode::OK, headers, html).into_response()
}

// ---------------------------------------------------------------------------
// Probe Service
// ---------------------------------------------------------------------------

/// GET /auth/probe/{resource_id} — check if the bearer token grants access.
async fn probe_handler(
    State(state): State<AppState>,
    Path(resource_id): Path<String>,
    req_headers: HeaderMap,
) -> Response {
    let auth_store = match state
        .auth
        .as_ref()
        .and_then(|a| a.downcast_ref::<AuthStore>())
    {
        Some(s) => s,
        None => return json_error("authUnavailable", "Auth is not enabled"),
    };
    let base = &state.config.server.base_url;
    let probe_id = format!("{base}/auth/probe/{resource_id}");

    // Extract Bearer token from Authorization header
    let token = req_headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let (status, heading, note) = match token {
        Some(t) if auth_store.validate_token(t) => (200u16, None, None),
        Some(_) => (
            401,
            Some(serde_json::json!({"en": ["Authentication expired"]})),
            Some(
                serde_json::json!({"en": ["Your token is invalid or expired. Please log in again."]}),
            ),
        ),
        None => (
            401,
            Some(serde_json::json!({"en": ["Authentication required"]})),
            Some(serde_json::json!({"en": ["Please log in to access this resource."]})),
        ),
    };

    let result = ProbeResult {
        id: probe_id,
        result_type: "AuthProbeResult2".to_string(),
        status,
        heading,
        note,
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().expect("valid"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));

    let code = if status == 200 {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    };

    (
        code,
        headers,
        serde_json::to_string(&result).expect("valid json"),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Logout
// ---------------------------------------------------------------------------

/// GET /auth/logout — clear the session cookie.
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
        format!("{cookie_name}=; Path=/; HttpOnly; Max-Age=0")
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

fn json_error(error_type: &str, description: &str) -> Response {
    let body = serde_json::json!({
        "type": error_type,
        "description": description,
    });

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().expect("valid"));

    (StatusCode::INTERNAL_SERVER_ERROR, headers, body.to_string()).into_response()
}
