use std::sync::RwLock;

use crate::types::{Activity, ActivityObject};

/// In-memory store for change activities.
///
/// Activities are stored chronologically (oldest first, newest last)
/// per the IIIF Change Discovery API 1.0 spec.
pub struct ActivityStore {
    activities: RwLock<Vec<Activity>>,
    page_size: usize,
}

impl ActivityStore {
    pub fn new(page_size: usize) -> Self {
        Self {
            activities: RwLock::new(Vec::new()),
            page_size,
        }
    }

    /// Record a new activity.
    pub fn record(&self, activity_type: &str, object_id: &str, object_type: &str) {
        let activities = self.activities.read().expect("activity lock");
        let id = format!("activity/{}", activities.len());
        drop(activities);

        let activity = Activity {
            id,
            activity_type: activity_type.to_string(),
            object: ActivityObject {
                id: object_id.to_string(),
                object_type: object_type.to_string(),
            },
            end_time: current_timestamp(),
        };

        self.activities
            .write()
            .expect("activity lock")
            .push(activity);
    }

    /// Total number of activities.
    pub fn total(&self) -> usize {
        self.activities.read().expect("activity lock").len()
    }

    /// Number of pages.
    pub fn page_count(&self) -> usize {
        let total = self.total();
        if total == 0 {
            1
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
    // Simple UTC timestamp without external crate
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to approximate ISO 8601 (good enough for demo)
    let secs_per_day = 86400u64;
    let days_since_epoch = now / secs_per_day;
    let time_of_day = now % secs_per_day;

    // Simple date calculation (not accounting for leap seconds)
    let mut year = 1970u64;
    let mut remaining_days = days_since_epoch;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0usize;
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining_days < days {
            month = i;
            break;
        }
        remaining_days -= days;
    }

    let day = remaining_days + 1;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!(
        "{year:04}-{:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
        month + 1
    )
}

fn is_leap_year(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let store = ActivityStore::new(10);
        store.record("Create", "http://example.org/manifest/1", "Manifest");
        store.record("Update", "http://example.org/manifest/1", "Manifest");

        assert_eq!(store.total(), 2);
        let page = store.get_page(0);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].activity_type, "Create");
        assert_eq!(page[1].activity_type, "Update");
    }

    #[test]
    fn pagination() {
        let store = ActivityStore::new(2);
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
    fn timestamp_format() {
        let ts = current_timestamp();
        // Should be ISO 8601: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20);
    }
}
