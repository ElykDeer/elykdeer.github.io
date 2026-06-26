use std::collections::{BTreeMap, BTreeSet};

use super::{path, EntryKind, FsError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundledTree {
    files: BTreeMap<String, String>,
    directories: BTreeSet<String>,
}

impl BundledTree {
    pub fn from_files(files: &[(&str, &str)], explicit_dirs: &[&str]) -> Result<Self, FsError> {
        let mut tree = Self::default();
        tree.directories.insert("/".to_string());

        for dir in explicit_dirs {
            tree.insert_directory(dir)?;
        }

        for (path, content) in files {
            tree.insert_file(path, content)?;
        }

        Ok(tree)
    }

    fn insert_file(&mut self, raw_path: &str, content: &str) -> Result<(), FsError> {
        let path = path::normalize_absolute(raw_path)?;
        if path == "/" {
            return Err(FsError::CannotOverwriteDirectory(path));
        }
        if self.directories.contains(&path) {
            return Err(FsError::CannotOverwriteDirectory(path));
        }

        let parent =
            path::parent_path(&path).ok_or_else(|| FsError::ParentMissing(path.clone()))?;
        self.ensure_directory_chain(&parent)?;
        self.files.insert(path, content.to_string());
        Ok(())
    }

    fn insert_directory(&mut self, raw_path: &str) -> Result<(), FsError> {
        let path = path::normalize_absolute(raw_path)?;
        self.ensure_directory_chain(&path)
    }

    pub fn read_file(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    pub fn files(&self) -> impl Iterator<Item = (&String, &String)> {
        self.files.iter()
    }

    pub fn directories(&self) -> impl Iterator<Item = &String> {
        self.directories.iter()
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn has_dir(&self, path: &str) -> bool {
        self.directories.contains(path)
    }

    pub fn has_path(&self, path: &str) -> bool {
        self.has_file(path) || self.has_dir(path)
    }

    pub fn direct_entries(&self, dir: &str) -> BTreeMap<String, EntryKind> {
        let mut entries = BTreeMap::new();

        for path in &self.directories {
            if let Some(name) = path::direct_child_name(path, dir) {
                entries.insert(name, EntryKind::Directory);
            }
        }

        for path in self.files.keys() {
            if let Some(name) = path::direct_child_name(path, dir) {
                entries.insert(name, EntryKind::File);
            }
        }

        entries
    }

    fn ensure_directory_chain(&mut self, path: &str) -> Result<(), FsError> {
        let normalized = path::normalize_absolute(path)?;
        if self.files.contains_key(&normalized) {
            return Err(FsError::AlreadyExists(normalized));
        }

        let mut current = String::from("/");
        self.directories.insert(current.clone());

        for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
            current = if current == "/" {
                format!("/{segment}")
            } else {
                format!("{current}/{segment}")
            };

            if self.files.contains_key(&current) {
                return Err(FsError::AlreadyExists(current));
            }
            self.directories.insert(current.clone());
        }

        Ok(())
    }
}
