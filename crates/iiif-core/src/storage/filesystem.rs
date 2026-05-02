use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tracing::debug;

use crate::error::IiifError;
use crate::storage::ImageStorage;

/// File-system backed image storage.
///
/// Images are looked up by scanning `root_dir` and its immediate
/// subdirectories. The subdirectory name determines access level
/// (e.g., `images/restricted/` requires authorization).
pub struct FilesystemStorage {
    root_dir: PathBuf,
}

const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff", "gif", "webp", "jp2"];

impl FilesystemStorage {
    pub fn new(root_dir: impl AsRef<Path>) -> Result<Self, IiifError> {
        let root_dir = root_dir.as_ref().to_path_buf();

        if !root_dir.exists() {
            fs::create_dir_all(&root_dir).map_err(|e| {
                IiifError::Storage(format!(
                    "Failed to create storage directory {}: {e}",
                    root_dir.display()
                ))
            })?;
        }

        if !root_dir.is_dir() {
            return Err(IiifError::Storage(format!(
                "Storage path is not a directory: {}",
                root_dir.display()
            )));
        }

        debug!(path = %root_dir.display(), "Initialized filesystem storage");
        Ok(Self { root_dir })
    }

    /// Find an image file by identifier, searching root and subdirectories.
    fn find_image_file(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        // 1. Check root directory
        if let Some(path) = self.try_find_in_dir(&self.root_dir, identifier) {
            return Ok(path);
        }

        // 2. Check immediate subdirectories
        if let Ok(entries) = fs::read_dir(&self.root_dir) {
            for entry in entries.flatten() {
                let subdir = entry.path();
                if subdir.is_dir() {
                    if let Some(path) = self.try_find_in_dir(&subdir, identifier) {
                        return Ok(path);
                    }
                }
            }
        }

        Err(IiifError::NotFound(format!(
            "Image not found: {identifier}"
        )))
    }

    /// Try to find an image in a specific directory.
    fn try_find_in_dir(&self, dir: &Path, identifier: &str) -> Option<PathBuf> {
        // Check with full name (if identifier includes extension)
        let direct = dir.join(identifier);
        if direct.is_file() {
            return Some(direct);
        }

        // Check with supported extensions
        for ext in SUPPORTED_EXTENSIONS {
            let path = dir.join(format!("{identifier}.{ext}"));
            if path.is_file() {
                return Some(path);
            }
        }

        None
    }

    /// Determine which subdirectory (relative to root) contains the image.
    /// Returns `None` if the image is in the root directory.
    fn find_containing_subdir(&self, identifier: &str) -> Option<String> {
        // Check if it's in root — if so, no subdirectory
        if self.try_find_in_dir(&self.root_dir, identifier).is_some() {
            return None;
        }

        // Check subdirectories
        if let Ok(entries) = fs::read_dir(&self.root_dir) {
            for entry in entries.flatten() {
                let subdir = entry.path();
                if subdir.is_dir() && self.try_find_in_dir(&subdir, identifier).is_some() {
                    return subdir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string());
                }
            }
        }

        None
    }
}

#[async_trait]
impl ImageStorage for FilesystemStorage {
    async fn exists(&self, identifier: &str) -> Result<bool, IiifError> {
        match self.find_image_file(identifier) {
            Ok(_) => Ok(true),
            Err(IiifError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError> {
        let path = self.find_image_file(identifier)?;
        debug!(path = %path.display(), "Reading image from filesystem");
        tokio::fs::read(&path)
            .await
            .map_err(|e| IiifError::Storage(format!("Failed to read {}: {e}", path.display())))
    }

    async fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        self.find_image_file(identifier)
    }

    async fn last_modified(&self, identifier: &str) -> Result<std::time::SystemTime, IiifError> {
        let path = self.find_image_file(identifier)?;
        let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
            IiifError::Storage(format!(
                "Failed to read metadata for {}: {e}",
                path.display()
            ))
        })?;
        metadata
            .modified()
            .map_err(|e| IiifError::Storage(format!("Failed to get modification time: {e}")))
    }

    fn containing_directory(&self, identifier: &str) -> Option<String> {
        self.find_containing_subdir(identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn finds_image_in_root() {
        let dir = std::env::temp_dir().join("iiif_test_fs_root");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut f = fs::File::create(dir.join("sample.jpg")).unwrap();
        f.write_all(b"fake-jpeg").unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert!(storage.exists("sample").await.unwrap());
        assert_eq!(storage.containing_directory("sample"), None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn finds_image_in_subdirectory() {
        let dir = std::env::temp_dir().join("iiif_test_fs_subdir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("restricted")).unwrap();

        let mut f = fs::File::create(dir.join("restricted/secret.jpg")).unwrap();
        f.write_all(b"secret-jpeg").unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert!(storage.exists("secret").await.unwrap());
        assert_eq!(
            storage.containing_directory("secret"),
            Some("restricted".to_string())
        );

        let bytes = storage.read_image("secret").await.unwrap();
        assert_eq!(bytes, b"secret-jpeg");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn root_takes_precedence_over_subdir() {
        let dir = std::env::temp_dir().join("iiif_test_fs_priority");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("restricted")).unwrap();

        let mut f = fs::File::create(dir.join("photo.jpg")).unwrap();
        f.write_all(b"root-version").unwrap();
        let mut f = fs::File::create(dir.join("restricted/photo.jpg")).unwrap();
        f.write_all(b"restricted-version").unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert_eq!(storage.containing_directory("photo"), None);
        assert_eq!(storage.read_image("photo").await.unwrap(), b"root-version");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn returns_not_found_for_missing() {
        let dir = std::env::temp_dir().join("iiif_test_fs_missing2");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert!(!storage.exists("nonexistent").await.unwrap());

        let _ = fs::remove_dir_all(&dir);
    }
}
