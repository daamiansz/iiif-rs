use serde::Serialize;

/// IIIF Change Discovery API 1.0 ordered collection.
///
/// `last` is required by spec when there is at least one page; for an empty
/// store we drop it (an empty `OrderedCollection` with `totalItems: 0` and
/// no pages is a legal terminal state).
#[derive(Debug, Clone, Serialize)]
pub struct OrderedCollection {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(rename = "totalItems")]
    pub total_items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<PageRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<PageRef>,
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
///
/// `Move` activities require `target` (new URI). `Refresh` activities use
/// `startTime` instead of `endTime` and have no `object`/`target`.
#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<ActivityObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ActivityObject>,
    #[serde(rename = "endTime", skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(rename = "startTime", skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// The object that was created/updated/deleted/moved.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityObject {
    pub id: String,
    #[serde(rename = "type")]
    pub object_type: String,
}

/// Optional actor that performed the activity.
#[derive(Debug, Clone, Serialize)]
pub struct Actor {
    pub id: String,
    #[serde(rename = "type")]
    pub actor_type: String,
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
        let (first, last) = if total == 0 {
            (None, None)
        } else {
            let last_idx = page_count.saturating_sub(1);
            (
                Some(PageRef {
                    id: format!("{base_url}/activity/page/0"),
                    resource_type: "OrderedCollectionPage".to_string(),
                }),
                Some(PageRef {
                    id: format!("{base_url}/activity/page/{last_idx}"),
                    resource_type: "OrderedCollectionPage".to_string(),
                }),
            )
        };

        Self {
            context: DISCOVERY_CONTEXT.to_string(),
            id: format!("{base_url}/activity/all-changes"),
            resource_type: "OrderedCollection".to_string(),
            total_items: total,
            first,
            last,
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
    fn empty_collection_omits_first_and_last() {
        let col = OrderedCollection::new("http://localhost:8080", 0, 0);
        let json = serde_json::to_string(&col).unwrap();
        // No phantom `page/0` reference for an empty store.
        assert!(!json.contains("page/0"));
        assert!(json.contains("\"totalItems\":0"));
    }

    #[test]
    fn create_activity_serializes() {
        let activity = Activity {
            id: "http://localhost:8080/activity/1".to_string(),
            activity_type: "Create".to_string(),
            object: Some(ActivityObject {
                id: "http://localhost:8080/manifest/test".to_string(),
                object_type: "Manifest".to_string(),
            }),
            target: None,
            end_time: Some("2026-04-15T12:00:00Z".to_string()),
            start_time: None,
            actor: None,
            summary: None,
        };
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("Create"));
        assert!(json.contains("endTime"));
        assert!(!json.contains("startTime"));
        assert!(!json.contains("target"));
    }

    #[test]
    fn move_activity_carries_target() {
        let activity = Activity {
            id: "http://localhost:8080/activity/2".to_string(),
            activity_type: "Move".to_string(),
            object: Some(ActivityObject {
                id: "http://localhost:8080/manifest/old".to_string(),
                object_type: "Manifest".to_string(),
            }),
            target: Some(ActivityObject {
                id: "http://localhost:8080/manifest/new".to_string(),
                object_type: "Manifest".to_string(),
            }),
            end_time: Some("2026-04-15T12:00:00Z".to_string()),
            start_time: None,
            actor: None,
            summary: None,
        };
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("Move"));
        assert!(json.contains("\"target\""));
        assert!(json.contains("manifest/new"));
    }

    #[test]
    fn refresh_activity_uses_start_time() {
        let activity = Activity {
            id: "http://localhost:8080/activity/3".to_string(),
            activity_type: "Refresh".to_string(),
            object: None,
            target: None,
            end_time: None,
            start_time: Some("2026-04-15T00:00:00Z".to_string()),
            actor: None,
            summary: None,
        };
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("Refresh"));
        assert!(json.contains("startTime"));
        assert!(!json.contains("endTime"));
        assert!(!json.contains("\"object\""));
        assert!(!json.contains("\"target\""));
    }
}
