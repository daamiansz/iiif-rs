//! `ImageStorage` adapter on top of `object_store::ObjectStore`.
//!
//! Backs S3 / Azure Blob / GCS / HTTP / local-filesystem (via object_store)
//! through a single `ImageStorage` shape. Identifiers are percent-encoded
//! before being concatenated with `prefix` so IIIF identifiers containing
//! `/` (`ark:/12025/...`) survive as a single object key.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use futures::TryStreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::gcp::GoogleCloudStorageBuilder;
use object_store::http::HttpBuilder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as OsPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

use crate::config::StorageSourceConfig;
use crate::error::IiifError;
use crate::storage::ImageStorage;

/// Characters that must be encoded for safe transport into an object store
/// path segment. We deliberately do NOT escape `/` — `object_store::Path` uses
/// it as the segment separator and S3/Azure/GCS treat segments as opaque keys
/// joined by `/`, so an IIIF id like `ark:/12025/654` lands as the natural
/// key `prefix/ark:/12025/654.jpg` (a hierarchical layout most users already
/// have). Control chars and a small set of meta-characters are still encoded.
const IDENTIFIER_ENCODE: &AsciiSet = &CONTROLS.add(b' ').add(b'#').add(b'?');

/// Object-store backed source.
pub struct ObjectStoreBackend {
    store: Arc<dyn ObjectStore>,
    /// Prefix appended to every key (e.g. `images/` or `iiif/photos/`).
    /// Must end with a slash if non-empty so we don't accidentally smash names.
    prefix: String,
    /// IIIF Image API extensions tried in order when the identifier doesn't
    /// already carry one. Matches FilesystemStorage's behaviour.
    extensions: Vec<String>,
    /// Access zone reported by `access_zone()`. Same value for every id this
    /// source serves — cloud has no directory structure to introspect.
    access_zone: Option<String>,
    /// Optional substring that the post-decoded identifier must start with for
    /// this source to handle it. Empty = catch-all. Used by RoutedStorage to
    /// avoid HEAD round-trips on every request.
    prefix_filter: String,
    /// Human-readable label for logs.
    label: String,
}

