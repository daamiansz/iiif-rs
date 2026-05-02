use serde::Serialize;

use iiif_core::config::ImageConfig;
use iiif_core::services::Service;

const IMAGE_CONTEXT: &str = "http://iiif.io/api/image/3/context.json";
const AUTH_CONTEXT: &str = "http://iiif.io/api/auth/2/context.json";

/// IIIF Image Information (info.json) response.
///
/// Conforms to IIIF Image API 3.0 specification.
#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    /// Either a single context URI string or an array (when auth context is prepended).
    #[serde(rename = "@context")]
    pub context: serde_json::Value,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub protocol: String,
    pub profile: String,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "maxWidth", skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    #[serde(rename = "maxHeight", skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
    #[serde(rename = "maxArea", skip_serializing_if = "Option::is_none")]
    pub max_area: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<SizeEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiles: Option<Vec<TileEntry>>,
    #[serde(rename = "extraQualities", skip_serializing_if = "Option::is_none")]
    pub extra_qualities: Option<Vec<String>>,
    #[serde(rename = "extraFormats", skip_serializing_if = "Option::is_none")]
    pub extra_formats: Option<Vec<String>>,
    #[serde(rename = "extraFeatures", skip_serializing_if = "Option::is_none")]
    pub extra_features: Option<Vec<String>>,
    #[serde(rename = "preferredFormats", skip_serializing_if = "Option::is_none")]
    pub preferred_formats: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rights: Option<String>,
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<Vec<serde_json::Value>>,
    #[serde(rename = "seeAlso", skip_serializing_if = "Option::is_none")]
    pub see_also: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<Service>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendering: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SizeEntry {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TileEntry {
    pub width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(rename = "scaleFactors")]
    pub scale_factors: Vec<u32>,
}

impl ImageInfo {
    /// Build an `ImageInfo` response for a given image.
    ///
    /// `auth_service` is the optional probe-service descriptor to embed in `service[]`
    /// for protected resources. When present, the auth API context is prepended to
    /// `@context` per IIIF Auth Flow 2.0.
    pub fn build(
        base_url: &str,
        identifier: &str,
        img_width: u32,
        img_height: u32,
        config: &ImageConfig,
        auth_service: Option<Service>,
    ) -> Self {
        let id = format!("{base_url}/{identifier}");

        let sizes = generate_sizes(img_width, img_height);

        let tiles = vec![TileEntry {
            width: config.tile_width,
            height: None,
            scale_factors: config.tile_scale_factors.clone(),
        }];

        let context = match &auth_service {
            Some(_) => serde_json::json!([AUTH_CONTEXT, IMAGE_CONTEXT]),
            None => serde_json::json!(IMAGE_CONTEXT),
        };

        let service = auth_service.map(|v| vec![v]);

        // `extraFeatures` is derived from runtime config, not hardcoded — info.json
        // must not advertise features the server is configured to refuse.
        let mut extra_features = vec![
            "baseUriRedirect".to_string(),
            "canonicalLinkHeader".to_string(),
            "cors".to_string(),
            "jsonldMediaType".to_string(),
            "mirroring".to_string(),
            "profileLinkHeader".to_string(),
            "regionByPct".to_string(),
            "regionByPx".to_string(),
            "regionSquare".to_string(),
            "rotationArbitrary".to_string(),
            "rotationBy90s".to_string(),
            "sizeByConfinedWh".to_string(),
            "sizeByH".to_string(),
            "sizeByPct".to_string(),
            "sizeByW".to_string(),
            "sizeByWh".to_string(),
        ];
        if config.allow_upscaling {
            extra_features.push("sizeUpscaling".to_string());
        }

        Self {
            context,
            id,
            resource_type: "ImageService3".to_string(),
            protocol: "http://iiif.io/api/image".to_string(),
            profile: "level2".to_string(),
            width: img_width,
            height: img_height,
            max_width: config.max_width,
            max_height: config.max_height,
            max_area: config.max_area,
            sizes: Some(sizes),
            tiles: Some(tiles),
            extra_qualities: Some(vec![
                "color".to_string(),
                "gray".to_string(),
                "bitonal".to_string(),
            ]),
            extra_formats: Some(vec!["png".to_string(), "webp".to_string()]),
            extra_features: Some(extra_features),
            preferred_formats: Some(vec!["webp".to_string(), "jpg".to_string()]),
            rights: None,
            part_of: None,
            see_also: None,
            service,
            homepage: None,
            logo: None,
            rendering: None,
            provider: None,
        }
    }
}

