pub mod filesystem;

use crate::error::IiifError;
use std::path::PathBuf;

/// Abstraction over image storage backends.
///
/// Implementations must be thread-safe (`Send + Sync`) because storage
/// is shared across request handlers via `Arc<dyn ImageStorage>`.
///
/// Methods are synchronous by design — callers should wrap calls in
/// `tokio::task::spawn_blocking` to avoid blocking the async runtime.
pub trait ImageStorage: Send + Sync {
    /// Check whether an image with the given identifier exists.
    fn exists(&self, identifier: &str) -> Result<bool, IiifError>;

    /// Read the raw bytes of an image.
    fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError>;

    /// Resolve the filesystem path for an identifier.
    fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError>;

    /// Get the last modification time of the image source file.
    fn last_modified(&self, identifier: &str) -> Result<std::time::SystemTime, IiifError>;

    /// Return the subdirectory name the image resides in, or `None` if at root.
    /// Used for directory-based access control.
    fn containing_directory(&self, identifier: &str) -> Option<String>;
}
