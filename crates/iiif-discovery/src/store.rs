use std::sync::RwLock;

use chrono::{SecondsFormat, Utc};

use crate::types::{Activity, ActivityObject};

/// In-memory store for change activities.
///
/// Activities are stored chronologically (oldest first, newest last)
/// per the IIIF Change Discovery API 1.0 spec.
pub struct ActivityStore {
    activities: RwLock<Vec<Activity>>,
    page_size: usize,
    base_url: String,
}

impl ActivityStore {
    /// Create a store. `base_url` is used to mint absolute IRIs for activity IDs.
    pub fn new(page_size: usize, base_url: impl Into<String>) -> Self {
        Self {
            activities: RwLock::new(Vec::new()),
            page_size,
            base_url: base_url.into(),
        }
    }

    /// Record an activity that targets a single object (Create / Update / Delete / Add / Remove).
    pub fn record(&self, activity_type: &str, object_id: &str, object_type: &str) {
        let activity = Activity {
            id: self.next_activity_id(),
            activity_type: activity_type.to_string(),
            object: Some(ActivityObject {
                id: object_id.to_string(),
                object_type: object_type.to_string(),
            }),
            target: None,
            end_time: Some(current_timestamp()),
            start_time: None,
            actor: None,
            summary: None,
        };
        self.push(activity);
    }

    /// Record a `Move` activity (republishing a resource at a new URI).
    pub fn record_move(
        &self,
        from_id: &str,
        to_id: &str,
        object_type: &str,
    ) {
        let activity = Activity {
            id: self.next_activity_id(),
            activity_type: "Move".to_string(),
            object: Some(ActivityObject {
                id: from_id.to_string(),
                object_type: object_type.to_string(),
            }),
            target: Some(ActivityObject {
                id: to_id.to_string(),
                object_type: object_type.to_string(),
            }),
            end_time: Some(current_timestamp()),
            start_time: None,
            actor: None,
            summary: None,
        };
        self.push(activity);
    }

    /// Record a `Refresh` activity (full re-issue of the stream). Uses `startTime`
    /// per spec — sorts before subsequent resource activities.
    pub fn record_refresh(&self) {
        let activity = Activity {
            id: self.next_activity_id(),
            activity_type: "Refresh".to_string(),
            object: None,
            target: None,
            end_time: None,
            start_time: Some(current_timestamp()),
            actor: None,
            summary: None,
        };
        self.push(activity);
    }

    fn next_activity_id(&self) -> String {
        let next = self.activities.read().expect("activity lock").len();
        format!("{}/activity/{}", self.base_url.trim_end_matches('/'), next)
    }

    fn push(&self, activity: Activity) {
        self.activities
            .write()
            .expect("activity lock")
            .push(activity);
    }

    /// Total number of activities.
    pub fn total(&self) -> usize {
        self.activities.read().expect("activity lock").len()
    }

    /// Number of pages. Returns `0` for an empty store.
    pub fn page_count(&self) -> usize {
        let total = self.total();
        if total == 0 {
            0
        } else {
            total.div_ceil(self.page_size)
        }
    }

    /// Page size.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Get activities for a specific page (0-indexed).
    /// Pages are ordered from oldest (page 0) to newest.
    pub fn get_page(&self, page: usize) -> Vec<Activity> {
        let activities = self.activities.read().expect("activity lock");
        let start = page * self.page_size;
        let end = (start + self.page_size).min(activities.len());
        if start >= activities.len() {
            return Vec::new();
        }
        activities[start..end].to_vec()
    }
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let store = ActivityStore::new(10, "http://localhost:8080");
        store.record("Create", "http://example.org/manifest/1", "Manifest");
        store.record("Update", "http://example.org/manifest/1", "Manifest");

        assert_eq!(store.total(), 2);
        let page = store.get_page(0);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].activity_type, "Create");
        assert_eq!(page[1].activity_type, "Update");
    }

    #[test]
    fn activity_id_is_absolute_iri() {
        let store = ActivityStore::new(10, "http://localhost:8080");
        store.record("Create", "http://example.org/m/1", "Manifest");
        let page = store.get_page(0);
        assert_eq!(page[0].id, "http://localhost:8080/activity/0");
    }

    #[test]
    fn pagination() {
        let store = ActivityStore::new(2, "http://localhost:8080");
        for i in 0..5 {
            store.record("Create", &format!("manifest/{i}"), "Manifest");
        }

        assert_eq!(store.page_count(), 3);
        assert_eq!(store.get_page(0).len(), 2);
        assert_eq!(store.get_page(1).len(), 2);
        assert_eq!(store.get_page(2).len(), 1);
        assert!(store.get_page(3).is_empty());
    }

    #[test]
    fn empty_store_has_zero_pages() {
        let store = ActivityStore::new(10, "http://localhost:8080");
        assert_eq!(store.page_count(), 0);
    }

    #[test]
    fn timestamp_format() {
        let ts = current_timestamp();
        // ISO 8601: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20);
    }

    #[test]
    fn move_helper_emits_target() {
        let store = ActivityStore::new(10, "http://localhost:8080");
        store.record_move(
            "http://example.org/old",
            "http://example.org/new",
            "Manifest",
        );
        let activity = &store.get_page(0)[0];
        assert_eq!(activity.activity_type, "Move");
        assert_eq!(
            activity.target.as_ref().unwrap().id,
            "http://example.org/new"
        );
    }

    #[test]
    fn refresh_helper_uses_start_time() {
        let store = ActivityStore::new(10, "http://localhost:8080");
        store.record_refresh();
        let activity = &store.get_page(0)[0];
        assert_eq!(activity.activity_type, "Refresh");
        assert!(activity.start_time.is_some());
        assert!(activity.end_time.is_none());
        assert!(activity.object.is_none());
    }
}
