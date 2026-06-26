use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{path, BundledTree, FsError};

/// User-created files and directories layered on top of the read-only bundled
/// tree. This is the part of the filesystem that persists in the profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOverlay {
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub directories: BTreeSet<String>,
}

impl UserOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate_paths(&self) -> Result<(), FsError> {
        for path in self.files.keys().chain(self.directories.iter()) {
            let normalized = path::normalize_absolute(path)?;
            if normalized != *path {
                return Err(FsError::InvalidOverlayPath(path.clone()));
            }
        }

        for file in self.files.keys() {
            if file == "/" || self.directories.contains(file) {
                return Err(FsError::InvalidOverlayPath(file.clone()));
            }
        }

        if self.directories.contains("/") {
            return Err(FsError::InvalidOverlayPath("/".to_string()));
        }

        Ok(())
    }

    pub fn validate_against_bundled(&self, bundled: &BundledTree) -> Result<(), FsError> {
        self.validate_paths()?;

        for file in self.files.keys() {
            // Bundled entries are read-only, so the overlay cannot redefine them.
            if bundled.has_path(file) {
                return Err(FsError::InvalidOverlayPath(file.clone()));
            }
            self.validate_parent(file, bundled)?;
        }

        for directory in &self.directories {
            if bundled.has_file(directory) {
                return Err(FsError::InvalidOverlayPath(directory.clone()));
            }
            self.validate_parent(directory, bundled)?;
        }

        Ok(())
    }

    fn validate_parent(&self, entry_path: &str, bundled: &BundledTree) -> Result<(), FsError> {
        let Some(parent) = path::parent_path(entry_path) else {
            return Err(FsError::InvalidOverlayPath(entry_path.to_string()));
        };

        if parent == "/" {
            return Ok(());
        }

        if self.directories.contains(&parent) || bundled.has_dir(&parent) {
            Ok(())
        } else {
            Err(FsError::InvalidOverlayPath(entry_path.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> BundledTree {
        BundledTree::from_files(&[("/docs/readme.md", "hi")], &["/docs"]).unwrap()
    }

    #[test]
    fn overlay_cannot_redefine_bundled_paths() {
        let mut file_over_bundled = UserOverlay::new();
        file_over_bundled
            .files
            .insert("/docs/readme.md".to_string(), "x".to_string());
        assert!(file_over_bundled
            .validate_against_bundled(&bundled())
            .is_err());

        let mut dir_over_file = UserOverlay::new();
        dir_over_file
            .directories
            .insert("/docs/readme.md".to_string());
        assert!(dir_over_file.validate_against_bundled(&bundled()).is_err());
    }

    #[test]
    fn user_files_can_live_in_bundled_directories() {
        let mut overlay = UserOverlay::new();
        overlay
            .files
            .insert("/docs/mine.md".to_string(), "x".to_string());
        assert!(overlay.validate_against_bundled(&bundled()).is_ok());
    }
}
