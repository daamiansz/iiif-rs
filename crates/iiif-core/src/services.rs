//! IIIF service descriptors as a single typed enum.
//!
//! Replaces the scattered ad-hoc `serde_json::Value` blobs that v0.2.x used
//! for auth/image/search service descriptors. Tagged via `#[serde(tag="type")]`
//! so each variant serialises with its IIIF type name.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Language-tagged string map, e.g. `{"en": ["Login"]}` / `{"none": ["..."]}`.
pub type LanguageMap = BTreeMap<String, Vec<String>>;

/// All IIIF service types this server can emit.
///
/// Serialised with the variant name as the JSON `type` discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Service {
    ImageService3(ImageService3),
    AuthProbeService2(AuthProbeService2),
    AuthAccessService2(AuthAccessService2),
    AuthAccessTokenService2(AuthAccessTokenService2),
    AuthLogoutService2(AuthLogoutService2),
    SearchService2(SearchService2),
    AutoCompleteService2(AutoCompleteService2),
}

/// Image API 3.0 service entry on a content resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageService3 {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl ImageService3 {
    pub fn level2(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            profile: Some("level2".to_string()),
        }
    }
}

/// Probe service — placed in the protected resource's `service[]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProbeService2 {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub service: Vec<Service>,
    #[serde(rename = "errorHeading", skip_serializing_if = "Option::is_none")]
    pub error_heading: Option<LanguageMap>,
    #[serde(rename = "errorNote", skip_serializing_if = "Option::is_none")]
    pub error_note: Option<LanguageMap>,
}

/// Access service — `id` required for `active`/`kiosk`, MUST be omitted for `external`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthAccessService2 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub profile: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub service: Vec<Service>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<LanguageMap>,
    #[serde(rename = "confirmLabel", skip_serializing_if = "Option::is_none")]
    pub confirm_label: Option<LanguageMap>,
}

/// Token sub-service. Spec 2.0 does NOT define `profile` on these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthAccessTokenService2 {
    pub id: String,
    #[serde(rename = "errorHeading", skip_serializing_if = "Option::is_none")]
    pub error_heading: Option<LanguageMap>,
    #[serde(rename = "errorNote", skip_serializing_if = "Option::is_none")]
    pub error_note: Option<LanguageMap>,
}

/// Logout sub-service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLogoutService2 {
    pub id: String,
    pub label: LanguageMap,
}

/// Content Search 2.0 service. AutoCompleteService2 is nested inside `service[]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchService2 {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub service: Vec<Service>,
}

/// Content Search 2.0 autocomplete sub-service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCompleteService2 {
    pub id: String,
}

/// Helper: build a `LanguageMap` with a single language code and value.
pub fn lang_map(language: &str, value: &str) -> LanguageMap {
    let mut map = BTreeMap::new();
    map.insert(language.to_string(), vec![value.to_string()]);
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_service_serializes_with_type_tag() {
        let svc = Service::ImageService3(ImageService3::level2("http://example.org/img"));
        let v = serde_json::to_value(&svc).unwrap();
        assert_eq!(v["type"], "ImageService3");
        assert_eq!(v["id"], "http://example.org/img");
        assert_eq!(v["profile"], "level2");
    }

    #[test]
    fn auth_probe_nests_access_which_nests_subservices() {
        let svc = Service::AuthProbeService2(AuthProbeService2 {
            id: "http://x/probe".to_string(),
            service: vec![Service::AuthAccessService2(AuthAccessService2 {
                id: Some("http://x/login".to_string()),
                profile: "active".to_string(),
                label: Some(lang_map("en", "Login")),
                heading: None,
                note: None,
                confirm_label: None,
                service: vec![
                    Service::AuthAccessTokenService2(AuthAccessTokenService2 {
                        id: "http://x/token".to_string(),
                        error_heading: None,
                        error_note: None,
                    }),
                    Service::AuthLogoutService2(AuthLogoutService2 {
                        id: "http://x/logout".to_string(),
                        label: lang_map("en", "Logout"),
                    }),
                ],
            })],
            error_heading: None,
            error_note: None,
        });
        let v = serde_json::to_value(&svc).unwrap();
        assert_eq!(v["type"], "AuthProbeService2");
        assert_eq!(v["service"][0]["type"], "AuthAccessService2");
        assert_eq!(v["service"][0]["profile"], "active");
        // Sub-services use `type` only, no `profile`.
        assert_eq!(v["service"][0]["service"][0]["type"], "AuthAccessTokenService2");
        assert!(v["service"][0]["service"][0].get("profile").is_none());
        assert_eq!(v["service"][0]["service"][1]["type"], "AuthLogoutService2");
    }

    #[test]
    fn search_service_nests_autocomplete() {
        let svc = Service::SearchService2(SearchService2 {
            id: "http://x/search".to_string(),
            service: vec![Service::AutoCompleteService2(AutoCompleteService2 {
                id: "http://x/autocomplete".to_string(),
            })],
        });
        let v = serde_json::to_value(&svc).unwrap();
        assert_eq!(v["type"], "SearchService2");
        assert_eq!(v["service"][0]["type"], "AutoCompleteService2");
    }
}
