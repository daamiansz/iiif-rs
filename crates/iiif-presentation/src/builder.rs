use std::io::Cursor;

use tracing::debug;

use iiif_core::config::AppConfig;
use iiif_core::error::IiifError;
use iiif_core::storage::ImageStorage;

use crate::types::*;

/// Build a Manifest for a single image, auto-linking to the Image API service.
pub fn build_manifest_for_image(
    identifier: &str,
    img_width: u32,
    img_height: u32,
    config: &AppConfig,
) -> Manifest {
    let base = &config.server.base_url;
    let manifest_id = format!("{base}/manifest/{identifier}");
    let canvas_id = format!("{base}/canvas/{identifier}/p1");
    let anno_page_id = format!("{base}/annotation-page/{identifier}/p1");
    let anno_id = format!("{base}/annotation/{identifier}/p1-image");
    let image_service_id = format!("{base}/{identifier}");
    let image_id = format!("{base}/{identifier}/full/max/0/default.jpg");
    let thumb_id = format!("{base}/{identifier}/full/200,/0/default.jpg");

    let thumb_w = 200;
    let thumb_h = (img_height as f64 / img_width as f64 * thumb_w as f64).round() as u32;

    Manifest {
        context: ContextValue::default(),
        id: manifest_id,
        resource_type: "Manifest".to_string(),
        label: lang("none", identifier),
        summary: None,
        metadata: None,
        rights: None,
        required_statement: None,
        provider: None,
        thumbnail: Some(vec![ContentResource {
            id: thumb_id,
            resource_type: "Image".to_string(),
            format: Some("image/jpeg".to_string()),
            width: Some(thumb_w),
            height: Some(thumb_h),
            duration: None,
            service: Some(vec![Service {
                id: image_service_id.clone(),
                service_type: "ImageService3".to_string(),
                profile: Some("level2".to_string()),
            }]),
            label: None,
        }]),
        viewing_direction: Some("left-to-right".to_string()),
        behavior: None,
        nav_date: None,
        items: vec![Canvas {
            id: canvas_id.clone(),
            resource_type: "Canvas".to_string(),
            label: Some(lang("none", identifier)),
            width: Some(img_width),
            height: Some(img_height),
            duration: None,
            thumbnail: None,
            metadata: None,
            items: Some(vec![AnnotationPage {
                id: anno_page_id,
                resource_type: "AnnotationPage".to_string(),
                items: Some(vec![Annotation {
                    id: anno_id,
                    resource_type: "Annotation".to_string(),
                    motivation: "painting".to_string(),
                    body: ContentResource {
                        id: image_id,
                        resource_type: "Image".to_string(),
                        format: Some("image/jpeg".to_string()),
                        width: Some(img_width),
                        height: Some(img_height),
                        duration: None,
                        service: Some(vec![Service {
                            id: image_service_id,
                            service_type: "ImageService3".to_string(),
                            profile: Some("level2".to_string()),
                        }]),
                        label: None,
                    },
                    target: canvas_id,
                }]),
            }]),
            annotations: None,
        }],
        structures: None,
        homepage: None,
        logo: None,
        rendering: None,
        see_also: None,
        service: None,
        services: None,
        part_of: Some(vec![PartOf {
            id: format!("{base}/collection/top"),
            resource_type: "Collection".to_string(),
            label: None,
        }]),
        start: None,
    }
}

/// Build the root Collection listing all images as Manifests.
pub fn build_root_collection(identifiers: &[(String, u32, u32)], config: &AppConfig) -> Collection {
    let base = &config.server.base_url;

    let items: Vec<CollectionItem> = identifiers
        .iter()
        .map(|(id, w, h)| {
            let thumb_w = 200u32;
            let thumb_h = (*h as f64 / *w as f64 * thumb_w as f64).round() as u32;

            CollectionItem {
                id: format!("{base}/manifest/{id}"),
                resource_type: "Manifest".to_string(),
                label: lang("none", id),
                thumbnail: Some(vec![ContentResource {
                    id: format!("{base}/{id}/full/200,/0/default.jpg"),
                    resource_type: "Image".to_string(),
                    format: Some("image/jpeg".to_string()),
                    width: Some(thumb_w),
                    height: Some(thumb_h.max(1)),
                    duration: None,
                    service: Some(vec![Service {
                        id: format!("{base}/{id}"),
                        service_type: "ImageService3".to_string(),
                        profile: Some("level2".to_string()),
                    }]),
                    label: None,
                }]),
            }
        })
        .collect();

    Collection {
        context: ContextValue::default(),
        id: format!("{base}/collection/top"),
        resource_type: "Collection".to_string(),
        label: lang("en", "All Images"),
        summary: Some(lang("en", &format!("{} items", items.len()))),
        metadata: None,
        rights: None,
        required_statement: None,
        provider: None,
        thumbnail: None,
        viewing_direction: None,
        behavior: None,
        items,
        homepage: None,
        logo: None,
        part_of: None,
        services: None,
    }
}

/// Scan the storage for all images and return their identifiers with dimensions.
pub fn scan_images(
    storage: &dyn ImageStorage,
    images_dir: &str,
) -> Result<Vec<(String, u32, u32)>, IiifError> {
    let dir = std::path::Path::new(images_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| IiifError::Storage(format!("Failed to read images directory: {e}")))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "tif" | "tiff" | "gif" | "webp"
        ) {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        match storage.read_image(&stem) {
            Ok(bytes) => {
                let reader = image::ImageReader::new(Cursor::new(&bytes))
                    .with_guessed_format()
                    .ok();
                if let Some(reader) = reader {
                    if let Ok((w, h)) = reader.into_dimensions() {
                        debug!(identifier = %stem, width = w, height = h, "Scanned image");
                        results.push((stem, w, h));
                    }
                }
            }
            Err(_) => continue,
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}
