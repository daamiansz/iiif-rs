pub mod filesystem;

use crate::error::IiifError;
use async_trait::async_trait;
use std::path::PathBuf;

/// Abstraction over image storage backends.
///
/// Implementations are async-friendly: I/O methods return futures so callers
/// can `.await` them directly without `spawn_blocking` boilerplate.
/// `containing_directory` stays sync because it's a cheap in-memory lookup
/// used inside hot middleware.
#[async_trait]
pub trait ImageStorage: Send + Sync {
    /// Check whether an image with the given identifier exists.
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError>;

    /// Read the raw bytes of an image.
    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError>;

    /// Resolve the filesystem path for an identifier.
    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError>;

    /// Get the last modification time of the image source file.
    async fn last_modified(&self, identifier: &str) -> Result<std::time::SystemTime, IiifError>;

    /// Return the subdirectory name the image resides in, or `None` if at root.
    /// Used for directory-based access control. In-memory lookup, stays sync.
    fn containing_directory(&self, identifier: &str) -> Option<String>;

    /// Read the sidecar metadata file for an image, if one exists. Default
    /// returns `None` for backends that don't carry sidecars.
    /// File convention: `<root_or_subdir>/<identifier>.toml`.
    async fn read_sidecar(&self, _identifier: &str) -> Option<Vec<u8>> {
        None
    }
}
