use std::fmt;
use std::str::FromStr;

use iiif_core::error::IiifError;

/// IIIF Image API quality parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// Server's default rendering.
    Default,
    /// Full color.
    Color,
    /// Grayscale.
    Gray,
    /// Black and white only.
    Bitonal,
}

impl FromStr for Quality {
    type Err = IiifError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "color" => Ok(Self::Color),
            "gray" => Ok(Self::Gray),
            "bitonal" => Ok(Self::Bitonal),
            _ => Err(IiifError::BadRequest(format!("Unsupported quality: {s}"))),
        }
    }
}

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Color => write!(f, "color"),
            Self::Gray => write!(f, "gray"),
            Self::Bitonal => write!(f, "bitonal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_qualities() {
        assert_eq!("default".parse::<Quality>().unwrap(), Quality::Default);
        assert_eq!("color".parse::<Quality>().unwrap(), Quality::Color);
        assert_eq!("gray".parse::<Quality>().unwrap(), Quality::Gray);
        assert_eq!("bitonal".parse::<Quality>().unwrap(), Quality::Bitonal);
    }

    #[test]
    fn reject_unknown() {
        assert!("sepia".parse::<Quality>().is_err());
        assert!("Color".parse::<Quality>().is_err());
    }

    #[test]
    fn display_roundtrip() {
        for q in [
            Quality::Default,
            Quality::Color,
            Quality::Gray,
            Quality::Bitonal,
        ] {
            assert_eq!(q.to_string().parse::<Quality>().unwrap(), q);
        }
    }
}
