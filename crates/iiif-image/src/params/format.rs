use std::fmt;
use std::str::FromStr;

use iiif_core::error::IiifError;

/// IIIF Image API output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Jpg,
    Png,
    Gif,
    Webp,
    Tif,
}

impl OutputFormat {
    /// MIME content type for HTTP responses.
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Jpg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Tif => "image/tiff",
        }
    }

    /// Map to `image` crate's `ImageFormat`.
    pub fn to_image_format(&self) -> image::ImageFormat {
        match self {
            Self::Jpg => image::ImageFormat::Jpeg,
            Self::Png => image::ImageFormat::Png,
            Self::Gif => image::ImageFormat::Gif,
            Self::Webp => image::ImageFormat::WebP,
            Self::Tif => image::ImageFormat::Tiff,
        }
    }

    /// File extension string.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Tif => "tif",
        }
    }
}

impl FromStr for OutputFormat {
    type Err = IiifError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jpg" | "jpeg" => Ok(Self::Jpg),
            "png" => Ok(Self::Png),
            "gif" => Ok(Self::Gif),
            "webp" => Ok(Self::Webp),
            "tif" | "tiff" => Ok(Self::Tif),
            _ => Err(IiifError::BadRequest(format!("Unsupported format: {s}"))),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_formats() {
        assert_eq!("jpg".parse::<OutputFormat>().unwrap(), OutputFormat::Jpg);
        assert_eq!("jpeg".parse::<OutputFormat>().unwrap(), OutputFormat::Jpg);
        assert_eq!("png".parse::<OutputFormat>().unwrap(), OutputFormat::Png);
        assert_eq!("gif".parse::<OutputFormat>().unwrap(), OutputFormat::Gif);
        assert_eq!("webp".parse::<OutputFormat>().unwrap(), OutputFormat::Webp);
        assert_eq!("tif".parse::<OutputFormat>().unwrap(), OutputFormat::Tif);
        assert_eq!("tiff".parse::<OutputFormat>().unwrap(), OutputFormat::Tif);
    }

    #[test]
    fn reject_unknown() {
        assert!("bmp".parse::<OutputFormat>().is_err());
        assert!("pdf".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn content_types() {
        assert_eq!(OutputFormat::Jpg.content_type(), "image/jpeg");
        assert_eq!(OutputFormat::Png.content_type(), "image/png");
        assert_eq!(OutputFormat::Webp.content_type(), "image/webp");
    }
}
