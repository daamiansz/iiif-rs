use std::fmt;
use std::str::FromStr;

use iiif_core::error::IiifError;

/// IIIF Image API rotation parameter.
///
/// Supports mirroring (horizontal flip) followed by clockwise rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Rotation {
    /// Whether to mirror the image along the vertical axis before rotating.
    pub mirror: bool,
    /// Clockwise rotation in degrees (0.0–360.0).
    pub degrees: f64,
}

impl Rotation {
    /// Returns `true` if this rotation is a no-op (no mirror, 0 degrees).
    pub fn is_noop(&self) -> bool {
        !self.mirror && (self.degrees == 0.0 || self.degrees == 360.0)
    }

    /// Returns `true` if the rotation is a multiple of 90 degrees.
    pub fn is_orthogonal(&self) -> bool {
        self.degrees % 90.0 == 0.0
    }
}

impl FromStr for Rotation {
    type Err = IiifError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (input, mirror) = if let Some(rest) = s.strip_prefix('!') {
            (rest, true)
        } else {
            (s, false)
        };

        if input.is_empty() {
            return Err(IiifError::BadRequest(
                "Rotation value must not be empty".to_string(),
            ));
        }

        let degrees: f64 = input
            .parse()
            .map_err(|_| IiifError::BadRequest(format!("Invalid rotation value: {input}")))?;

        if !(0.0..=360.0).contains(&degrees) {
            return Err(IiifError::BadRequest(format!(
                "Rotation must be between 0 and 360, got {degrees}"
            )));
        }

        Ok(Self { mirror, degrees })
    }
}

impl fmt::Display for Rotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mirror {
            write!(f, "!")?;
        }
        // Use integer representation when possible
        if self.degrees.fract() == 0.0 {
            write!(f, "{}", self.degrees as u32)
        } else {
            write!(f, "{}", self.degrees)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zero() {
        let r: Rotation = "0".parse().unwrap();
        assert!(!r.mirror);
        assert_eq!(r.degrees, 0.0);
        assert!(r.is_noop());
    }

    #[test]
    fn parse_90() {
        let r: Rotation = "90".parse().unwrap();
        assert_eq!(r.degrees, 90.0);
        assert!(r.is_orthogonal());
    }

    #[test]
    fn parse_mirror_180() {
        let r: Rotation = "!180".parse().unwrap();
        assert!(r.mirror);
        assert_eq!(r.degrees, 180.0);
    }

    #[test]
    fn parse_arbitrary() {
        let r: Rotation = "22.5".parse().unwrap();
        assert_eq!(r.degrees, 22.5);
        assert!(!r.is_orthogonal());
    }

    #[test]
    fn reject_out_of_range() {
        assert!("361".parse::<Rotation>().is_err());
        assert!("-1".parse::<Rotation>().is_err());
    }

    #[test]
    fn reject_empty() {
        assert!("".parse::<Rotation>().is_err());
        assert!("!".parse::<Rotation>().is_err());
    }

    #[test]
    fn display_integer() {
        let r = Rotation {
            mirror: false,
            degrees: 90.0,
        };
        assert_eq!(r.to_string(), "90");
    }

    #[test]
    fn display_mirror_float() {
        let r = Rotation {
            mirror: true,
            degrees: 22.5,
        };
        assert_eq!(r.to_string(), "!22.5");
    }
}
