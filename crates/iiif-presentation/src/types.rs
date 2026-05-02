use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use iiif_core::services::Service as ServiceEntry;

/// Language-tagged string map, e.g. `{"en": ["Title"], "pl": ["Tytuł"]}`.
pub type LanguageMap = BTreeMap<String, Vec<String>>;

/// Helper: create a single-language map.
pub fn lang(language: &str, value: &str) -> LanguageMap {
    let mut map = BTreeMap::new();
    map.insert(language.to_string(), vec![value.to_string()]);
    map
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// IIIF Presentation API 3.0 Manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "@context")]
    pub context: ContextValue,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub label: LanguageMap,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rights: Option<String>,
    #[serde(rename = "requiredStatement", skip_serializing_if = "Option::is_none")]
    pub required_statement: Option<MetadataEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Vec<Provider>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Vec<ContentResource>>,
    #[serde(rename = "viewingDirection", skip_serializing_if = "Option::is_none")]
    pub viewing_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior: Option<Vec<String>>,
    #[serde(rename = "navDate", skip_serializing_if = "Option::is_none")]
    pub nav_date: Option<String>,

    pub items: Vec<Canvas>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub structures: Option<Vec<Range>>,

    // Linking properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Vec<ExternalResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<Vec<ContentResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendering: Option<Vec<ExternalResource>>,
    #[serde(rename = "seeAlso", skip_serializing_if = "Option::is_none")]
    pub see_also: Option<Vec<ExternalResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<Service>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<Vec<PartOf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

/// IIIF Presentation API 3.0 Collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    #[serde(rename = "@context")]
    pub context: ContextValue,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub label: LanguageMap,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rights: Option<String>,
    #[serde(rename = "requiredStatement", skip_serializing_if = "Option::is_none")]
    pub required_statement: Option<MetadataEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Vec<Provider>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Vec<ContentResource>>,
    #[serde(rename = "viewingDirection", skip_serializing_if = "Option::is_none")]
    pub viewing_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior: Option<Vec<String>>,

    pub items: Vec<CollectionItem>,

    // Linking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Vec<ExternalResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<Vec<ContentResource>>,
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<Vec<PartOf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
}

/// An item within a Collection — either a Manifest reference or a nested Collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionItem {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub label: LanguageMap,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Vec<ContentResource>>,
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

/// A single view/page/scene within a Manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Vec<ContentResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataEntry>>,

    /// Annotation pages with `motivation: "painting"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<AnnotationPage>>,

    /// Annotation pages with non-painting content (comments, transcriptions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<AnnotationPage>>,
}

// ---------------------------------------------------------------------------
// Annotation
// ---------------------------------------------------------------------------

/// An ordered list of Annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationPage {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<Annotation>>,
}

/// Associates a content resource with a Canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub motivation: String,
    pub body: ContentResource,
    pub target: String,
}

// ---------------------------------------------------------------------------
// Range (Table of Contents)
// ---------------------------------------------------------------------------

/// Structural grouping of Canvases (chapters, sections, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<RangeItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplementary: Option<serde_json::Value>,
}

/// An item within a Range — a Canvas reference, a sub-Range, or a SpecificResource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RangeItem {
    Canvas(CanvasRef),
    Range(Range),
}

/// A reference to a Canvas (possibly with a temporal/spatial fragment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

// ---------------------------------------------------------------------------
// Content Resource
// ---------------------------------------------------------------------------

/// A web resource (image, video, audio, text) associated with a Canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentResource {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<ServiceEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// IIIF service reference (Image API, Auth, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Metadata label/value pair for descriptive properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub label: LanguageMap,
    pub value: LanguageMap,
}

/// Agent/organization providing the resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub label: LanguageMap,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Vec<ExternalResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<Vec<ContentResource>>,
}

/// Link to an external resource (homepage, rendering, seeAlso).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalResource {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Reference to a parent resource (Collection or Manifest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartOf {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
}

/// The `@context` value — can be a single string or an array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextValue {
    Single(String),
    Multiple(Vec<String>),
}

impl Default for ContextValue {
    fn default() -> Self {
        Self::Single("http://iiif.io/api/presentation/3/context.json".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serializes_to_valid_json_ld() {
        let manifest = Manifest {
            context: ContextValue::default(),
            id: "http://localhost:8080/manifest/test".to_string(),
            resource_type: "Manifest".to_string(),
            label: lang("en", "Test Manifest"),
            summary: Some(lang("en", "A test manifest")),
            metadata: None,
            rights: None,
            required_statement: None,
            provider: None,
            thumbnail: None,
            viewing_direction: None,
            behavior: None,
            nav_date: None,
            items: vec![Canvas {
                id: "http://localhost:8080/canvas/p1".to_string(),
                resource_type: "Canvas".to_string(),
                label: Some(lang("en", "Page 1")),
                width: Some(1000),
                height: Some(800),
                duration: None,
                thumbnail: None,
                metadata: None,
                items: None,
                annotations: None,
            }],
            structures: None,
            homepage: None,
            logo: None,
            rendering: None,
            see_also: None,
            service: None,
            services: None,
            part_of: None,
            start: None,
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        assert!(json.contains("\"@context\""));
        assert!(json.contains("\"Manifest\""));
        assert!(json.contains("\"Canvas\""));
        assert!(json.contains("Test Manifest"));
    }

    #[test]
    fn collection_serializes() {
        let collection = Collection {
            context: ContextValue::default(),
            id: "http://localhost:8080/collection/top".to_string(),
            resource_type: "Collection".to_string(),
            label: lang("en", "All Images"),
            summary: None,
            metadata: None,
            rights: None,
            required_statement: None,
            provider: None,
            thumbnail: None,
            viewing_direction: None,
            behavior: None,
            items: vec![CollectionItem {
                id: "http://localhost:8080/manifest/test".to_string(),
                resource_type: "Manifest".to_string(),
                label: lang("en", "Test Image"),
                thumbnail: None,
            }],
            homepage: None,
            logo: None,
            part_of: None,
            services: None,
        };

        let json = serde_json::to_string(&collection).unwrap();
        assert!(json.contains("\"Collection\""));
        assert!(json.contains("All Images"));
    }

    #[test]
    fn language_map_helper() {
        let map = lang("pl", "Tytuł");
        assert_eq!(map["pl"], vec!["Tytuł".to_string()]);
    }
}
