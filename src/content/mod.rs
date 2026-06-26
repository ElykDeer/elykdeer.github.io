use crate::fs::BundledTree;

/// Read-only files shipped with the app. The filesystem seeds itself from this
/// list on first load; user edits and new files live in the overlay on top of
/// it. `/root` is the user's home (default working dir); `/python` holds
/// pip-installed packages under `/python/site-packages`. Both are mirrored into
/// the Python runtime.
const BUNDLED_FILES: &[(&str, &str)] = &[];
const BUNDLED_DIRECTORIES: &[&str] = &["/root", "/python"];

pub fn bundled_tree() -> BundledTree {
    BundledTree::from_files(BUNDLED_FILES, BUNDLED_DIRECTORIES)
        .expect("bundled content paths are valid")
}
