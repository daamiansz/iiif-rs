use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, GrayImage, Luma, Rgb, RgbImage, Rgba, RgbaImage};
use tracing::debug;

use iiif_core::config::ImageConfig;
use iiif_core::error::IiifError;

use crate::params::{OutputFormat, Quality, Region, Rotation, Size};

/// Execute the full IIIF image processing pipeline.
///
/// Processing order per IIIF Image API 3.0 spec:
/// Region → Size → Rotation → Quality → Format
pub fn process_image(
    source: &[u8],
    region: &Region,
    size: &Size,
    rotation: &Rotation,
    quality: &Quality,
    format: &OutputFormat,
    config: &ImageConfig,
) -> Result<Vec<u8>, IiifError> {
    let img = image::load_from_memory(source)
        .map_err(|e| IiifError::ImageProcessing(format!("Failed to decode source image: {e}")))?;

    let (orig_w, orig_h) = img.dimensions();
    debug!(width = orig_w, height = orig_h, "Loaded source image");

    // 1. Region
    let img = apply_region(img, region)?;
    let (region_w, region_h) = img.dimensions();
    debug!(width = region_w, height = region_h, "Applied region");

    // 2. Size
    let img = apply_size(img, size, region_w, region_h, config)?;
    debug!(width = img.width(), height = img.height(), "Applied size");

    // 3. Rotation
    let img = apply_rotation(img, rotation)?;
    debug!(
        width = img.width(),
        height = img.height(),
        "Applied rotation"
    );

    // 4. Quality
    let img = apply_quality(img, quality);
    debug!("Applied quality: {quality}");

    // 5. Format (encode)
    encode(img, format)
}

/// Retrieve image dimensions without performing full decode.
pub fn get_dimensions(source: &[u8]) -> Result<(u32, u32), IiifError> {
    let reader = image::ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|e| IiifError::ImageProcessing(format!("Failed to guess image format: {e}")))?;

    let dims = reader
        .into_dimensions()
        .map_err(|e| IiifError::ImageProcessing(format!("Failed to read image dimensions: {e}")))?;

    Ok(dims)
}

fn apply_region(img: DynamicImage, region: &Region) -> Result<DynamicImage, IiifError> {
    let (img_w, img_h) = img.dimensions();
    let (x, y, w, h) = region.resolve(img_w, img_h)?;

    if x == 0 && y == 0 && w == img_w && h == img_h {
        return Ok(img);
    }

    Ok(img.crop_imm(x, y, w, h))
}

fn apply_size(
    img: DynamicImage,
    size: &Size,
    region_w: u32,
    region_h: u32,
    config: &ImageConfig,
) -> Result<DynamicImage, IiifError> {
    let (target_w, target_h) = size.resolve(
        region_w,
        region_h,
        config.max_width,
        config.max_height,
        config.max_area,
    )?;

    let (current_w, current_h) = img.dimensions();
    if target_w == current_w && target_h == current_h {
        return Ok(img);
    }

    if !config.allow_upscaling && (target_w > current_w || target_h > current_h) {
        return Err(IiifError::NotImplemented(
            "Upscaling is not supported by this server".to_string(),
        ));
    }

    Ok(img.resize_exact(target_w, target_h, FilterType::Lanczos3))
}

fn apply_rotation(img: DynamicImage, rotation: &Rotation) -> Result<DynamicImage, IiifError> {
    let img = if rotation.mirror { img.fliph() } else { img };

    if rotation.degrees == 0.0 || rotation.degrees == 360.0 {
        return Ok(img);
    }

    let degrees_normalized = rotation.degrees % 360.0;

    if (degrees_normalized - 90.0).abs() < f64::EPSILON {
        Ok(img.rotate90())
    } else if (degrees_normalized - 180.0).abs() < f64::EPSILON {
        Ok(img.rotate180())
    } else if (degrees_normalized - 270.0).abs() < f64::EPSILON {
        Ok(img.rotate270())
    } else {
        Ok(rotate_arbitrary(img, degrees_normalized))
    }
}

/// Rotate an image by an arbitrary angle (in degrees) clockwise.
///
/// Uses inverse mapping with bilinear interpolation. The output image is
/// sized to the bounding box of the rotated original. Areas not covered
/// by the source are left transparent (RGBA).
fn rotate_arbitrary(img: DynamicImage, degrees: f64) -> DynamicImage {
    let radians = degrees.to_radians();
    let cos_a = radians.cos();
    let sin_a = radians.sin();

    let (w, h) = img.dimensions();
    let fw = w as f64;
    let fh = h as f64;

    // Bounding box of the rotated image
    let new_w = (fw * cos_a.abs() + fh * sin_a.abs()).ceil() as u32;
    let new_h = (fw * sin_a.abs() + fh * cos_a.abs()).ceil() as u32;

    let cx_src = fw / 2.0;
    let cy_src = fh / 2.0;
    let cx_dst = new_w as f64 / 2.0;
    let cy_dst = new_h as f64 / 2.0;

    let rgba = img.to_rgba8();
    let mut output = RgbaImage::new(new_w, new_h);

    for out_y in 0..new_h {
        for out_x in 0..new_w {
            let dx = out_x as f64 - cx_dst;
            let dy = out_y as f64 - cy_dst;

            // Inverse rotation: rotate the destination point back to source space
            let src_x = dx * cos_a + dy * sin_a + cx_src;
            let src_y = -dx * sin_a + dy * cos_a + cy_src;

            if src_x >= 0.0 && src_x < (w - 1) as f64 && src_y >= 0.0 && src_y < (h - 1) as f64 {
                let pixel = bilinear_sample(&rgba, src_x, src_y);
                output.put_pixel(out_x, out_y, pixel);
            }
        }
    }

    DynamicImage::ImageRgba8(output)
}

