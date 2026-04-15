use serde::Serialize;

/// IIIF Change Discovery API 1.0 ordered collection.
#[derive(Debug, Clone, Serialize)]
pub struct OrderedCollection {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(rename = "totalItems")]
    pub total_items: usize,
    pub first: PageRef,
    pub last: PageRef,
}

/// IIIF Change Discovery API 1.0 ordered collection page.
#[derive(Debug, Clone, Serialize)]
pub struct OrderedCollectionPage {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(rename = "startIndex")]
    pub start_index: usize,
    #[serde(rename = "partOf")]
    pub part_of: PageRef,
    #[serde(rename = "orderedItems")]
    pub ordered_items: Vec<Activity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<PageRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<PageRef>,
}

/// A single activity in the stream.
#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub object: ActivityObject,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

/// The object that was created/updated/deleted.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityObject {
    pub id: String,
    #[serde(rename = "type")]
    pub object_type: String,
}

/// Reference to a page or collection.
#[derive(Debug, Clone, Serialize)]
pub struct PageRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

pub const DISCOVERY_CONTEXT: &str = "http://iiif.io/api/discovery/1/context.json";

impl OrderedCollection {
    pub fn new(base_url: &str, total: usize, page_count: usize) -> Self {
        Self {
            context: DISCOVERY_CONTEXT.to_string(),
            id: format!("{base_url}/activity/all-changes"),
            resource_type: "OrderedCollection".to_string(),
            total_items: total,
            first: PageRef {
                id: format!("{base_url}/activity/page/0"),
                resource_type: "OrderedCollectionPage".to_string(),
            },
            last: PageRef {
                id: format!("{base_url}/activity/page/{}", page_count.saturating_sub(1)),
                resource_type: "OrderedCollectionPage".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_serializes() {
        let col = OrderedCollection::new("http://localhost:8080", 42, 3);
        let json = serde_json::to_string(&col).unwrap();
        assert!(json.contains("OrderedCollection"));
        assert!(json.contains("totalItems"));
        assert!(json.contains("page/0"));
    }

    #[test]
    fn activity_serializes() {
        let activity = Activity {
            id: "http://localhost:8080/activity/1".to_string(),
            activity_type: "Create".to_string(),
            object: ActivityObject {
                id: "http://localhost:8080/manifest/test".to_string(),
                object_type: "Manifest".to_string(),
            },
            end_time: "2026-04-15T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("Create"));
        assert!(json.contains("endTime"));
    }
}
