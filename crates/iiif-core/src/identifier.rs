use crate::error::IiifError;
use std::fmt;

/// A validated, percent-decoded IIIF resource identifier.
///
/// Ensures the identifier is valid UTF-8 after decoding and contains
/// no path traversal sequences.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageIdentifier(String);

impl ImageIdentifier {
    /// Create a new identifier from a raw, possibly percent-encoded string.
    pub fn from_encoded(raw: &str) -> Result<Self, IiifError> {
        if raw.is_empty() {
            return Err(IiifError::BadRequest(
                "Image identifier must not be empty".to_string(),
            ));
        }

        let decoded = percent_decode(raw)?;

        if decoded.contains("..") {
            return Err(IiifError::BadRequest(
                "Image identifier must not contain path traversal sequences".to_string(),
            ));
        }

        if decoded.starts_with('/') || decoded.starts_with('\\') {
            return Err(IiifError::BadRequest(
                "Image identifier must not start with a path separator".to_string(),
            ));
        }

        Ok(Self(decoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ImageIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn percent_decode(input: &str) -> Result<String, IiifError> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut iter = input.bytes();

    while let Some(b) = iter.next() {
        if b == b'%' {
            let hi = iter
                .next()
                .ok_or_else(|| IiifError::BadRequest("Incomplete percent-encoding".to_string()))?;
            let lo = iter
                .next()
                .ok_or_else(|| IiifError::BadRequest("Incomplete percent-encoding".to_string()))?;

            let hex = [hi, lo];
            let hex_str = std::str::from_utf8(&hex)
                .map_err(|_| IiifError::BadRequest("Invalid percent-encoding".to_string()))?;
            let byte = u8::from_str_radix(hex_str, 16)
                .map_err(|_| IiifError::BadRequest("Invalid percent-encoding".to_string()))?;

            bytes.push(byte);
        } else {
            bytes.push(b);
        }
    }

    // Decoded bytes form a UTF-8 string — multi-byte sequences like `%C3%A9`
    // (é) MUST be assembled as UTF-8, not pushed one Latin-1 char at a time.
    String::from_utf8(bytes)
        .map_err(|_| IiifError::BadRequest("Identifier is not valid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simple_identifier() {
        let id = ImageIdentifier::from_encoded("abcd1234").unwrap();
        assert_eq!(id.as_str(), "abcd1234");
    }

    #[test]
    fn valid_encoded_identifier() {
        let id = ImageIdentifier::from_encoded("ark:%2F12025%2F654xz321").unwrap();
        assert_eq!(id.as_str(), "ark:/12025/654xz321");
    }

    #[test]
    fn rejects_empty() {
        assert!(ImageIdentifier::from_encoded("").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(ImageIdentifier::from_encoded("..%2Fetc%2Fpasswd").is_err());
        assert!(ImageIdentifier::from_encoded("images/../secrets").is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        assert!(ImageIdentifier::from_encoded("%2Fetc%2Fpasswd").is_err());
    }

    #[test]
    fn decodes_utf8_multibyte_sequences() {
        // %C3%A9 → é (Latin small letter e with acute, U+00E9)
        let id = ImageIdentifier::from_encoded("caf%C3%A9").unwrap();
        assert_eq!(id.as_str(), "café");
        assert_eq!(id.as_str().chars().count(), 4);
    }

    #[test]
    fn decodes_three_byte_utf8() {
        // %E2%9C%93 → ✓ (check mark, U+2713)
        let id = ImageIdentifier::from_encoded("ok%E2%9C%93").unwrap();
        assert_eq!(id.as_str(), "ok✓");
    }

    #[test]
    fn decodes_polish_diacritics() {
        // %C5%82 → ł (U+0142)
        let id = ImageIdentifier::from_encoded("%C5%82amig%C5%82%C3%B3wka").unwrap();
        assert_eq!(id.as_str(), "łamigłówka");
    }

    #[test]
    fn rejects_invalid_utf8_byte_sequence() {
        // %C3 alone is an incomplete UTF-8 sequence
        assert!(ImageIdentifier::from_encoded("bad%C3").is_err());
    }

    #[test]
    fn double_encoded_percent_decodes_once() {
        // %25 → '%'. The IIIF spec note: a literal `%` in an identifier is
        // sent as `%25`, so `%2525` decodes once to `%25` (literal sequence).
        let id = ImageIdentifier::from_encoded("a%25b").unwrap();
        assert_eq!(id.as_str(), "a%b");

        let id2 = ImageIdentifier::from_encoded("a%2525b").unwrap();
        assert_eq!(id2.as_str(), "a%25b");
    }
}
