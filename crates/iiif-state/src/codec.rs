use iiif_core::error::IiifError;

/// Encode a content state JSON string to base64url (IIIF Content State API 1.0).
///
/// Steps: UTF-8 bytes → base64url encoding (RFC 4648) with padding stripped.
pub fn encode_content_state(json: &str) -> String {
    base64url_encode(json.as_bytes())
}

/// Decode a base64url-encoded content state back to JSON string.
pub fn decode_content_state(encoded: &str) -> Result<String, IiifError> {
    let bytes = base64url_decode(encoded)?;
    String::from_utf8(bytes)
        .map_err(|e| IiifError::BadRequest(format!("Content state is not valid UTF-8: {e}")))
}

/// Validate that the decoded JSON is a valid content state.
///
/// A content state must be one of:
/// 1. A full Annotation with `motivation: "contentState"`
/// 2. An Annotation URI (string)
/// 3. A target body (object with `id` and `type`)
pub fn validate_content_state(json: &str) -> Result<serde_json::Value, IiifError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| IiifError::BadRequest(format!("Invalid JSON in content state: {e}")))?;

    // Check for valid content state forms
    match &value {
        // Form 1: Full Annotation
        serde_json::Value::Object(obj)
            if obj.get("type").and_then(|t| t.as_str()) == Some("Annotation") =>
        {
            Ok(value)
        }
        // Form 3: Target body (has id and type)
        serde_json::Value::Object(obj) if obj.contains_key("id") && obj.contains_key("type") => {
            Ok(value)
        }
        // Form 2: URI string
        serde_json::Value::String(s) if s.starts_with("http") => Ok(value),
        _ => Err(IiifError::BadRequest(
            "Content state must be an Annotation, a resource with id/type, or a URI string"
                .to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Base64url (RFC 4648 §5) without padding
// ---------------------------------------------------------------------------

fn base64url_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

fn base64url_decode(input: &str) -> Result<Vec<u8>, IiifError> {
    fn char_to_val(c: u8) -> Result<u8, IiifError> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err(IiifError::BadRequest(format!(
                "Invalid base64url character: {}",
                c as char
            ))),
        }
    }

    // Strip padding if present
    let input = input.trim_end_matches('=');
    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let vals: Vec<u8> = chunk
            .iter()
            .map(|&b| char_to_val(b))
            .collect::<Result<_, _>>()?;

        let triple = match vals.len() {
            4 => {
                ((vals[0] as u32) << 18)
                    | ((vals[1] as u32) << 12)
                    | ((vals[2] as u32) << 6)
                    | (vals[3] as u32)
            }
            3 => ((vals[0] as u32) << 18) | ((vals[1] as u32) << 12) | ((vals[2] as u32) << 6),
            2 => ((vals[0] as u32) << 18) | ((vals[1] as u32) << 12),
            _ => {
                return Err(IiifError::BadRequest(
                    "Invalid base64url length".to_string(),
                ))
            }
        };

        result.push((triple >> 16) as u8);
        if vals.len() > 2 {
            result.push((triple >> 8 & 0xFF) as u8);
        }
        if vals.len() > 3 {
            result.push((triple & 0xFF) as u8);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let original = r#"{"id":"https://example.org/canvas/1","type":"Canvas"}"#;
        let encoded = encode_content_state(original);
        let decoded = decode_content_state(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn encode_known_value() {
        // "Hello" in base64url = "SGVsbG8"
        assert_eq!(base64url_encode(b"Hello"), "SGVsbG8");
    }

    #[test]
    fn decode_known_value() {
        let result = base64url_decode("SGVsbG8").unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn validate_annotation() {
        let json = r#"{"type":"Annotation","motivation":"contentState","target":{"id":"https://example.org/canvas/1","type":"Canvas"}}"#;
        assert!(validate_content_state(json).is_ok());
    }

    #[test]
    fn validate_target_body() {
        let json = r#"{"id":"https://example.org/canvas/1","type":"Canvas"}"#;
        assert!(validate_content_state(json).is_ok());
    }

    #[test]
    fn validate_uri() {
        let json = r#""https://example.org/annotation/1""#;
        assert!(validate_content_state(json).is_ok());
    }

    #[test]
    fn validate_invalid() {
        assert!(validate_content_state(r#"{"foo":"bar"}"#).is_err());
        assert!(validate_content_state("not json").is_err());
    }

    #[test]
    fn handles_unicode() {
        let original =
            r#"{"id":"https://example.org/opis","type":"Canvas","label":"Opis po polsku: źdźbło"}"#;
        let encoded = encode_content_state(original);
        let decoded = decode_content_state(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}
