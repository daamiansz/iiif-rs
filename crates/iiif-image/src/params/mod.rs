pub mod format;
pub mod quality;
pub mod region;
pub mod rotation;
pub mod size;

pub use format::OutputFormat;
pub use quality::Quality;
pub use region::Region;
pub use rotation::Rotation;
pub use size::{Size, SizeMode};

use iiif_core::error::IiifError;
use iiif_core::identifier::ImageIdentifier;

/// A fully parsed IIIF Image API request.
#[derive(Debug, Clone)]
pub struct ImageRequest {
    pub identifier: ImageIdentifier,
    pub region: Region,
    pub size: Size,
    pub rotation: Rotation,
    pub quality: Quality,
    pub format: OutputFormat,
}

/// Parse the combined `{quality}.{format}` path segment.
pub fn parse_quality_format(s: &str) -> Result<(Quality, OutputFormat), IiifError> {
    let dot_pos = s.rfind('.').ok_or_else(|| {
        IiifError::BadRequest("Missing format extension in quality.format segment".to_string())
    })?;

    let quality_str = &s[..dot_pos];
    let format_str = &s[dot_pos + 1..];

    if quality_str.is_empty() {
        return Err(IiifError::BadRequest("Quality value is empty".to_string()));
    }
    if format_str.is_empty() {
        return Err(IiifError::BadRequest(
            "Format extension is empty".to_string(),
        ));
    }

    let quality = quality_str.parse()?;
    let format = format_str.parse()?;
    Ok((quality, format))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quality_format_valid() {
        let (q, f) = parse_quality_format("default.jpg").unwrap();
        assert_eq!(q, Quality::Default);
        assert_eq!(f, OutputFormat::Jpg);
    }

    #[test]
    fn parse_quality_format_gray_png() {
        let (q, f) = parse_quality_format("gray.png").unwrap();
        assert_eq!(q, Quality::Gray);
        assert_eq!(f, OutputFormat::Png);
    }

    #[test]
    fn parse_quality_format_missing_dot() {
        assert!(parse_quality_format("defaultjpg").is_err());
    }

    #[test]
    fn parse_quality_format_empty_parts() {
        assert!(parse_quality_format(".jpg").is_err());
        assert!(parse_quality_format("default.").is_err());
    }
}