/// Sample a pixel using bilinear interpolation.
fn bilinear_sample(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);

    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let mut result = [0u8; 4];
    for i in 0..4 {
        let v = (1.0 - fx) * (1.0 - fy) * p00[i] as f64
            + fx * (1.0 - fy) * p10[i] as f64
            + (1.0 - fx) * fy * p01[i] as f64
            + fx * fy * p11[i] as f64;
        result[i] = v.round().clamp(0.0, 255.0) as u8;
    }
    Rgba(result)
}

fn apply_quality(img: DynamicImage, quality: &Quality) -> DynamicImage {
    match quality {
        Quality::Default | Quality::Color => img,
        Quality::Gray => img.grayscale(),
        Quality::Bitonal => {
            let gray = img.to_luma8();
            let threshold = 128u8;
            let bitonal = GrayImage::from_fn(gray.width(), gray.height(), |x, y| {
                let Luma([v]) = gray.get_pixel(x, y);
                if *v >= threshold {
                    Luma([255])
                } else {
                    Luma([0])
                }
            });
            DynamicImage::ImageLuma8(bitonal)
        }
    }
}

/// Composite an RGBA image onto a white background, producing RGB.
fn composite_on_white(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = RgbImage::new(w, h);

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let Rgba([r, g, b, a]) = *pixel;
        let alpha = a as f64 / 255.0;
        let bg = 255.0;
        let ro = (r as f64 * alpha + bg * (1.0 - alpha)).round() as u8;
        let go = (g as f64 * alpha + bg * (1.0 - alpha)).round() as u8;
        let bo = (b as f64 * alpha + bg * (1.0 - alpha)).round() as u8;
        rgb.put_pixel(x, y, Rgb([ro, go, bo]));
    }

    DynamicImage::ImageRgb8(rgb)
}

fn encode(img: DynamicImage, format: &OutputFormat) -> Result<Vec<u8>, IiifError> {
    // Formats without alpha support need compositing onto a white background
    let img = if img.color().has_alpha() {
        match format {
            OutputFormat::Jpg | OutputFormat::Tif | OutputFormat::Gif => composite_on_white(&img),
            OutputFormat::Png | OutputFormat::Webp => img,
        }
    } else {
        img
    };

    let mut buffer = Cursor::new(Vec::new());

    img.write_to(&mut buffer, format.to_image_format())
        .map_err(|e| {
            IiifError::ImageProcessing(format!("Failed to encode image as {format}: {e}"))
        })?;

    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{Size, SizeMode};

    fn test_config() -> ImageConfig {
        ImageConfig {
            max_width: Some(4096),
            max_height: Some(4096),
            max_area: Some(16_777_216),
            allow_upscaling: true,
            tile_width: 512,
            tile_scale_factors: vec![1, 2, 4],
        }
    }

    fn create_test_image(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgb8(w, h);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn full_pipeline_identity() {
        let src = create_test_image(100, 80);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 0.0,
            },
            &Quality::Default,
            &OutputFormat::Png,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.width(), 100);
        assert_eq!(decoded.height(), 80);
    }

    #[test]
    fn pipeline_region_and_resize() {
        let src = create_test_image(200, 200);
        let result = process_image(
            &src,
            &Region::Absolute {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            &Size {
                mode: SizeMode::Width(50),
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 0.0,
            },
            &Quality::Default,
            &OutputFormat::Jpg,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }

    #[test]
    fn pipeline_grayscale() {
        let src = create_test_image(50, 50);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 0.0,
            },
            &Quality::Gray,
            &OutputFormat::Png,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.width(), 50);
    }

    #[test]
    fn pipeline_rotate_90() {
        let src = create_test_image(100, 50);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 90.0,
            },
            &Quality::Default,
            &OutputFormat::Png,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 100);
    }

    #[test]
    fn pipeline_arbitrary_rotation() {
        let src = create_test_image(100, 100);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 45.0,
            },
            &Quality::Default,
            &OutputFormat::Png,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        // 100x100 rotated 45° → bounding box ~142x142
        assert!(decoded.width() > 100);
        assert!(decoded.height() > 100);
    }

    #[test]
    fn pipeline_arbitrary_rotation_jpeg_white_bg() {
        let src = create_test_image(80, 60);
        // Arbitrary rotation + JPEG = transparent areas composited on white
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 30.0,
            },
            &Quality::Default,
            &OutputFormat::Jpg,
            &test_config(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn pipeline_mirror_only() {
        let src = create_test_image(100, 80);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: true,
                degrees: 0.0,
            },
            &Quality::Default,
            &OutputFormat::Png,
            &test_config(),
        )
        .unwrap();

        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.width(), 100);
        assert_eq!(decoded.height(), 80);
    }

    #[test]
    fn pipeline_bitonal() {
        let src = create_test_image(50, 50);
        let result = process_image(
            &src,
            &Region::Full,
            &Size {
                mode: SizeMode::Max,
                upscale: false,
            },
            &Rotation {
                mirror: false,
                degrees: 0.0,
            },
            &Quality::Bitonal,
            &OutputFormat::Png,
            &test_config(),
        );
        assert!(result.is_ok());
    }
}
