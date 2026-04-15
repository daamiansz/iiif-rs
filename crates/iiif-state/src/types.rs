use serde::{Deserialize, Serialize};

/// Content state request (for POST endpoint).
#[derive(Debug, Deserialize)]
pub struct ContentStateRequest {
    /// Raw JSON content state (one of the three forms).
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    /// Base64url-encoded content state.
    #[serde(default)]
    pub encoded: Option<String>,
}

/// Content state response.
#[derive(Debug, Serialize)]
pub struct ContentStateResponse {
    /// The decoded/validated content state.
    pub content: serde_json::Value,
    /// Base64url-encoded form (for sharing via URL).
    pub encoded: String,
}

/// Content state encoding response (encode endpoint).
#[derive(Debug, Serialize)]
pub struct EncodeResponse {
    pub encoded: String,
}

/// Content state decoding response (decode endpoint).
#[derive(Debug, Serialize)]
pub struct DecodeResponse {
    pub content: serde_json::Value,
}
