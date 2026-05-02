//! End-to-end tests for the IIIF Authorization Flow API 2.0 router.
//!
//! Exercises the spec-mandated response shapes that v0.2.1 introduced:
//! probe must HTTP 200, token bodies carry `AuthAccessToken2`/`AuthAccessTokenError2`,
//! `targetOrigin` enforcement, and `Set-Cookie` security attributes.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use tower::ServiceExt;

use async_trait::async_trait;
use iiif_auth::AuthStore;
use iiif_core::config::{AppConfig, AuthConfig, UserCredential};
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

fn build_app() -> (Router, Arc<AuthStore>) {
    let config = AppConfig {
        auth: AuthConfig {
            enabled: true,
            pattern: "active".to_string(),
            cookie_name: "iiif_access".to_string(),
            token_ttl: 3600,
            protected_dirs: vec!["restricted".to_string()],
            users: vec![UserCredential {
                username: "alice".to_string(),
                password: "wonderland".to_string(),
            }],
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    };

    let store = Arc::new(AuthStore::new(config.auth.token_ttl));
    let state = AppState {
        config: Arc::new(config),
        storage: Arc::new(StubStorage),
    };
    let app = iiif_auth::router()
        .layer(Extension(Arc::clone(&store)))
        .with_state(state);
    (app, store)
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn probe_without_token_returns_http_200_and_body_status_401() {
    let (app, _) = build_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/probe/some-image")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Spec: probe MUST always be HTTP 200; auth state lives in the JSON body.
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["@context"], "http://iiif.io/api/auth/2/context.json");
    assert_eq!(body["type"], "AuthProbeResult2");
    assert_eq!(body["status"], 401);
}

#[tokio::test]
async fn probe_with_valid_bearer_returns_status_200_in_body() {
    let (app, store) = build_app();

    let session_id = store.create_session("alice");
    let (token, _ttl) = store.issue_token(&session_id).unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/probe/some-image")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["status"], 200);
    assert!(body.get("heading").is_none());
}

#[tokio::test]
async fn token_without_origin_emits_invalid_origin_error_to_wildcard_target() {
    let (app, _) = build_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/token?messageId=abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    // Error body MUST carry `type: "AuthAccessTokenError2"` and `profile: "invalidOrigin"`.
    assert!(html.contains(r#""type":"AuthAccessTokenError2""#));
    assert!(html.contains(r#""profile":"invalidOrigin""#));
    // Error path is the only context where targetOrigin "*" is allowed (no token leaks).
    assert!(html.contains(r#"postMessage("#));
    assert!(html.contains(r#", "*")"#));
    assert!(html.contains(r#""messageId":"abc""#));
}

#[tokio::test]
async fn token_with_origin_but_no_session_emits_missing_aspect_to_origin() {
    let (app, _) = build_app();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/token?messageId=xyz&origin=https://viewer.example.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains(r#""type":"AuthAccessTokenError2""#));
    assert!(html.contains(r#""profile":"missingAspect""#));
    assert!(html.contains(r#""messageId":"xyz""#));
    // Error to the client's origin, not "*".
    assert!(html.contains(r#", "https://viewer.example.org")"#));
}

#[tokio::test]
async fn token_with_session_emits_access_token_to_origin() {
    let (app, store) = build_app();

    let session_id = store.create_session("alice");

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/token?messageId=42&origin=https://viewer.example.org")
                .header("cookie", format!("iiif_access={session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains(r#""type":"AuthAccessToken2""#));
    assert!(html.contains(r#""accessToken":""#));
    assert!(html.contains(r#""expiresIn":3600"#));
    assert!(html.contains(r#""messageId":"42""#));
    // Success postMessage MUST go to the exact origin, never "*".
    assert!(html.contains(r#", "https://viewer.example.org")"#));
    assert!(!html.contains(r#", "*")"#));
}

#[tokio::test]
async fn login_sets_cookie_with_security_attributes() {
    let (app, _) = build_app();

    let form = "username=alice&password=wonderland&origin=https%3A%2F%2Fviewer.example.org";
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie present")
        .to_str()
        .unwrap()
        .to_string();
    // Base URL is http:// in the test config — expect dev-friendly attributes.
    assert!(cookie.starts_with("iiif_access="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
}

#[tokio::test]
async fn token_handler_does_not_break_out_of_script_tag_via_origin() {
    let (app, _) = build_app();

    // Attacker-controlled origin that contains `</script>` breakout.
    let evil_uri = "/auth/token?messageId=m&origin=https://evil.com%3C/script%3E%3Cscript%3Ealert(1)%3C/script%3E";

    let resp = app
        .oneshot(Request::builder().uri(evil_uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    // Body must contain exactly one closing script tag — the one we wrote.
    assert_eq!(
        html.matches("</script>").count(),
        1,
        "HTML body broke out of the script tag: {html}"
    );
    // Strict origin validator rejects the malformed origin → invalidOrigin error.
    assert!(html.contains(r#""profile":"invalidOrigin""#));
}

#[tokio::test]
async fn token_handler_does_not_break_out_of_script_tag_via_message_id() {
    let (app, store) = build_app();

    let session_id = store.create_session("alice");

    // messageId is echoed in the JSON body — must not allow </script> breakout.
    let evil_message_id = "m%3C/script%3E%3Cscript%3Ealert(1)%3C/script%3E";
    let uri = format!(
        "/auth/token?messageId={evil_message_id}&origin=https://viewer.example.org"
    );

    let resp = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("cookie", format!("iiif_access={session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert_eq!(
        html.matches("</script>").count(),
        1,
        "JSON body broke out of script tag via messageId: {html}"
    );
}

#[tokio::test]
async fn token_rejects_origin_not_in_allowlist() {
    // Build with a non-empty allowlist that does NOT include evil.com.
    let config = AppConfig {
        auth: AuthConfig {
            enabled: true,
            pattern: "active".to_string(),
            cookie_name: "iiif_access".to_string(),
            token_ttl: 3600,
            protected_dirs: vec!["restricted".to_string()],
            users: vec![UserCredential {
                username: "alice".to_string(),
                password: "wonderland".to_string(),
            }],
            allowed_origins: vec!["https://viewer.example.org".to_string()],
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    };
    let store = Arc::new(AuthStore::new(3600));
    let state = AppState {
        config: Arc::new(config),
        storage: Arc::new(StubStorage),
    };
    let app = iiif_auth::router()
        .layer(Extension(Arc::clone(&store)))
        .with_state(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/token?messageId=m&origin=https://evil.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains(r#""profile":"invalidOrigin""#));
}

#[tokio::test]
async fn probe_emits_substitute_when_configured() {
    let config = AppConfig {
        auth: AuthConfig {
            enabled: true,
            pattern: "active".to_string(),
            cookie_name: "iiif_access".to_string(),
            token_ttl: 3600,
            protected_dirs: vec!["restricted".to_string()],
            users: vec![UserCredential {
                username: "alice".to_string(),
                password: "wonderland".to_string(),
            }],
            substitute_size: "^200,".to_string(),
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    };
    let store = Arc::new(AuthStore::new(3600));
    let state = AppState {
        config: Arc::new(config),
        storage: Arc::new(StubStorage),
    };
    let app = iiif_auth::router()
        .layer(Extension(Arc::clone(&store)))
        .with_state(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/probe/secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["status"], 401);
    let subst = body["substitute"].as_array().unwrap();
    assert_eq!(subst.len(), 1);
    assert!(subst[0]["id"]
        .as_str()
        .unwrap()
        .ends_with("/secret/full/^200,/0/default.jpg"));
    assert_eq!(subst[0]["type"], "Image");
}

#[tokio::test]
async fn logout_invalidates_existing_tokens() {
    let (app, store) = build_app();
    let session_id = store.create_session("alice");
    let (token, _) = store.issue_token(&session_id).unwrap();
    assert!(store.validate_token(&token));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .header("cookie", format!("iiif_access={session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Active purge: token must no longer validate.
    assert!(!store.validate_token(&token));
}

#[tokio::test]
async fn logout_clears_cookie() {
    let (app, store) = build_app();

    let session_id = store.create_session("alice");

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .header("cookie", format!("iiif_access={session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("Max-Age=0"));
    // Session must be invalidated server-side.
    assert!(store.validate_session(&session_id).is_none());
}
