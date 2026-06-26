pub mod overlay;
pub mod path;
pub mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use overlay::UserOverlay;
pub use path::{normalize_absolute, normalize_path, PathError};
pub use tree::BundledTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    Path(PathError),
    NotFound(String),
    NotDirectory(String),
    IsDirectory(String),
    AlreadyExists(String),
    ParentMissing(String),
    CannotOverwriteDirectory(String),
    ReadOnly(String),
    CannotRemoveRoot,
    InvalidOverlayPath(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(err) => write!(f, "{err}"),
            Self::NotFound(path) => write!(f, "not found: {path}"),
            Self::NotDirectory(path) => write!(f, "not a directory: {path}"),
            Self::IsDirectory(path) => write!(f, "is a directory: {path}"),
            Self::AlreadyExists(path) => write!(f, "already exists: {path}"),
            Self::ParentMissing(path) => write!(f, "parent directory does not exist: {path}"),
            Self::CannotOverwriteDirectory(path) => write!(f, "cannot overwrite directory: {path}"),
            Self::ReadOnly(path) => write!(f, "read-only file: {path}"),
            Self::CannotRemoveRoot => write!(f, "cannot remove /"),
            Self::InvalidOverlayPath(path) => write!(f, "invalid overlay path: {path}"),
        }
    }
}

impl std::error::Error for FsError {}

