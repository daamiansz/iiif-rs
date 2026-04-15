use std::fmt;
use std::str::FromStr;

use iiif_core::error::IiifError;

/// IIIF Image API region parameter.
///
/// Defines which area of the full image to extract.
/// Processing order: region is applied first.
#[derive(Debug, Clone, PartialEq)]
pub enum Region {
    /// The full image, no cropping.
    Full,
    /// A square region centered on the shorter dimension.
    Square,
    /// Absolute pixel coordinates.
    Absolute { x: u32, y: u32, w: u32, h: u32 },
    /// Percentage-based coordinates (0.0–100.0).
    Percent { x: f64, y: f64, w: f64, h: f64 },
}

impl Region {
    /// Resolve this region to absolute pixel coordinates given the source image dimensions.
    /// Returns `(x, y, width, height)` clamped to image bounds.
    pub fn resolve(
        &self,
        img_width: u32,
        img_height: u32,
    ) -> Result<(u32, u32, u32, u32), IiifError> {
        match self {
            Self::Full => Ok((0, 0, img_width, img_height)),

            Self::Square => {
                let side = img_width.min(img_height);
                let x = (img_width - side) / 2;
                let y = (img_height - side) / 2;
                Ok((x, y, side, side))
            }

            Self::Absolute { x, y, w, h } => {
                if *w == 0 || *h == 0 {
                    return Err(IiifError::BadRequest(
                        "Region width and height must be greater than zero".to_string(),
                    ));
                }
                if *x >= img_width || *y >= img_height {
                    return Err(IiifError::BadRequest(
                        "Region is entirely outside image bounds".to_string(),
                    ));
                }
                // Clamp to image bounds
                let clamped_w = (*w).min(img_width.saturating_sub(*x));
                let clamped_h = (*h).min(img_height.saturating_sub(*y));
                Ok((*x, *y, clamped_w, clamped_h))
            }

            Self::Percent { x, y, w, h } => {
                if *w <= 0.0 || *h <= 0.0 {
                    return Err(IiifError::BadRequest(
                        "Region percentage width and height must be greater than zero".to_string(),
                    ));
                }
                let abs_x = (*x / 100.0 * img_width as f64).round() as u32;
                let abs_y = (*y / 100.0 * img_height as f64).round() as u32;
                let abs_w = (*w / 100.0 * img_width as f64).round() as u32;
                let abs_h = (*h / 100.0 * img_height as f64).round() as u32;

                if abs_x >= img_width || abs_y >= img_height {
                    return Err(IiifError::BadRequest(
                        "Region is entirely outside image bounds".to_string(),
                    ));
                }

                let clamped_w = abs_w.min(img_width.saturating_sub(abs_x)).max(1);
                let clamped_h = abs_h.min(img_height.saturating_sub(abs_y)).max(1);
                Ok((abs_x, abs_y, clamped_w, clamped_h))
            }
        }
    }
}

impl FromStr for Region {
    type Err = IiifError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full" => Ok(Self::Full),
            "square" => Ok(Self::Square),
            s if s.starts_with("pct:") => {
                let coords = &s[4..];
                let parts: Vec<&str> = coords.split(',').collect();
                if parts.len() != 4 {
                    return Err(IiifError::BadRequest(format!(
                        "Percent region requires exactly 4 values, got {}",
                        parts.len()
                    )));
                }
                let x = parse_float(parts[0], "region x")?;
                let y = parse_float(parts[1], "region y")?;
                let w = parse_float(parts[2], "region w")?;
                let h = parse_float(parts[3], "region h")?;

                if x < 0.0 || y < 0.0 || w < 0.0 || h < 0.0 {
                    return Err(IiifError::BadRequest(
                        "Region percentage values must be non-negative".to_string(),
                    ));
                }

                Ok(Self::Percent { x, y, w, h })
            }
            s => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() != 4 {
                    return Err(IiifError::BadRequest(format!(
                        "Absolute region requires exactly 4 values, got {}",
                        parts.len()
                    )));
                }
                let x = parse_u32(parts[0], "region x")?;
                let y = parse_u32(parts[1], "region y")?;
                let w = parse_u32(parts[2], "region w")?;
                let h = parse_u32(parts[3], "region h")?;

                Ok(Self::Absolute { x, y, w, h })
            }
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Square => write!(f, "square"),
            Self::Absolute { x, y, w, h } => write!(f, "{x},{y},{w},{h}"),
            Self::Percent { x, y, w, h } => write!(f, "pct:{x},{y},{w},{h}"),
        }
    }
}

fn parse_u32(s: &str, field: &str) -> Result<u32, IiifError> {
    s.parse::<u32>()
        .map_err(|_| IiifError::BadRequest(format!("Invalid integer for {field}: {s}")))
}

fn parse_float(s: &str, field: &str) -> Result<f64, IiifError> {
    s.parse::<f64>()
        .map_err(|_| IiifError::BadRequest(format!("Invalid number for {field}: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full() {
        assert_eq!("full".parse::<Region>().unwrap(), Region::Full);
    }

    #[test]
    fn parse_square() {
        assert_eq!("square".parse::<Region>().unwrap(), Region::Square);
    }

    #[test]
    fn parse_absolute() {
        let r: Region = "10,20,300,400".parse().unwrap();
        assert_eq!(
            r,
            Region::Absolute {
                x: 10,
                y: 20,
                w: 300,
                h: 400
            }
        );
    }

    #[test]
    fn parse_percent() {
        let r: Region = "pct:10.5,20,30,40.5".parse().unwrap();
        assert_eq!(
            r,
            Region::Percent {
                x: 10.5,
                y: 20.0,
                w: 30.0,
                h: 40.5
            }
        );
    }

    #[test]
    fn reject_invalid() {
        assert!("pct:10,20".parse::<Region>().is_err());
        assert!("10,20".parse::<Region>().is_err());
        assert!("invalid".parse::<Region>().is_err());
    }

    #[test]
    fn resolve_full() {
        let r = Region::Full.resolve(800, 600).unwrap();
        assert_eq!(r, (0, 0, 800, 600));
    }

    #[test]
    fn resolve_square_landscape() {
        let r = Region::Square.resolve(800, 600).unwrap();
        assert_eq!(r, (100, 0, 600, 600));
    }

    #[test]
    fn resolve_square_portrait() {
        let r = Region::Square.resolve(400, 600).unwrap();
        assert_eq!(r, (0, 100, 400, 400));
    }

    #[test]
    fn resolve_clamps_to_bounds() {
        let r = Region::Absolute {
            x: 700,
            y: 500,
            w: 200,
            h: 200,
        }
        .resolve(800, 600)
        .unwrap();
        assert_eq!(r, (700, 500, 100, 100));
    }

    #[test]
    fn resolve_rejects_zero_dimensions() {
        assert!(Region::Absolute {
            x: 0,
            y: 0,
            w: 0,
            h: 100
        }
        .resolve(800, 600)
        .is_err());
    }

    #[test]
    fn resolve_rejects_outside_bounds() {
        assert!(Region::Absolute {
            x: 800,
            y: 0,
            w: 100,
            h: 100
        }
        .resolve(800, 600)
        .is_err());
    }
}
