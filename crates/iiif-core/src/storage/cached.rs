//! Disk-backed read-through cache around an `ImageStorage`.
//!
//! Used to wrap remote backends (HTTP, S3 with high-latency endpoints) so
//! repeated requests for the same source bytes don't re-fetch from the origin.
//! Cache files live under `<cache_dir>/source/<sha256-of-id>.bin`, sharing the
//! same directory the image-pipeline already uses for processed tiles.
//!
//! Invalidation is by-design simple: the cache key does NOT include
//! `last_modified`. Operators wanting to refresh delete the directory (or its
//! relevant entries). This costs one HEAD per request but lets users replace
//! a misbehaving image without restarting the server. v0.4.x may add a
//! TTL-based or ETag-based check if real workloads demand it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::error::IiifError;
use crate::storage::ImageStorage;

/// Wraps an inner storage with an on-disk byte cache for `read_image`.
pub struct CachedSourceStorage {
    inner: Arc<dyn ImageStorage>,
    cache_dir: PathBuf,
    /// Stable per-source label; folded into the cache key so two sources
    /// fronting the same identifier don't collide.
    label: String,
}

impl CachedSourceStorage {
    pub fn new(inner: Arc<dyn ImageStorage>, cache_dir: PathBuf, label: impl Into<String>) -> Self {
        Self {
            inner,
            cache_dir,
            label: label.into(),
        }
    }

    fn cache_path(&self, identifier: &str) -> PathBuf {
        let mut h = Sha256::new();
        h.update(self.label.as_bytes());
        h.update(b"|");
        h.update(identifier.as_bytes());
        let digest = h.finalize();
        let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        self.cache_dir.join("source").join(format!("{hex}.bin"))
    }
}

#[async_trait]
impl ImageStorage for CachedSourceStorage {
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError> {
        self.inner.exists(identifier).await
    }

    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError> {
        let path = self.cache_path(identifier);
        // 1) Disk hit — return immediately.
        if let Ok(bytes) = tokio::fs::read(&path).await {
            return Ok(bytes);
        }
        // 2) Miss — fetch from inner, then write through.
        let bytes = self.inner.read_image(identifier).await?;
        if let Some(parent) = path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                tracing::warn!(path = %parent.display(), error = %e, "source cache mkdir failed");
                return Ok(bytes);
            }
        }
        if let Err(e) = tokio::fs::write(&path, &bytes).await {
            tracing::warn!(path = %path.display(), error = %e, "source cache write failed");
        }
        Ok(bytes)
    }

    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        self.inner.resolve_path(identifier).await
    }

    async fn last_modified(&self, identifier: &str) -> Result<SystemTime, IiifError> {
        self.inner.last_modified(identifier).await
    }

    fn access_zone(&self, identifier: &str) -> Option<String> {
        self.inner.access_zone(identifier)
    }

    async fn read_sidecar(&self, identifier: &str) -> Option<Vec<u8>> {
        self.inner.read_sidecar(identifier).await
    }

    fn claims(&self, identifier: &str) -> bool {
        self.inner.claims(identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    /// Minimal fake whose read_image bumps a counter — lets us assert the cache
    /// actually short-circuits the inner call on a hit.
    struct CountingSource {
        bytes: Vec<u8>,
        reads: AtomicUsize,
    }

    #[async_trait]
    impl ImageStorage for CountingSource {
        async fn exists(&self, _id: &str) -> Result<bool, IiifError> {
            Ok(true)
        }
        async fn read_image(&self, _id: &str) -> Result<Vec<u8>, IiifError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.bytes.clone())
        }
        async fn resolve_path(&self, _id: &str) -> Result<PathBuf, IiifError> {
            Err(IiifError::NotFound("n/a".into()))
        }
        async fn last_modified(&self, _id: &str) -> Result<SystemTime, IiifError> {
            Ok(SystemTime::UNIX_EPOCH)
        }
        fn access_zone(&self, _id: &str) -> Option<String> {
            None
        }
    }

    #[tokio::test]
    async fn second_read_hits_disk_cache() {
        let tmp = TempDir::new().unwrap();
        let inner = Arc::new(CountingSource {
            bytes: b"image-bytes".to_vec(),
            reads: AtomicUsize::new(0),
        });
        let inner_handle: Arc<dyn ImageStorage> = Arc::clone(&inner) as _;
        let cached = CachedSourceStorage::new(inner_handle, tmp.path().to_path_buf(), "lbl");

        let a = cached.read_image("foo").await.unwrap();
        let b = cached.read_image("foo").await.unwrap();
        assert_eq!(a, b"image-bytes");
        assert_eq!(b, b"image-bytes");
        // Inner saw exactly one read; second came from disk.
        assert_eq!(inner.reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn different_labels_avoid_cache_collision() {
        let tmp = TempDir::new().unwrap();
        let s1 = Arc::new(CountingSource {
            bytes: b"from-1".to_vec(),
            reads: AtomicUsize::new(0),
        });
        let s2 = Arc::new(CountingSource {
            bytes: b"from-2".to_vec(),
            reads: AtomicUsize::new(0),
        });
        let s1_h: Arc<dyn ImageStorage> = Arc::clone(&s1) as _;
        let s2_h: Arc<dyn ImageStorage> = Arc::clone(&s2) as _;
        let c1 = CachedSourceStorage::new(s1_h, tmp.path().to_path_buf(), "label-A");
        let c2 = CachedSourceStorage::new(s2_h, tmp.path().to_path_buf(), "label-B");

        assert_eq!(c1.read_image("foo").await.unwrap(), b"from-1");
        assert_eq!(c2.read_image("foo").await.unwrap(), b"from-2");
    }
}
