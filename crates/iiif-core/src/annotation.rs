//! W3C Web Annotation Data Model primitives shared between Presentation 3.0
//! and Content Search 2.0.
//!
//! These types are pure data — no behaviour — so they live here in `iiif-core`
//! to avoid coupling search to presentation (or vice-versa).

use serde::{Deserialize, Serialize};

use crate::services::LanguageMap;

/// A region/point selector on a content resource. Tagged enum — JSON discriminator
/// is the `type` field. Spec defines (at least) FragmentSelector / PointSelector /
/// SvgSelector; the Content Search API adds TextQuoteSelector for hit augmentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Selector {
    /// e.g. `value: "xywh=0,0,100,100"` or `value: "t=10,30"`.
    FragmentSelector { value: String },
    /// `x`/`y` for spatial point, `t` for temporal point. At least one required.
    PointSelector {
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        t: Option<f64>,
    },
    /// Arbitrary SVG shape — `value` is the SVG markup.
    SvgSelector { value: String },
    /// Used by Content Search 2.0 for hit augmentation context.
    TextQuoteSelector {
        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
        exact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        suffix: Option<String>,
    },
}

/// A SpecificResource pins a region/portion of a `source` resource via a selector.
/// Used as Annotation `target` in hit augmentation (Search 2.0) and as Canvas
/// fragment references in Range structures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecificResource {
    pub source: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub selector: Selector,
}

impl SpecificResource {
    pub fn new(source: impl Into<String>, selector: Selector) -> Self {
        Self {
            source: source.into(),
            resource_type: "SpecificResource".to_string(),
            selector,
        }
    }
}

/// An annotation target: either a plain URI string (typical for `painting`
/// annotations referencing a whole Canvas), a single SpecificResource (a
/// region/fragment), or an array of SpecificResources for phrase matches
/// that span multiple source annotations (Search 2.0 hit augmentation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnnotationTarget {
    Id(String),
    Specific(SpecificResource),
    Multiple(Vec<SpecificResource>),
}

impl From<String> for AnnotationTarget {
    fn from(s: String) -> Self {
        Self::Id(s)
    }
}

impl From<&str> for AnnotationTarget {
    fn from(s: &str) -> Self {
        Self::Id(s.to_string())
    }
}

impl From<SpecificResource> for AnnotationTarget {
    fn from(s: SpecificResource) -> Self {
        Self::Specific(s)
    }
}

/// W3C Web Annotation Data Model: an unordered collection of AnnotationPages.
/// Used by `Range.supplementary` to group transcription/OCR pages, and by
/// Search 2.0 paged responses (via `partOf`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationCollection {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<LanguageMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<AnnotationPageRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<AnnotationPageRef>,
}

/// Reference to an AnnotationPage (`{id, type: "AnnotationPage"}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationPageRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

impl AnnotationPageRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            resource_type: "AnnotationPage".to_string(),
        }
    }
}

/// Reference to an AnnotationCollection (`{id, type: "AnnotationCollection"}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationCollectionRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

impl AnnotationCollectionRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            resource_type: "AnnotationCollection".to_string(),
        }
    }
}
