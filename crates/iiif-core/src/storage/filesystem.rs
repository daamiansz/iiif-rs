use std::fs;
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::error::IiifError;
use crate::storage::ImageStorage;

/// File-system backed image storage.
///
/// Images are looked up by scanning `root_dir` for files whose stem
/// matches the requested identifier (any image extension is accepted).
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

    fn find_image_file(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        // First, check if identifier already includes an extension
        let direct_path = self.root_dir.join(identifier);
        if direct_path.is_file() {
            return Ok(direct_path);
        }

        // Otherwise, search for files with supported extensions
        for ext in SUPPORTED_EXTENSIONS {
            let path = self.root_dir.join(format!("{identifier}.{ext}"));
            if path.is_file() {
                return Ok(path);
            }
        }

        Err(IiifError::NotFound(format!(
            "Image not found: {identifier}"
        )))
    }
}

impl ImageStorage for FilesystemStorage {
    fn exists(&self, identifier: &str) -> Result<bool, IiifError> {
        match self.find_image_file(identifier) {
            Ok(_) => Ok(true),
            Err(IiifError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn read_image(&self, identifier: &str) -> Result<Vec<u8>, IiifError> {
        let path = self.find_image_file(identifier)?;
        debug!(path = %path.display(), "Reading image from filesystem");
        fs::read(&path)
            .map_err(|e| IiifError::Storage(format!("Failed to read {}: {e}", path.display())))
    }

    fn resolve_path(&self, identifier: &str) -> Result<PathBuf, IiifError> {
        self.find_image_file(identifier)
    }

    fn last_modified(&self, identifier: &str) -> Result<std::time::SystemTime, IiifError> {
        let path = self.find_image_file(identifier)?;
        let metadata = fs::metadata(&path).map_err(|e| {
            IiifError::Storage(format!(
                "Failed to read metadata for {}: {e}",
                path.display()
            ))
        })?;
        metadata
            .modified()
            .map_err(|e| IiifError::Storage(format!("Failed to get modification time: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_image_with_extension() {
        let dir = std::env::temp_dir().join("iiif_test_fs_ext");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut f = fs::File::create(dir.join("sample.jpg")).unwrap();
        f.write_all(b"fake-jpeg").unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert!(storage.exists("sample").unwrap());

        let bytes = storage.read_image("sample").unwrap();
        assert_eq!(bytes, b"fake-jpeg");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_not_found_for_missing() {
        let dir = std::env::temp_dir().join("iiif_test_fs_missing");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let storage = FilesystemStorage::new(&dir).unwrap();
        assert!(!storage.exists("nonexistent").unwrap());
        assert!(storage.read_image("nonexistent").is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
