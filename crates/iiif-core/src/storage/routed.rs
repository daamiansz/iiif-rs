//! Multi-source storage router.
//!
//! Holds an ordered list of `Arc<dyn ImageStorage>` and dispatches lookups to
//! the first source that *claims* the identifier. NotFound at one source
//! falls through to the next; any other error short-circuits with the real
//! reason (so an S3 outage doesn't silently mask data behind the next
//! source).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use tracing::debug;

use crate::error::IiifError;
use crate::storage::ImageStorage;

/// Dispatches `ImageStorage` calls across multiple backends.
pub struct RoutedStorage {
    sources: Vec<Arc<dyn ImageStorage>>,
}

impl RoutedStorage {
    pub fn new(sources: Vec<Arc<dyn ImageStorage>>) -> Self {
        Self { sources }
    }

    fn pick(&self, identifier: &str) -> Vec<&Arc<dyn ImageStorage>> {
        // Sources whose sync `claims` says they could own this id, in order.
        // The `_else` branch keeps catch-all sources (no prefix_filter etc.)
        // available as a fallback after the prefix-filtered sources.
        self.sources.iter().filter(|s| s.claims(identifier)).collect()
    }
}

#[async_trait]
impl ImageStorage for RoutedStorage {
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError> {
        for source in self.pick(identifier) {
            match source.exists(identifier).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(IiifError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(false)
    }

    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError> {
        let candidates = self.pick(identifier);
        if candidates.is_empty() {
            return Err(IiifError::NotFound(format!(
                "No source claims identifier: {identifier}"
            )));
        }
        let mut last_err: Option<IiifError> = None;
        for source in candidates {
            match source.read_image(identifier).await {
                Ok(bytes) => return Ok(bytes),
                Err(IiifError::NotFound(msg)) => {
                    debug!(identifier, msg, "source missed; trying next");
                    last_err = Some(IiifError::NotFound(msg));
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            IiifError::NotFound(format!("Image not found: {identifier}"))
        }))
    }

    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        for source in self.pick(identifier) {
            match source.resolve_path(identifier).await {
                Ok(path) => return Ok(path),
                Err(IiifError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(IiifError::NotFound(format!(
            "Image not found: {identifier}"
        )))
    }

    async fn last_modified(&self, identifier: &str) -> Result<SystemTime, IiifError> {
        for source in self.pick(identifier) {
            match source.last_modified(identifier).await {
                Ok(t) => return Ok(t),
                Err(IiifError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(IiifError::NotFound(format!(
            "Image not found: {identifier}"
        )))
    }

    fn access_zone(&self, identifier: &str) -> Option<String> {
        for source in self.pick(identifier) {
            if let Some(zone) = source.access_zone(identifier) {
                return Some(zone);
            }
        }
        None
    }

    async fn read_sidecar(&self, identifier: &str) -> Option<Vec<u8>> {
        for source in self.pick(identifier) {
            if let Some(bytes) = source.read_sidecar(identifier).await {
                return Some(bytes);
            }
        }
        None
    }

    fn claims(&self, identifier: &str) -> bool {
        // The router itself claims iff at least one source does.
        self.sources.iter().any(|s| s.claims(identifier))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Minimal fake source that returns canned answers per identifier.
    struct FakeSource {
        label: String,
        files: Vec<(String, Vec<u8>)>,
        zone: Option<String>,
        prefix_filter: Option<String>,
        /// Surface a hard error for these identifiers (simulates "S3 down").
        broken: Vec<String>,
        head_calls: Mutex<usize>,
    }

    impl FakeSource {
        fn new(label: &str) -> Self {
            Self {
                label: label.into(),
                files: Vec::new(),
                zone: None,
                prefix_filter: None,
                broken: Vec::new(),
                head_calls: Mutex::new(0),
            }
        }
        fn with_file(mut self, id: &str, bytes: &[u8]) -> Self {
            self.files.push((id.into(), bytes.into()));
            self
        }
        fn with_zone(mut self, z: &str) -> Self {
            self.zone = Some(z.into());
            self
        }
        fn with_prefix(mut self, p: &str) -> Self {
            self.prefix_filter = Some(p.into());
            self
        }
        fn broken_for(mut self, id: &str) -> Self {
            self.broken.push(id.into());
            self
        }
        fn head_calls(&self) -> usize {
            *self.head_calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl ImageStorage for FakeSource {
        async fn exists(&self, id: &str) -> Result<bool, IiifError> {
            *self.head_calls.lock().unwrap() += 1;
            if self.broken.iter().any(|b| b == id) {
                return Err(IiifError::Storage(format!("{} broke", self.label)));
            }
            Ok(self.files.iter().any(|(i, _)| i == id))
        }
        async fn read_image(&self, id: &str) -> Result<Vec<u8>, IiifError> {
            if self.broken.iter().any(|b| b == id) {
                return Err(IiifError::Storage(format!("{} broke", self.label)));
            }
            self.files
                .iter()
                .find(|(i, _)| i == id)
                .map(|(_, b)| b.clone())
                .ok_or_else(|| IiifError::NotFound(format!("not in {}: {id}", self.label)))
        }
        async fn resolve_path(&self, id: &str) -> Result<PathBuf, IiifError> {
            if self.files.iter().any(|(i, _)| i == id) {
                Ok(PathBuf::from(format!("{}/{id}", self.label)))
            } else {
                Err(IiifError::NotFound(id.into()))
            }
        }
        async fn last_modified(&self, _id: &str) -> Result<SystemTime, IiifError> {
            Err(IiifError::NotFound("not used in routed tests".into()))
        }
        fn access_zone(&self, id: &str) -> Option<String> {
            if !self.claims(id) {
                return None;
            }
            self.zone.clone()
        }
        fn claims(&self, id: &str) -> bool {
            match &self.prefix_filter {
                Some(p) => id.starts_with(p),
                None => true,
            }
        }
    }

    #[tokio::test]
    async fn first_source_with_file_wins() {
        let s1 = FakeSource::new("a").with_file("foo", b"from-a");
        let s2 = FakeSource::new("b").with_file("foo", b"from-b");
        let routed = RoutedStorage::new(vec![Arc::new(s1), Arc::new(s2)]);
        assert_eq!(routed.read_image("foo").await.unwrap(), b"from-a");
    }

    #[tokio::test]
    async fn falls_through_on_not_found() {
        let s1 = FakeSource::new("a"); // empty
        let s2 = FakeSource::new("b").with_file("foo", b"from-b");
        let routed = RoutedStorage::new(vec![Arc::new(s1), Arc::new(s2)]);
        assert_eq!(routed.read_image("foo").await.unwrap(), b"from-b");
    }

    #[tokio::test]
    async fn hard_error_short_circuits_router() {
        // Storage outage on s1 must NOT be silently masked by s2 having the file.
        let s1 = FakeSource::new("a").broken_for("foo");
        let s2 = FakeSource::new("b").with_file("foo", b"from-b");
        let routed = RoutedStorage::new(vec![Arc::new(s1), Arc::new(s2)]);
        let err = routed.read_image("foo").await.unwrap_err();
        assert!(matches!(err, IiifError::Storage(_)));
    }

    #[tokio::test]
    async fn prefix_filter_skips_unclaimed_sources() {
        // s1 has prefix_filter "books-" but doesn't have "photo-1" — must be skipped
        // entirely so its broken `exists` doesn't fire.
        let s1 = FakeSource::new("books").with_prefix("books-").broken_for("photo-1");
        let s2 = FakeSource::new("photos").with_file("photo-1", b"P");
        let s1_arc: Arc<dyn ImageStorage> = Arc::new(s1);
        let s2_arc: Arc<dyn ImageStorage> = Arc::new(s2);
        let routed = RoutedStorage::new(vec![Arc::clone(&s1_arc), Arc::clone(&s2_arc)]);
        // Reads photo-1 from s2 with no detour through s1.
        assert_eq!(routed.read_image("photo-1").await.unwrap(), b"P");
    }

    #[tokio::test]
    async fn access_zone_uses_first_claiming_source() {
        let s1 = FakeSource::new("public");
        let s2 = FakeSource::new("private")
            .with_prefix("rare-")
            .with_zone("restricted");
        let routed = RoutedStorage::new(vec![Arc::new(s1), Arc::new(s2)]);
        assert_eq!(routed.access_zone("rare-x"), Some("restricted".into()));
        // public-y matches s1's catch-all (no prefix); zone is None.
        assert_eq!(routed.access_zone("public-y"), None);
    }

    #[tokio::test]
    async fn prefix_filter_avoids_head_round_trip() {
        // s1 is a cloud source with prefix_filter "rare-"; identifier "photo-1"
        // has the wrong prefix, so its exists() must NOT be called.
        let s1 = Arc::new(FakeSource::new("books").with_prefix("rare-"));
        let s2 = Arc::new(FakeSource::new("photos").with_file("photo-1", b"P"));
        let routed = RoutedStorage::new(vec![
            Arc::clone(&s1) as Arc<dyn ImageStorage>,
            Arc::clone(&s2) as Arc<dyn ImageStorage>,
        ]);
        let _ = routed.exists("photo-1").await.unwrap();
        assert_eq!(s1.head_calls(), 0, "filtered source must not be queried");
    }
}