impl ObjectStoreBackend {
    pub fn new(
        store: Arc<dyn ObjectStore>,
        prefix: impl Into<String>,
        access_zone: Option<String>,
        prefix_filter: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let mut p = prefix.into();
        if !p.is_empty() && !p.ends_with('/') {
            p.push('/');
        }
        Self {
            store,
            prefix: p,
            extensions: vec![
                "jpg".into(),
                "jpeg".into(),
                "png".into(),
                "tif".into(),
                "tiff".into(),
                "gif".into(),
                "webp".into(),
                "jp2".into(),
            ],
            access_zone,
            prefix_filter: prefix_filter.into(),
            label: label.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn prefix_filter(&self) -> &str {
        &self.prefix_filter
    }

    /// Returns true when this source claims `identifier` based on its
    /// configured `prefix_filter`. Empty filter = catch-all (always true).
    /// Used by the router to skip HEAD round-trips for sources that clearly
    /// can't own this id.
    pub fn claims(&self, identifier: &str) -> bool {
        self.prefix_filter.is_empty() || identifier.starts_with(&self.prefix_filter)
    }

    /// Encode an identifier so it survives as a single object_store path
    /// segment. `/` and other reserved chars become percent-escaped; the
    /// caller's IIIF URL is unchanged because percent-decoding happens
    /// upstream in `ImageIdentifier`.
    fn encoded(&self, identifier: &str) -> String {
        utf8_percent_encode(identifier, IDENTIFIER_ENCODE).to_string()
    }

    fn key(&self, identifier: &str, ext: &str) -> OsPath {
        let raw = if ext.is_empty() {
            format!("{}{}", self.prefix, self.encoded(identifier))
        } else {
            format!("{}{}.{}", self.prefix, self.encoded(identifier), ext)
        };
        OsPath::from(raw)
    }

    /// Try the bare identifier first (when it already carries an extension),
    /// then each known extension. Returns the first key that exists with its
    /// HEAD metadata.
    async fn locate(
        &self,
        identifier: &str,
    ) -> Result<(OsPath, object_store::ObjectMeta), IiifError> {
        // 1) Bare path (caller may have included extension already).
        let bare = self.key(identifier, "");
        match self.store.head(&bare).await {
            Ok(meta) => return Ok((bare, meta)),
            Err(object_store::Error::NotFound { .. }) => {}
            Err(e) => return Err(map_store_error(e, &self.label)),
        }
        // 2) Each known extension.
        for ext in &self.extensions {
            let key = self.key(identifier, ext);
            match self.store.head(&key).await {
                Ok(meta) => return Ok((key, meta)),
                Err(object_store::Error::NotFound { .. }) => continue,
                Err(e) => return Err(map_store_error(e, &self.label)),
            }
        }
        Err(IiifError::NotFound(format!(
            "Image not found: {identifier}"
        )))
    }
}

#[async_trait]
impl ImageStorage for ObjectStoreBackend {
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError> {
        if !self.claims(identifier) {
            return Ok(false);
        }
        match self.locate(identifier).await {
            Ok(_) => Ok(true),
            Err(IiifError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError> {
        if !self.claims(identifier) {
            return Err(IiifError::NotFound(format!(
                "Identifier not handled by source `{}`: {identifier}",
                self.label
            )));
        }
        let (key, _meta) = self.locate(identifier).await?;
        let result = self
            .store
            .get(&key)
            .await
            .map_err(|e| map_store_error(e, &self.label))?;
        let bytes = result
            .into_stream()
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| map_store_error(e, &self.label))?;
        Ok(bytes.into_iter().flatten().collect())
    }

    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        // Cloud objects have no on-disk path. Surface the object-store key as
        // a virtual `PathBuf` for callers that only want a stable handle.
        if !self.claims(identifier) {
            return Err(IiifError::NotFound(format!(
                "Identifier not handled by source `{}`: {identifier}",
                self.label
            )));
        }
        let (key, _meta) = self.locate(identifier).await?;
        Ok(PathBuf::from(key.to_string()))
    }

    async fn last_modified(&self, identifier: &str) -> Result<SystemTime, IiifError> {
        if !self.claims(identifier) {
            return Err(IiifError::NotFound(format!(
                "Identifier not handled by source `{}`: {identifier}",
                self.label
            )));
        }
        let (_key, meta) = self.locate(identifier).await?;
        Ok(meta.last_modified.into())
    }

    fn access_zone(&self, identifier: &str) -> Option<String> {
        if !ImageStorage::claims(self, identifier) {
            return None;
        }
        self.access_zone.clone()
    }

    fn claims(&self, identifier: &str) -> bool {
        // Cheap, in-memory: just the configured prefix_filter test. Real
        // existence is verified later by `read_image`/`exists` over the
        // network — that's the whole point of having a filter.
        self.prefix_filter.is_empty() || identifier.starts_with(&self.prefix_filter)
    }

    async fn read_sidecar(&self, identifier: &str) -> Option<Vec<u8>> {
        if !self.claims(identifier) {
            return None;
        }
        let key = self.key(identifier, "toml");
        match self.store.get(&key).await {
            Ok(result) => match result.into_stream().try_collect::<Vec<_>>().await {
                Ok(chunks) => Some(chunks.into_iter().flatten().collect()),
                Err(_) => None,
            },
            Err(_) => None,
        }
    }
}

/// Map an `object_store::Error` to our `IiifError`. `NotFound` is the only
/// "soft" outcome the router can recover from; everything else is hard so it
/// short-circuits the search rather than silently masking infrastructure issues.
fn map_store_error(e: object_store::Error, label: &str) -> IiifError {
    match e {
        object_store::Error::NotFound { path, .. } => {
            IiifError::NotFound(format!("Object `{path}` not found in source `{label}`"))
        }
        other => IiifError::Storage(format!("Object store `{label}` error: {other}")),
    }
}

/// Helper: write bytes to an object-store path. Used by sidecar/test fixtures.
#[allow(dead_code)]
pub async fn put_bytes(
    store: &dyn ObjectStore,
    path: &str,
    bytes: Vec<u8>,
) -> Result<(), IiifError> {
    let p = OsPath::from(path);
    store
        .put(&p, PutPayload::from(bytes))
        .await
        .map(|_| ())
        .map_err(|e| map_store_error(e, "memory"))
}

/// Build an `ObjectStoreBackend` from a `StorageSourceConfig`. Returns
/// `IiifError::BadRequest` for unknown `kind` or missing required fields,
/// `IiifError::Storage` if the backend constructor itself fails.
pub fn build_source(cfg: &StorageSourceConfig) -> Result<ObjectStoreBackend, IiifError> {
    let label = if cfg.label.is_empty() {
        cfg.kind.clone()
    } else {
        cfg.label.clone()
    };
    let access_zone = if cfg.access_zone.is_empty() {
        None
    } else {
        Some(cfg.access_zone.clone())
    };

    let store: Arc<dyn ObjectStore> = match cfg.kind.as_str() {
        "s3" => {
            if cfg.bucket.is_empty() {
                return Err(IiifError::BadRequest(format!(
                    "[storage.sources] kind=s3 requires `bucket` (label `{label}`)"
                )));
            }
            let mut builder = AmazonS3Builder::from_env().with_bucket_name(&cfg.bucket);
            if !cfg.region.is_empty() {
                builder = builder.with_region(&cfg.region);
            }
            if !cfg.url.is_empty() {
                builder = builder.with_endpoint(&cfg.url).with_allow_http(true);
            }
            Arc::new(
                builder
                    .build()
                    .map_err(|e| IiifError::Storage(format!("S3 backend `{label}`: {e}")))?,
            )
        }
        "azure" => {
            if cfg.account.is_empty() || cfg.container.is_empty() {
                return Err(IiifError::BadRequest(format!(
                    "[storage.sources] kind=azure requires `account` and `container` (label `{label}`)"
                )));
            }
            Arc::new(
                MicrosoftAzureBuilder::from_env()
                    .with_account(&cfg.account)
                    .with_container_name(&cfg.container)
                    .build()
                    .map_err(|e| IiifError::Storage(format!("Azure backend `{label}`: {e}")))?,
            )
        }
        "gcs" => {
            if cfg.bucket.is_empty() {
                return Err(IiifError::BadRequest(format!(
                    "[storage.sources] kind=gcs requires `bucket` (label `{label}`)"
                )));
            }
            Arc::new(
                GoogleCloudStorageBuilder::from_env()
                    .with_bucket_name(&cfg.bucket)
                    .build()
                    .map_err(|e| IiifError::Storage(format!("GCS backend `{label}`: {e}")))?,
            )
        }
        "http" => {
            if cfg.url.is_empty() {
                return Err(IiifError::BadRequest(format!(
                    "[storage.sources] kind=http requires `url` (label `{label}`)"
                )));
            }
            Arc::new(
                HttpBuilder::new()
                    .with_url(&cfg.url)
                    .build()
                    .map_err(|e| IiifError::Storage(format!("HTTP backend `{label}`: {e}")))?,
            )
        }
        "local" => {
            let path = if cfg.url.is_empty() {
                std::env::current_dir()
                    .map_err(|e| IiifError::Storage(format!("local backend cwd: {e}")))?
            } else {
                std::path::PathBuf::from(&cfg.url)
            };
            Arc::new(
                LocalFileSystem::new_with_prefix(&path)
                    .map_err(|e| IiifError::Storage(format!("local backend `{label}`: {e}")))?,
            )
        }
        other => {
            return Err(IiifError::BadRequest(format!(
                "[storage.sources] unknown kind `{other}` (label `{label}`)"
            )));
        }
    };

    Ok(ObjectStoreBackend::new(
        store,
        cfg.prefix.clone(),
        access_zone,
        cfg.prefix_filter.clone(),
        label,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    fn store() -> Arc<dyn ObjectStore> {
        Arc::new(InMemory::new()) as Arc<dyn ObjectStore>
    }

    #[tokio::test]
    async fn read_image_finds_object_with_extension() {
        let s = store();
        put_bytes(s.as_ref(), "imgs/photo.jpg", b"jpeg-bytes".to_vec())
            .await
            .unwrap();
        let backend = ObjectStoreBackend::new(
            Arc::clone(&s),
            "imgs/",
            None,
            "",
            "test",
        );
        assert!(backend.exists("photo").await.unwrap());
        let bytes = backend.read_image("photo").await.unwrap();
        assert_eq!(bytes, b"jpeg-bytes");
    }

    #[tokio::test]
    async fn missing_returns_not_found() {
        let backend = ObjectStoreBackend::new(store(), "imgs/", None, "", "test");
        assert!(!backend.exists("absent").await.unwrap());
        let err = backend.read_image("absent").await.unwrap_err();
        assert!(matches!(err, IiifError::NotFound(_)));
    }

    #[tokio::test]
    async fn slash_in_identifier_maps_to_hierarchical_key() {
        let s = store();
        // The caller's bucket layout: `imgs/ark:/12025/654.jpg`. Object stores
        // treat the segments as an opaque hierarchical key, so the IIIF id
        // `ark:/12025/654` lands naturally without custom encoding.
        put_bytes(s.as_ref(), "imgs/ark:/12025/654.jpg", b"x".to_vec())
            .await
            .unwrap();
        let backend = ObjectStoreBackend::new(Arc::clone(&s), "imgs/", None, "", "test");
        assert!(backend.exists("ark:/12025/654").await.unwrap());
        assert_eq!(backend.read_image("ark:/12025/654").await.unwrap(), b"x");
    }

    #[tokio::test]
    async fn access_zone_returns_configured_value_only_for_claimed_ids() {
        let backend = ObjectStoreBackend::new(
            store(),
            "imgs/",
            Some("restricted".to_string()),
            "private-",
            "test",
        );
        assert_eq!(
            backend.access_zone("private-x"),
            Some("restricted".to_string())
        );
        // Identifier outside the prefix filter is None — router will try the
        // next source.
        assert_eq!(backend.access_zone("public-y"), None);
    }

    #[tokio::test]
    async fn prefix_filter_short_circuits_unclaimed_ids() {
        let s = store();
        put_bytes(s.as_ref(), "imgs/foo.jpg", b"x".to_vec())
            .await
            .unwrap();
        let backend = ObjectStoreBackend::new(
            Arc::clone(&s),
            "imgs/",
            None,
            "books-",
            "books",
        );
        // Even though the object exists, claims() returns false → exists is
        // false without round-tripping to storage.
        assert!(!backend.exists("foo").await.unwrap());
        // Claimed prefix triggers the actual lookup.
        put_bytes(s.as_ref(), "imgs/books-bar.jpg", b"y".to_vec())
            .await
            .unwrap();
        assert!(backend.exists("books-bar").await.unwrap());
    }

    #[tokio::test]
    async fn read_sidecar_returns_none_when_absent() {
        let backend = ObjectStoreBackend::new(store(), "imgs/", None, "", "test");
        assert!(backend.read_sidecar("anything").await.is_none());
    }

    #[tokio::test]
    async fn read_sidecar_finds_toml() {
        let s = store();
        put_bytes(s.as_ref(), "imgs/photo.toml", b"label = \"X\"".to_vec())
            .await
            .unwrap();
        let backend = ObjectStoreBackend::new(Arc::clone(&s), "imgs/", None, "", "test");
        let body = backend.read_sidecar("photo").await.unwrap();
        assert_eq!(body, b"label = \"X\"");
    }
}
