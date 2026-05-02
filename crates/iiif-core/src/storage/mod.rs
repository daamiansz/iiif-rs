pub mod cached;
pub mod filesystem;
pub mod object_store_backend;
pub mod routed;

use crate::error::IiifError;
use async_trait::async_trait;
use std::path::PathBuf;

/// Abstraction over image storage backends.
///
/// Implementations are async-friendly: I/O methods return futures so callers
/// can `.await` them directly without `spawn_blocking` boilerplate.
/// `access_zone` stays sync because it's a cheap in-memory lookup used inside
/// hot middleware.
#[async_trait]
pub trait ImageStorage: Send + Sync {
    /// Check whether an image with the given identifier exists.
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError>;

    /// Read the raw bytes of an image.
    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError>;

    /// Resolve the filesystem path for an identifier (when the backend has one).
    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError>;

    /// Get the last modification time of the image source file/object.
    async fn last_modified(&self, identifier: &str) -> Result<std::time::SystemTime, IiifError>;

    /// Return the access zone an image belongs to, or `None` for the public
    /// (unprotected) zone. Used by the auth middleware to gate requests
    /// against `auth.protected_dirs`. The semantics are universal across
    /// backends:
    ///
    /// - **Filesystem** — zone = subdirectory name (e.g. `"restricted"` for
    ///   `images/restricted/foo.jpg`); `None` for files in the root.
    /// - **Object-store / cloud** — zone = config-driven `access_zone` field
    ///   declared on the source; same value for every identifier the source
    ///   serves (cloud has no directory hierarchy).
    /// - **Routed multi-source** — delegates to whichever underlying source
    ///   owns the identifier.
    fn access_zone(&self, identifier: &str) -> Option<String>;

    /// Read the sidecar metadata file for an image, if one exists. Default
    /// returns `None` for backends that don't carry sidecars.
    /// File convention: `<root_or_subdir>/<identifier>.toml`.
    async fn read_sidecar(&self, _identifier: &str) -> Option<Vec<u8>> {
        None
    }

    /// Quick check (no async I/O on the hot path) whether this backend could
    /// own `identifier`. Used by `RoutedStorage` to decide which source to
    /// try first / skip without paying a HEAD round-trip on every request.
    /// Default: catch-all (`true`).
    fn claims(&self, _identifier: &str) -> bool {
        true
    }
}
