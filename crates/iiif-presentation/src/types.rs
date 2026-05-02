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
    pub start: Option<Start>,

    #[serde(rename = "placeholderCanvas", skip_serializing_if = "Option::is_none")]
    pub placeholder_canvas: Option<Box<Canvas>>,
    #[serde(rename = "accompanyingCanvas", skip_serializing_if = "Option::is_none")]
    pub accompanying_canvas: Option<Box<Canvas>>,
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

    #[serde(rename = "placeholderCanvas", skip_serializing_if = "Option::is_none")]
    pub placeholder_canvas: Option<Box<Canvas>>,
    #[serde(rename = "accompanyingCanvas", skip_serializing_if = "Option::is_none")]
    pub accompanying_canvas: Option<Box<Canvas>>,
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
    pub target: AnnotationTarget,
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
    /// Reference to an AnnotationCollection grouping `supplementing` annotations
    /// for the canvases this Range covers (e.g. OCR/transcription pages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplementary: Option<AnnotationCollectionRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<Start>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior: Option<Vec<String>>,
}

/// An item within a Range — a Canvas reference, a sub-Range, or a SpecificResource
/// (when the range targets a sub-region or temporal fragment of a Canvas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RangeItem {
    Canvas(CanvasRef),
    Range(Range),
    Specific(SpecificResource),
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

// W3C Web Annotation Data Model primitives (Selector, SpecificResource,
// AnnotationCollection, refs, AnnotationTarget) live in `iiif-core::annotation`
// because they're shared with `iiif-search`. Re-exported here for ergonomics.
pub use iiif_core::annotation::{
    AnnotationCollection, AnnotationCollectionRef, AnnotationPageRef, AnnotationTarget, Selector,
    SpecificResource,
};

// ---------------------------------------------------------------------------
// Start (typed Manifest/Range entry-point reference)
// ---------------------------------------------------------------------------

/// Manifest/Range start: either a Canvas (id+type) or a SpecificResource
/// (id+type+source+selector) pinning a particular fragment as the opening view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Start {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
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

/// Wrap an embeddable resource (Canvas, AnnotationPage, Annotation, ...) for
/// standalone serialisation with the IIIF Presentation `@context` prepended.
///
/// Embedded resources MUST NOT carry `@context`; standalone ones MUST. Rather
/// than thread an optional context through every type, we wrap on the way out.
#[derive(Debug, Clone, Serialize)]
pub struct Standalone<T: Serialize> {
    #[serde(rename = "@context")]
    pub context: ContextValue,
    #[serde(flatten)]
    pub inner: T,
}

impl<T: Serialize> Standalone<T> {
    pub fn new(inner: T) -> Self {
        Self {
            context: ContextValue::default(),
            inner,
        }
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
                placeholder_canvas: None,
                accompanying_canvas: None,
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
            placeholder_canvas: None,
            accompanying_canvas: None,
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

    #[test]
    fn fragment_selector_serializes_with_type_tag() {
        let sel = Selector::FragmentSelector {
            value: "xywh=0,0,100,100".to_string(),
        };
        let v = serde_json::to_value(&sel).unwrap();
        assert_eq!(v["type"], "FragmentSelector");
        assert_eq!(v["value"], "xywh=0,0,100,100");
    }

    #[test]
    fn text_quote_selector_omits_optional_prefix_suffix() {
        let sel = Selector::TextQuoteSelector {
            prefix: None,
            exact: "Genesis".to_string(),
            suffix: Some(", chapter 1".to_string()),
        };
        let v = serde_json::to_value(&sel).unwrap();
        assert_eq!(v["type"], "TextQuoteSelector");
        assert_eq!(v["exact"], "Genesis");
        assert!(v.get("prefix").is_none());
        assert_eq!(v["suffix"], ", chapter 1");
    }

    #[test]
    fn point_selector_serializes_only_set_fields() {
        let sel = Selector::PointSelector {
            x: Some(10.0),
            y: Some(20.0),
            t: None,
        };
        let v = serde_json::to_value(&sel).unwrap();
        assert_eq!(v["type"], "PointSelector");
        assert_eq!(v["x"], 10.0);
        assert!(v.get("t").is_none());
    }

    #[test]
    fn specific_resource_serializes_with_selector() {
        let sr = SpecificResource::new(
            "http://example.org/anno/42",
            Selector::TextQuoteSelector {
                prefix: Some("of ".to_string()),
                exact: "creation".to_string(),
                suffix: Some(" of the world".to_string()),
            },
        );
        let v = serde_json::to_value(&sr).unwrap();
        assert_eq!(v["type"], "SpecificResource");
        assert_eq!(v["source"], "http://example.org/anno/42");
        assert_eq!(v["selector"]["type"], "TextQuoteSelector");
        assert_eq!(v["selector"]["exact"], "creation");
    }

    #[test]
    fn annotation_target_serializes_as_string_or_object() {
        let id_target: AnnotationTarget = "http://example.org/canvas/p1".into();
        assert_eq!(
            serde_json::to_value(&id_target).unwrap(),
            serde_json::json!("http://example.org/canvas/p1")
        );

        let specific = AnnotationTarget::Specific(SpecificResource::new(
            "http://example.org/anno/1",
            Selector::FragmentSelector {
                value: "xywh=0,0,10,10".to_string(),
            },
        ));
        let v = serde_json::to_value(&specific).unwrap();
        assert_eq!(v["type"], "SpecificResource");
        assert_eq!(v["selector"]["type"], "FragmentSelector");
    }

    #[test]
    fn annotation_target_multiple_serializes_as_array() {
        // Phrase match across multiple source annotations.
        let multi = AnnotationTarget::Multiple(vec![
            SpecificResource::new(
                "http://example.org/anno/1",
                Selector::TextQuoteSelector {
                    prefix: None,
                    exact: "first".to_string(),
                    suffix: None,
                },
            ),
            SpecificResource::new(
                "http://example.org/anno/2",
                Selector::TextQuoteSelector {
                    prefix: None,
                    exact: "second".to_string(),
                    suffix: None,
                },
            ),
        ]);
        let v = serde_json::to_value(&multi).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn annotation_collection_serializes() {
        let coll = AnnotationCollection {
            id: "http://example.org/coll/1".to_string(),
            resource_type: "AnnotationCollection".to_string(),
            label: Some(lang("en", "All matches")),
            first: Some(AnnotationPageRef::new("http://example.org/page/0")),
            last: Some(AnnotationPageRef::new("http://example.org/page/9")),
        };
        let v = serde_json::to_value(&coll).unwrap();
        assert_eq!(v["type"], "AnnotationCollection");
        assert_eq!(v["first"]["type"], "AnnotationPage");
        assert_eq!(v["last"]["id"], "http://example.org/page/9");
    }

    #[test]
    fn range_supplementary_typed() {
        let range = Range {
            id: "http://example.org/range/1".to_string(),
            resource_type: "Range".to_string(),
            label: Some(lang("en", "Chapter 1")),
            items: None,
            supplementary: Some(AnnotationCollectionRef::new(
                "http://example.org/transcription/ch1",
            )),
            start: None,
            behavior: None,
        };
        let v = serde_json::to_value(&range).unwrap();
        assert_eq!(v["supplementary"]["type"], "AnnotationCollection");
        assert_eq!(v["supplementary"]["id"], "http://example.org/transcription/ch1");
    }
}
