use axum::extract::{Query, State};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tracing::info;

use iiif_core::error::IiifError;
use iiif_core::state::AppState;

use crate::codec;
use crate::types::{ContentStateResponse, DecodeResponse, EncodeResponse};

/// Build the axum router for IIIF Content State API 1.0.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/content-state/encode", post(encode_handler))
        .route("/content-state/decode", get(decode_handler))
        .route(
            "/content-state",
            get(get_state_handler).post(post_state_handler),
        )
}

/// POST `/content-state/encode` — encode a JSON content state to base64url.
async fn encode_handler(Json(body): Json<serde_json::Value>) -> Result<Response, IiifError> {
    let json_str = serde_json::to_string(&body)
        .map_err(|e| IiifError::BadRequest(format!("Invalid JSON: {e}")))?;

    codec::validate_content_state(&json_str)?;

    let encoded = codec::encode_content_state(&json_str);
    info!(encoded_len = encoded.len(), "Encoded content state");

    let resp = EncodeResponse { encoded };
    Ok((StatusCode::OK, state_headers(), Json(resp)).into_response())
}

#[derive(Deserialize)]
struct DecodeQuery {
    content: String,
}

/// GET `/content-state/decode?content=...` — decode base64url to JSON.
async fn decode_handler(Query(params): Query<DecodeQuery>) -> Result<Response, IiifError> {
    let json_str = codec::decode_content_state(&params.content)?;
    let value = codec::validate_content_state(&json_str)?;

    info!("Decoded content state");

    let resp = DecodeResponse { content: value };
    Ok((StatusCode::OK, state_headers(), Json(resp)).into_response())
}

/// GET `/content-state?content=...` — accept encoded state, return decoded + re-encoded.
async fn get_state_handler(
    State(_state): State<AppState>,
    Query(params): Query<DecodeQuery>,
) -> Result<Response, IiifError> {
    let json_str = codec::decode_content_state(&params.content)?;
    let value = codec::validate_content_state(&json_str)?;
    let encoded = codec::encode_content_state(&json_str);

    let resp = ContentStateResponse {
        content: value,
        encoded,
    };
    Ok((StatusCode::OK, state_headers(), Json(resp)).into_response())
}

/// POST `/content-state` — accept JSON content state, return validated + encoded.
async fn post_state_handler(
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, IiifError> {
    let json_str = serde_json::to_string(&body)
        .map_err(|e| IiifError::BadRequest(format!("Invalid JSON: {e}")))?;

    let value = codec::validate_content_state(&json_str)?;
    let encoded = codec::encode_content_state(&json_str);

    let resp = ContentStateResponse {
        content: value,
        encoded,
    };
    Ok((StatusCode::OK, state_headers(), Json(resp)).into_response())
}

fn state_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().expect("valid"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().expect("valid"));
    headers
}

