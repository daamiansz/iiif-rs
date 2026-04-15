use std::fmt;
use std::str::FromStr;

use iiif_core::error::IiifError;

/// IIIF Image API size parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Size {
    pub mode: SizeMode,
    /// Whether upscaling beyond the extracted region is allowed (`^` prefix).
    pub upscale: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SizeMode {
    /// Maximum dimensions without exceeding server limits.
    Max,
    /// Scale to a specific width, maintaining aspect ratio.
    Width(u32),
    /// Scale to a specific height, maintaining aspect ratio.
    Height(u32),
    /// Scale to a percentage of the extracted region.
    Percent(f64),
    /// Scale to exact width and height (may distort aspect ratio).
    Exact { w: u32, h: u32 },
    /// Scale to fit within a bounding box, maintaining aspect ratio.
    BestFit { w: u32, h: u32 },
}

impl Size {
    /// Resolve to final `(width, height)` given the extracted region dimensions
    /// and server-configured limits.
    pub fn resolve(
        &self,
        region_w: u32,
        region_h: u32,
        max_width: Option<u32>,
        max_height: Option<u32>,
        max_area: Option<u64>,
    ) -> Result<(u32, u32), IiifError> {
        let (mut w, mut h) = match &self.mode {
            SizeMode::Max => (region_w, region_h),

            SizeMode::Width(tw) => {
                let tw = *tw;
                if !self.upscale && tw > region_w {
                    return Err(IiifError::BadRequest(
                        "Requested width exceeds region width and upscaling is not enabled"
                            .to_string(),
                    ));
                }
                let th = scale_dimension(region_h, tw, region_w);
                (tw, th)
            }

            SizeMode::Height(th) => {
                let th = *th;
                if !self.upscale && th > region_h {
                    return Err(IiifError::BadRequest(
                        "Requested height exceeds region height and upscaling is not enabled"
                            .to_string(),
                    ));
                }
                let tw = scale_dimension(region_w, th, region_h);
                (tw, th)
            }

            SizeMode::Percent(pct) => {
                if !self.upscale && *pct > 100.0 {
                    return Err(IiifError::BadRequest(
                        "Percentage exceeds 100% and upscaling is not enabled".to_string(),
                    ));
                }
                if *pct <= 0.0 {
                    return Err(IiifError::BadRequest(
                        "Percentage must be greater than zero".to_string(),
                    ));
                }
                let tw = (region_w as f64 * pct / 100.0).round() as u32;
                let th = (region_h as f64 * pct / 100.0).round() as u32;
                (tw.max(1), th.max(1))
            }

            SizeMode::Exact { w: tw, h: th } => {
                if !self.upscale && (*tw > region_w || *th > region_h) {
                    return Err(IiifError::BadRequest(
                        "Requested dimensions exceed region and upscaling is not enabled"
                            .to_string(),
                    ));
                }
                (*tw, *th)
            }

            SizeMode::BestFit { w: bw, h: bh } => {
                if !self.upscale && (*bw > region_w || *bh > region_h) {
                    return Err(IiifError::BadRequest(
                        "Requested bounding box exceeds region and upscaling is not enabled"
                            .to_string(),
                    ));
                }
                fit_within(region_w, region_h, *bw, *bh)
            }
        };

        // Apply server limits
        if let Some(mw) = max_width {
            if w > mw {
                h = scale_dimension(h, mw, w);
                w = mw;
            }
        }
        if let Some(mh) = max_height {
            if h > mh {
                w = scale_dimension(w, mh, h);
                h = mh;
            }
        }
        if let Some(ma) = max_area {
            let area = w as u64 * h as u64;
            if area > ma {
                let scale = (ma as f64 / area as f64).sqrt();
                w = (w as f64 * scale).floor() as u32;
                h = (h as f64 * scale).floor() as u32;
            }
        }

        // Ensure at least 1×1
        w = w.max(1);
        h = h.max(1);

        Ok((w, h))
    }
}

/// Scale `value` proportionally: `value * target / reference`.
fn scale_dimension(value: u32, target: u32, reference: u32) -> u32 {
    if reference == 0 {
        return 1;
    }
    ((value as f64 * target as f64) / reference as f64)
        .round()
        .max(1.0) as u32
}

/// Calculate dimensions that fit within `max_w × max_h` while maintaining the
/// aspect ratio of `src_w × src_h`.
fn fit_within(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let scale_w = max_w as f64 / src_w as f64;
    let scale_h = max_h as f64 / src_h as f64;
    let scale = scale_w.min(scale_h);
    let w = (src_w as f64 * scale).round().max(1.0) as u32;
    let h = (src_h as f64 * scale).round().max(1.0) as u32;
    (w, h)
}

impl FromStr for Size {
    type Err = IiifError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (input, upscale) = if let Some(rest) = s.strip_prefix('^') {
            (rest, true)
        } else {
            (s, false)
        };

