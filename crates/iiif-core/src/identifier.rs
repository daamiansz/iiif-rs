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
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();

    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars
                .next()
                .ok_or_else(|| IiifError::BadRequest("Incomplete percent-encoding".to_string()))?;
            let lo = chars
                .next()
                .ok_or_else(|| IiifError::BadRequest("Incomplete percent-encoding".to_string()))?;

            let hex = [hi, lo];
            let hex_str = std::str::from_utf8(&hex)
                .map_err(|_| IiifError::BadRequest("Invalid percent-encoding".to_string()))?;
            let byte = u8::from_str_radix(hex_str, 16)
                .map_err(|_| IiifError::BadRequest("Invalid percent-encoding".to_string()))?;

            result.push(byte as char);
        } else {
            result.push(b as char);
        }
    }

    Ok(result)
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
}