impl From<PathError> for FsError {
    fn from(value: PathError) -> Self {
        Self::Path(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualFileSystem {
    pub bundled: BundledTree,
    pub overlay: UserOverlay,
}

impl VirtualFileSystem {
    pub fn new(bundled: BundledTree, overlay: UserOverlay) -> Result<Self, FsError> {
        overlay.validate_against_bundled(&bundled)?;
        Ok(Self { bundled, overlay })
    }

    pub fn with_empty_overlay(bundled: BundledTree) -> Self {
        Self {
            bundled,
            overlay: UserOverlay::new(),
        }
    }

    pub fn resolve_path(&self, cwd: &str, input: &str) -> Result<String, FsError> {
        Ok(path::normalize_path(cwd, input)?)
    }

    pub fn read_file(&self, cwd: &str, input: &str) -> Result<ResolvedFile, FsError> {
        let path = self.resolve_path(cwd, input)?;
        if self.has_dir(&path) {
            return Err(FsError::IsDirectory(path));
        }

        if let Some(content) = self.overlay.files.get(&path) {
            return Ok(ResolvedFile {
                path,
                content: content.clone(),
            });
        }

        if let Some(content) = self.bundled.read_file(&path) {
            return Ok(ResolvedFile {
                path,
                content: content.to_string(),
            });
        }

        Err(FsError::NotFound(path))
    }

    pub fn list_dir(&self, cwd: &str, input: Option<&str>) -> Result<Vec<ListEntry>, FsError> {
        let path = self.resolve_path(cwd, input.unwrap_or("."))?;
        if self.has_file(&path) {
            return Err(FsError::NotDirectory(path));
        }
        if !self.has_dir(&path) {
            return Err(FsError::NotFound(path));
        }

        let mut entries: BTreeMap<String, ListEntry> = BTreeMap::new();

        for (name, kind) in self.bundled.direct_entries(&path) {
            let entry_path = child_path(&path, &name);
            entries.insert(
                name.clone(),
                ListEntry {
                    name,
                    path: entry_path,
                    kind,
                },
            );
        }

        for dir in &self.overlay.directories {
            if let Some(name) = path::direct_child_name(dir, &path) {
                entries.insert(
                    name.clone(),
                    ListEntry {
                        name,
                        path: dir.clone(),
                        kind: EntryKind::Directory,
                    },
                );
            }
        }

        for file in self.overlay.files.keys() {
            if let Some(name) = path::direct_child_name(file, &path) {
                entries.insert(
                    name.clone(),
                    ListEntry {
                        name,
                        path: file.clone(),
                        kind: EntryKind::File,
                    },
                );
            }
        }

        Ok(entries.into_values().collect())
    }

    pub fn write_file(
        &mut self,
        cwd: &str,
        input: &str,
        content: impl Into<String>,
    ) -> Result<(), FsError> {
        let path = self.resolve_path(cwd, input)?;
        if path == "/" || self.has_dir(&path) {
            return Err(FsError::CannotOverwriteDirectory(path));
        }
        if self.bundled.has_file(&path) {
            return Err(FsError::ReadOnly(path));
        }

        let parent =
            path::parent_path(&path).ok_or_else(|| FsError::ParentMissing(path.clone()))?;
        if !self.has_dir(&parent) {
            return Err(FsError::ParentMissing(parent));
        }

        self.overlay.files.insert(path, content.into());
        Ok(())
    }

    pub fn mkdir(&mut self, cwd: &str, input: &str) -> Result<(), FsError> {
        let path = self.resolve_path(cwd, input)?;
        if path == "/" || self.has_path(&path) {
            return Err(FsError::AlreadyExists(path));
        }
        let parent =
            path::parent_path(&path).ok_or_else(|| FsError::ParentMissing(path.clone()))?;
        if !self.has_dir(&parent) {
            return Err(FsError::ParentMissing(parent));
        }
        self.overlay.directories.insert(path);
        Ok(())
    }

    /// Removes a user file, or a user directory and everything under it.
    /// Directories are only removed when `recursive` is set. Bundled files are
    /// read-only and cannot be removed.
    pub fn remove_path(&mut self, cwd: &str, input: &str, recursive: bool) -> Result<(), FsError> {
        let path = self.resolve_path(cwd, input)?;
        if path == "/" {
            return Err(FsError::CannotRemoveRoot);
        }

        if self.overlay.files.remove(&path).is_some() {
            return Ok(());
        }

        if self.has_dir(&path) && !recursive {
            return Err(FsError::IsDirectory(path));
        }

        if self.overlay.directories.contains(&path) {
            self.overlay
                .directories
                .retain(|dir| dir != &path && !path::is_under(dir, &path));
            self.overlay
                .files
                .retain(|file, _| !path::is_under(file, &path));
            return Ok(());
        }

        if self.bundled.has_path(&path) {
            return Err(FsError::ReadOnly(path));
        }

        Err(FsError::NotFound(path))
    }

    /// Files under terminal-owned Python mirror roots, as `(path, content)`.
    /// Other runtime paths are left alone.
    pub fn managed_files(&self) -> Vec<(String, String)> {
        let mut files: BTreeMap<String, String> = self
            .bundled
            .files()
            .map(|(path, content)| (path.clone(), content.clone()))
            .collect();
        for (path, content) in &self.overlay.files {
            files.insert(path.clone(), content.clone());
        }
        files.retain(|path, _| is_managed_python_path(path));
        files.into_iter().collect()
    }

    /// Directories under terminal-owned Python mirror roots, sorted shallow-first.
    /// Mirror roots themselves are implicit, so they are excluded.
    pub fn managed_dirs(&self) -> Vec<String> {
        let mut dirs: BTreeSet<String> = self.bundled.directories().cloned().collect();
        dirs.extend(self.overlay.directories.iter().cloned());
        dirs.retain(|path| is_managed_python_path(path));
        let mut dirs: Vec<String> = dirs.into_iter().collect();
        dirs.sort_by_key(|path| path.matches('/').count());
        dirs
    }

    /// Record a file the Python runtime wrote. Bundled paths are read-only, so a
    /// write over one is dropped (it lives only in the session's Python FS).
    pub fn apply_external_write(&mut self, path: &str, content: String) {
        if self.bundled.has_file(path) {
            return;
        }
        self.ensure_overlay_dir(path);
        self.overlay.files.insert(path.to_string(), content);
    }

    /// Record a directory the Python runtime created.
    pub fn apply_external_mkdir(&mut self, path: &str) {
        if path == "/" || self.bundled.has_dir(path) {
            return;
        }
        self.overlay.directories.insert(path.to_string());
    }

    /// Record that the Python runtime removed a path (and anything beneath it).
    pub fn apply_external_remove(&mut self, path: &str) {
        self.overlay.files.remove(path);
        self.overlay
            .files
            .retain(|file, _| !path::is_under(file, path));
        self.overlay
            .directories
            .retain(|dir| dir != path && !path::is_under(dir, path));
    }

    /// Add the overlay directory entries needed for `file`'s parent chain,
    /// skipping ancestors already provided by the bundled tree.
    fn ensure_overlay_dir(&mut self, file: &str) {
        let Some(parent) = path::parent_path(file) else {
            return;
        };
        let mut current = parent;
        while current != "/" && !self.bundled.has_dir(&current) {
            self.overlay.directories.insert(current.clone());
            match path::parent_path(&current) {
                Some(next) => current = next,
                None => break,
            }
        }
    }

    pub fn has_path(&self, path: &str) -> bool {
        self.has_file(path) || self.has_dir(path)
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.overlay.files.contains_key(path) || self.bundled.has_file(path)
    }

    pub fn has_dir(&self, path: &str) -> bool {
        if path == "/" {
            return true;
        }
        self.overlay.directories.contains(path) || self.bundled.has_dir(path)
    }
}

fn is_managed_python_path(path: &str) -> bool {
    ["/root", "/python"]
        .iter()
        .any(|root| path::is_under(path, root))
}

fn child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs() -> VirtualFileSystem {
        let bundled = BundledTree::from_files(
            &[("/manual.md", "base"), ("/docs/welcome.md", "hello")],
            &["/docs"],
        )
        .unwrap();
        VirtualFileSystem::with_empty_overlay(bundled)
    }

    #[test]
    fn reads_bundled_and_user_files() {
        let mut fs = fs();
        assert_eq!(fs.read_file("/", "/manual.md").unwrap().content, "base");
        fs.write_file("/", "/docs/local.md", "mine").unwrap();
        assert_eq!(fs.read_file("/", "/docs/local.md").unwrap().content, "mine");
    }

    #[test]
    fn bundled_files_are_read_only() {
        let mut fs = fs();
        assert!(matches!(
            fs.write_file("/", "/manual.md", "x"),
            Err(FsError::ReadOnly(_))
        ));
        assert!(matches!(
            fs.remove_path("/", "/manual.md", false),
            Err(FsError::ReadOnly(_))
        ));
        assert_eq!(fs.read_file("/", "/manual.md").unwrap().content, "base");
    }

    #[test]
    fn removes_user_file() {
        let mut fs = fs();
        fs.write_file("/", "/docs/local.md", "local").unwrap();
        fs.remove_path("/", "/docs/local.md", false).unwrap();
        assert!(fs.read_file("/", "/docs/local.md").is_err());
    }

    #[test]
    fn removes_user_directory_recursively() {
        let mut fs = fs();
        fs.mkdir("/", "/scratch").unwrap();
        fs.mkdir("/", "/scratch/sub").unwrap();
        fs.write_file("/", "/scratch/sub/note.md", "hi").unwrap();

        fs.remove_path("/", "/scratch", true).unwrap();

        assert!(!fs.has_dir("/scratch"));
        assert!(!fs.has_dir("/scratch/sub"));
        assert!(fs.read_file("/", "/scratch/sub/note.md").is_err());
    }

    #[test]
    fn managed_view_is_scoped_to_the_home_subtree() {
        let mut fs = fs();
        fs.mkdir("/", "/root").unwrap();
        fs.mkdir("/root", "/root/work").unwrap();
        fs.write_file("/root/work", "in.md", "synced").unwrap();
        fs.mkdir("/", "/python").unwrap();
        fs.mkdir("/python", "/python/site-packages").unwrap();
        fs.write_file("/python/site-packages", "pkg.py", "imported")
            .unwrap();
        // A path outside the home must not be mirrored into Python.
        fs.mkdir("/", "/scratch").unwrap();
        fs.write_file("/scratch", "out.md", "unsynced").unwrap();

        let files = fs.managed_files();
        assert!(files.iter().any(|(path, _)| path == "/root/work/in.md"));
        assert!(files
            .iter()
            .any(|(path, _)| path == "/python/site-packages/pkg.py"));
        assert!(files.iter().all(|(path, _)| path != "/scratch/out.md"));

        let dirs = fs.managed_dirs();
        assert!(dirs.contains(&"/root/work".to_string()));
        assert!(dirs.contains(&"/python/site-packages".to_string()));
        assert!(!dirs.contains(&"/root".to_string()));
        assert!(!dirs.contains(&"/python".to_string()));
        assert!(!dirs.contains(&"/scratch".to_string()));
    }

    #[test]
    fn external_write_creates_overlay_parents_but_not_bundled_ones() {
        let mut fs = fs();
        fs.apply_external_write("/docs/from_python.md", "generated".to_string());
        fs.apply_external_write("/made/up/deep.txt", "x".to_string());

        // Bundled /docs must not be duplicated into the overlay.
        assert!(!fs.overlay.directories.contains("/docs"));
        assert!(fs.overlay.directories.contains("/made"));
        assert!(fs.overlay.directories.contains("/made/up"));
        assert_eq!(
            fs.read_file("/", "/docs/from_python.md").unwrap().content,
            "generated"
        );
    }

    #[test]
    fn external_write_over_bundled_file_is_ignored() {
        let mut fs = fs();
        fs.apply_external_write("/manual.md", "tampered".to_string());
        assert_eq!(fs.read_file("/", "/manual.md").unwrap().content, "base");
        assert!(!fs.overlay.files.contains_key("/manual.md"));
    }

    #[test]
    fn external_remove_prunes_subtree() {
        let mut fs = fs();
        fs.apply_external_mkdir("/proj");
        fs.apply_external_mkdir("/proj/src");
        fs.apply_external_write("/proj/src/main.py", "code".to_string());

        fs.apply_external_remove("/proj");
        assert!(!fs.has_dir("/proj"));
        assert!(!fs.has_dir("/proj/src"));
        assert!(fs.read_file("/", "/proj/src/main.py").is_err());
    }

    #[test]
    fn refuses_to_remove_directory_without_recursive() {
        let mut fs = fs();
        fs.mkdir("/", "/scratch").unwrap();

        assert!(matches!(
            fs.remove_path("/", "/scratch", false),
            Err(FsError::IsDirectory(_))
        ));
        assert!(fs.has_dir("/scratch"));
    }
}