        let mode = if input == "max" {
            SizeMode::Max
        } else if let Some(rest) = input.strip_prefix("pct:") {
            let n = rest
                .parse::<f64>()
                .map_err(|_| IiifError::BadRequest(format!("Invalid percentage value: {rest}")))?;
            SizeMode::Percent(n)
        } else if let Some(rest) = input.strip_prefix('!') {
            let (w, h) = parse_wh(rest)?;
            SizeMode::BestFit { w, h }
        } else if let Some(w_str) = input.strip_suffix(',') {
            let w = w_str
                .parse::<u32>()
                .map_err(|_| IiifError::BadRequest(format!("Invalid width value: {w_str}")))?;
            SizeMode::Width(w)
        } else if let Some(h_str) = input.strip_prefix(',') {
            let h = h_str
                .parse::<u32>()
                .map_err(|_| IiifError::BadRequest(format!("Invalid height value: {h_str}")))?;
            SizeMode::Height(h)
        } else if input.contains(',') {
            let (w, h) = parse_wh(input)?;
            SizeMode::Exact { w, h }
        } else {
            return Err(IiifError::BadRequest(format!(
                "Invalid size parameter: {s}"
            )));
        };

        Ok(Self { mode, upscale })
    }
}

fn parse_wh(s: &str) -> Result<(u32, u32), IiifError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(IiifError::BadRequest(format!("Expected w,h but got: {s}")));
    }
    let w = parts[0]
        .parse::<u32>()
        .map_err(|_| IiifError::BadRequest(format!("Invalid width: {}", parts[0])))?;
    let h = parts[1]
        .parse::<u32>()
        .map_err(|_| IiifError::BadRequest(format!("Invalid height: {}", parts[1])))?;
    if w == 0 || h == 0 {
        return Err(IiifError::BadRequest(
            "Width and height must be greater than zero".to_string(),
        ));
    }
    Ok((w, h))
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.upscale {
            write!(f, "^")?;
        }
        match &self.mode {
            SizeMode::Max => write!(f, "max"),
            SizeMode::Width(w) => write!(f, "{w},"),
            SizeMode::Height(h) => write!(f, ",{h}"),
            SizeMode::Percent(n) => write!(f, "pct:{n}"),
            SizeMode::Exact { w, h } => write!(f, "{w},{h}"),
            SizeMode::BestFit { w, h } => write!(f, "!{w},{h}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_max() {
        let s: Size = "max".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Max);
        assert!(!s.upscale);
    }

    #[test]
    fn parse_upscale_max() {
        let s: Size = "^max".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Max);
        assert!(s.upscale);
    }

    #[test]
    fn parse_width() {
        let s: Size = "300,".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Width(300));
        assert!(!s.upscale);
    }

    #[test]
    fn parse_height() {
        let s: Size = ",200".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Height(200));
    }

    #[test]
    fn parse_percent() {
        let s: Size = "pct:50".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Percent(50.0));
    }

    #[test]
    fn parse_exact() {
        let s: Size = "300,200".parse().unwrap();
        assert_eq!(s.mode, SizeMode::Exact { w: 300, h: 200 });
    }

    #[test]
    fn parse_best_fit() {
        let s: Size = "!300,200".parse().unwrap();
        assert_eq!(s.mode, SizeMode::BestFit { w: 300, h: 200 });
    }

    #[test]
    fn parse_upscale_best_fit() {
        let s: Size = "^!300,200".parse().unwrap();
        assert_eq!(s.mode, SizeMode::BestFit { w: 300, h: 200 });
        assert!(s.upscale);
    }

    #[test]
    fn resolve_max_no_limits() {
        let s = Size {
            mode: SizeMode::Max,
            upscale: false,
        };
        assert_eq!(s.resolve(800, 600, None, None, None).unwrap(), (800, 600));
    }

    #[test]
    fn resolve_max_with_limits() {
        let s = Size {
            mode: SizeMode::Max,
            upscale: false,
        };
        assert_eq!(
            s.resolve(800, 600, Some(400), None, None).unwrap(),
            (400, 300)
        );
    }

    #[test]
    fn resolve_width_scales_proportionally() {
        let s = Size {
            mode: SizeMode::Width(400),
            upscale: false,
        };
        assert_eq!(s.resolve(800, 600, None, None, None).unwrap(), (400, 300));
    }

    #[test]
    fn resolve_width_rejects_upscale() {
        let s = Size {
            mode: SizeMode::Width(1000),
            upscale: false,
        };
        assert!(s.resolve(800, 600, None, None, None).is_err());
    }

    #[test]
    fn resolve_best_fit() {
        let s = Size {
            mode: SizeMode::BestFit { w: 200, h: 200 },
            upscale: false,
        };
        let (w, h) = s.resolve(800, 600, None, None, None).unwrap();
        assert!(w <= 200 && h <= 200);
        // 800:600 = 4:3, fit in 200x200 -> 200x150
        assert_eq!((w, h), (200, 150));
    }
}
