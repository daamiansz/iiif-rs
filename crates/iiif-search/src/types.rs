use serde::Serialize;

/// IIIF Content Search API 2.0 search response (one AnnotationPage of a paginated set).
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
    #[serde(rename = "startIndex", skip_serializing_if = "Option::is_none")]
    pub start_index: Option<usize>,
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<AnnotationCollection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<PageRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<PageRef>,
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
    /// Build a paginated search response.
    ///
    /// `collection_id` is the AnnotationCollection URI (search query without `&page=`).
    /// `page_url` is `collection_id` for page 0, or `collection_id&page=N` for page N.
    /// `total` is the total number of matches across all pages; `page_size` is the
    /// per-page item count; `page` is the zero-based current page index.
    pub fn paginated(
        collection_id: &str,
        items: Vec<SearchAnnotation>,
        ignored: Vec<String>,
        total: usize,
        page: usize,
        page_size: usize,
        page_url_for: impl Fn(usize) -> String,
    ) -> Self {
        let page_count = if total == 0 {
            1
        } else {
            total.div_ceil(page_size)
        };
        let last_page = page_count.saturating_sub(1);

        let part_of = AnnotationCollection {
            id: collection_id.to_string(),
            resource_type: "AnnotationCollection".to_string(),
            total,
            first: PageRef {
                id: page_url_for(0),
                resource_type: "AnnotationPage".to_string(),
            },
            last: Some(PageRef {
                id: page_url_for(last_page),
                resource_type: "AnnotationPage".to_string(),
            }),
        };

        let next = if page < last_page {
            Some(PageRef {
                id: page_url_for(page + 1),
                resource_type: "AnnotationPage".to_string(),
            })
        } else {
            None
        };
        let prev = if page > 0 {
            Some(PageRef {
                id: page_url_for(page - 1),
                resource_type: "AnnotationPage".to_string(),
            })
        } else {
            None
        };

        Self {
            context: SEARCH_CONTEXT.to_string(),
            id: page_url_for(page),
            resource_type: "AnnotationPage".to_string(),
            items,
            ignored,
            start_index: Some(page * page_size),
            part_of: Some(part_of),
            next,
            prev,
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
        let collection = "http://localhost:8080/search?q=bird";
        let resp = SearchResponse::paginated(
            collection,
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
            1,
            0,
            50,
            |p| {
                if p == 0 {
                    collection.to_string()
                } else {
                    format!("{collection}&page={p}")
                }
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("AnnotationPage"));
        assert!(json.contains("AnnotationCollection"));
        assert!(json.contains("search/2/context.json"));
        assert!(json.contains("\"total\":1"));
        assert!(json.contains("\"startIndex\":0"));
    }

    #[test]
    fn search_response_paging_links() {
        let collection = "http://localhost:8080/search?q=x";
        let url = |p: usize| -> String {
            if p == 0 {
                collection.to_string()
            } else {
                format!("{collection}&page={p}")
            }
        };

        // 130 results, 50 per page → 3 pages (0, 1, 2). Page 1 has both prev and next.
        let resp =
            SearchResponse::paginated(collection, vec![], vec![], 130, 1, 50, url);

        let part_of = resp.part_of.as_ref().unwrap();
        assert_eq!(part_of.total, 130);
        assert_eq!(part_of.first.id, collection);
        assert_eq!(
            part_of.last.as_ref().unwrap().id,
            format!("{collection}&page=2")
        );
        assert_eq!(resp.prev.as_ref().unwrap().id, collection);
        assert_eq!(
            resp.next.as_ref().unwrap().id,
            format!("{collection}&page=2")
        );
        assert_eq!(resp.start_index, Some(50));
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