/// Generate a list of preferred sizes by halving dimensions until below 128px.
fn generate_sizes(width: u32, height: u32) -> Vec<SizeEntry> {
    let mut sizes = Vec::new();
    let mut w = width;
    let mut h = height;

    while w >= 128 && h >= 128 {
        sizes.push(SizeEntry {
            width: w,
            height: h,
        });
        w /= 2;
        h /= 2;
    }

    // Always include the smallest reasonable size
    if sizes.is_empty() || (w > 0 && h > 0) {
        sizes.push(SizeEntry {
            width: w.max(1),
            height: h.max(1),
        });
    }

    sizes.reverse();
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_sizes_produces_pyramid() {
        let sizes = generate_sizes(4000, 3000);
        assert!(sizes.len() > 1);
        // Sizes should be ascending
        for pair in sizes.windows(2) {
            assert!(pair[0].width < pair[1].width);
        }
        // Last entry should be the full size
        assert_eq!(sizes.last().unwrap().width, 4000);
        assert_eq!(sizes.last().unwrap().height, 3000);
    }

    #[test]
    fn info_json_serializes_correctly() {
        let config = ImageConfig {
            max_width: Some(4096),
            max_height: Some(4096),
            max_area: Some(16_777_216),
            allow_upscaling: true,
            tile_width: 512,
            tile_scale_factors: vec![1, 2, 4, 8, 16],
        };

        let info =
            ImageInfo::build("http://localhost:8080", "test123", 6000, 4000, &config, None);
        let json = serde_json::to_string_pretty(&info).unwrap();

        assert!(json.contains("\"@context\""));
        assert!(json.contains("\"ImageService3\""));
        assert!(json.contains("\"http://iiif.io/api/image\""));
        assert!(json.contains("\"level2\""));
        assert!(json.contains("\"sizeUpscaling\""));
        assert!(json.contains("\"jsonldMediaType\""));
        // No auth → context is a single string
        assert_eq!(
            info.context,
            serde_json::json!("http://iiif.io/api/image/3/context.json")
        );
        // No probe service when not protected
        assert!(info.service.is_none());
    }

    #[test]
    fn info_json_omits_size_upscaling_when_disabled() {
        let config = ImageConfig {
            max_width: Some(4096),
            max_height: Some(4096),
            max_area: Some(16_777_216),
            allow_upscaling: false,
            tile_width: 512,
            tile_scale_factors: vec![1, 2, 4, 8, 16],
        };

        let info =
            ImageInfo::build("http://localhost:8080", "test123", 6000, 4000, &config, None);
        let features = info.extra_features.unwrap();
        assert!(!features.iter().any(|f| f == "sizeUpscaling"));
    }

    #[test]
    fn info_json_with_auth_prepends_auth_context() {
        use iiif_core::services::AuthProbeService2;

        let config = ImageConfig {
            max_width: None,
            max_height: None,
            max_area: None,
            allow_upscaling: true,
            tile_width: 512,
            tile_scale_factors: vec![1, 2, 4, 8, 16],
        };
        let probe = Service::AuthProbeService2(AuthProbeService2 {
            id: "http://x/probe".to_string(),
            service: vec![],
            error_heading: None,
            error_note: None,
        });
        let info = ImageInfo::build(
            "http://localhost:8080",
            "test",
            100,
            100,
            &config,
            Some(probe),
        );

        let ctx = info.context.as_array().unwrap();
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx[0], "http://iiif.io/api/auth/2/context.json");
        assert_eq!(ctx[1], "http://iiif.io/api/image/3/context.json");
        // Serialise the info to verify the embedded service has the right type tag.
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["service"][0]["type"], "AuthProbeService2");
    }
}
