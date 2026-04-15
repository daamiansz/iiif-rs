use serde::Serialize;

/// IIIF Content Search API 2.0 search response.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub items: Vec<SearchAnnotation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<AnnotationCollection>,
}

/// Annotation in a search result.
#[derive(Debug, Clone, Serialize)]
pub struct SearchAnnotation {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub motivation: String,
    pub body: TextualBody,
    pub target: String,
}

/// TextualBody for search results.
#[derive(Debug, Clone, Serialize)]
pub struct TextualBody {
    #[serde(rename = "type")]
    pub body_type: String,
    pub value: String,
    pub format: String,
}

/// AnnotationCollection for paginated results.
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationCollection {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub total: usize,
    pub first: PageRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<PageRef>,
}

/// Reference to an AnnotationPage.
#[derive(Debug, Clone, Serialize)]
pub struct PageRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

/// IIIF Content Search API 2.0 autocomplete response.
#[derive(Debug, Clone, Serialize)]
pub struct AutocompleteResponse {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub items: Vec<TermEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
}

/// A single term in autocomplete results.
#[derive(Debug, Clone, Serialize)]
pub struct TermEntry {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

/// Search service descriptor for embedding in Manifests.
#[derive(Debug, Clone, Serialize)]
pub struct SearchServiceDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<AutocompleteServiceDescriptor>>,
}

/// Autocomplete sub-service descriptor.
#[derive(Debug, Clone, Serialize)]
pub struct AutocompleteServiceDescriptor {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
}

impl SearchServiceDescriptor {
    pub fn new(base_url: &str) -> Self {
        Self {
            id: format!("{base_url}/search"),
            service_type: "SearchService2".to_string(),
            service: Some(vec![AutocompleteServiceDescriptor {
                id: format!("{base_url}/autocomplete"),
                service_type: "AutoCompleteService2".to_string(),
            }]),
        }
    }
}

const SEARCH_CONTEXT: &str = "http://iiif.io/api/search/2/context.json";

impl SearchResponse {
    pub fn new(request_url: &str, items: Vec<SearchAnnotation>, ignored: Vec<String>) -> Self {
        Self {
            context: SEARCH_CONTEXT.to_string(),
            id: request_url.to_string(),
            resource_type: "AnnotationPage".to_string(),
            items,
            ignored,
            part_of: None,
        }
    }
}

impl AutocompleteResponse {
    pub fn new(request_url: &str, items: Vec<TermEntry>, ignored: Vec<String>) -> Self {
        Self {
            context: SEARCH_CONTEXT.to_string(),
            id: request_url.to_string(),
            resource_type: "TermPage".to_string(),
            items,
            ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_response_serializes() {
        let resp = SearchResponse::new(
            "http://localhost:8080/search?q=bird",
            vec![SearchAnnotation {
                id: "http://localhost:8080/annotation/search/1".to_string(),
                resource_type: "Annotation".to_string(),
                motivation: "painting".to_string(),
                body: TextualBody {
                    body_type: "TextualBody".to_string(),
                    value: "A bird".to_string(),
                    format: "text/plain".to_string(),
                },
                target: "http://localhost:8080/canvas/p1#xywh=0,0,100,100".to_string(),
            }],
            vec![],
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("AnnotationPage"));
        assert!(json.contains("search/2/context.json"));
    }

    #[test]
    fn autocomplete_response_serializes() {
        let resp = AutocompleteResponse::new(
            "http://localhost:8080/autocomplete?q=bir",
            vec![
                TermEntry {
                    value: "bird".to_string(),
                    total: Some(5),
                },
                TermEntry {
                    value: "birth".to_string(),
                    total: Some(2),
                },
            ],
            vec![],
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("TermPage"));
        assert!(json.contains("bird"));
    }

    #[test]
    fn service_descriptor() {
        let desc = SearchServiceDescriptor::new("http://localhost:8080");
        let json = serde_json::to_string(&desc).unwrap();
        assert!(json.contains("SearchService2"));
        assert!(json.contains("AutoCompleteService2"));
    }
}
