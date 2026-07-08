use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::content;
use crate::fs::UserOverlay;

pub const PROFILE_STORAGE_KEY: &str = "elyk.profile.v1";
pub const CURRENT_PROFILE_VERSION: u32 = 1;
pub const CURRENT_BACKUP_VERSION: u32 = 1;
pub const BACKUP_STORAGE_KEYS: [&str; 6] = [
    "elyk.bubbles.high-score",
    "elyk.bubbles.save.normal",
    "elyk.bubbles.save.hard",
    "elyk.bubbles.progress.v1",
    "elyk.textropolis.progress.v1",
    "elyk.wordhunt.progress.v1",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileV1 {
    pub version: u32,
    #[serde(default)]
    pub fs_overlay: UserOverlay,
    #[serde(default)]
    pub settings: ProfileSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteBackupV1 {
    pub version: u32,
    pub profile: ProfileV1,
    #[serde(default)]
    pub local_storage: BTreeMap<String, String>,
}

impl Default for ProfileV1 {
    fn default() -> Self {
        Self {
            version: CURRENT_PROFILE_VERSION,
            fs_overlay: UserOverlay::new(),
            settings: ProfileSettings::default(),
        }
    }
}

impl ProfileV1 {
    pub fn validate(&self) -> Result<(), ProfileImportError> {
        if self.version != CURRENT_PROFILE_VERSION {
            return Err(ProfileImportError::UnsupportedVersion(self.version));
        }
        if self.settings.theme.trim().is_empty() {
            return Err(ProfileImportError::InvalidShape(
                "settings.theme cannot be empty".to_string(),
            ));
        }
        let bundled = content::bundled_tree();
        self.fs_overlay
            .validate_against_bundled(&bundled)
            .map_err(|err| ProfileImportError::InvalidShape(err.to_string()))?;
        Ok(())
    }

    pub fn with_overlay(fs_overlay: UserOverlay) -> Self {
        Self {
            fs_overlay,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSettings {
    /// Session-only UI state: the overlay always shows on load and `toggle`
    /// only hides it for the current session, so this is never persisted.
    #[serde(skip, default = "default_overlay_visible")]
    pub overlay_visible: bool,
    pub theme: String,
}

fn default_overlay_visible() -> bool {
    true
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            overlay_visible: default_overlay_visible(),
            theme: "default".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ProfileImportError {
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    InvalidShape(String),
}

impl fmt::Display for ProfileImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "invalid profile JSON: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported profile version: {version}")
            }
            Self::InvalidShape(message) => write!(f, "invalid profile shape: {message}"),
        }
    }
}

impl std::error::Error for ProfileImportError {}

pub fn export_profile_json(profile: &ProfileV1) -> Result<String, serde_json::Error> {
    serde_json::to_string(profile)
}

pub fn import_profile_json(input: &str) -> Result<ProfileV1, ProfileImportError> {
    let value: serde_json::Value = serde_json::from_str(input).map_err(ProfileImportError::Json)?;
    if value.as_object().is_some_and(|object| {
        object.contains_key("profile") || object.contains_key("local_storage")
    }) {
        return Err(ProfileImportError::InvalidShape(
            "site backups must be imported from the save box; run import with no arguments"
                .to_string(),
        ));
    }
    let profile: ProfileV1 = serde_json::from_value(value).map_err(ProfileImportError::Json)?;
    profile.validate()?;
    Ok(profile)
}

pub fn export_site_backup_json(
    profile: &ProfileV1,
    local_storage: BTreeMap<String, String>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&SiteBackupV1 {
        version: CURRENT_BACKUP_VERSION,
        profile: profile.clone(),
        local_storage: local_storage
            .into_iter()
            .filter(|(key, _)| BACKUP_STORAGE_KEYS.contains(&key.as_str()))
            .collect(),
    })
}

pub fn import_site_backup_json(input: &str) -> Result<SiteBackupV1, ProfileImportError> {
    let backup: SiteBackupV1 = serde_json::from_str(input).map_err(ProfileImportError::Json)?;
    if backup.version != CURRENT_BACKUP_VERSION {
        return Err(ProfileImportError::UnsupportedVersion(backup.version));
    }
    backup.profile.validate()?;
    for key in backup.local_storage.keys() {
        if !BACKUP_STORAGE_KEYS.contains(&key.as_str()) {
            return Err(ProfileImportError::InvalidShape(format!(
                "unsupported backup storage key: {key}"
            )));
        }
    }
    Ok(backup)
}

#[derive(Debug)]
pub enum ProfileStorageError {
    Import(ProfileImportError),
    Json(serde_json::Error),
    Storage(String),
}

impl fmt::Display for ProfileStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Import(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "could not serialize profile: {err}"),
            Self::Storage(err) => write!(f, "storage error: {err}"),
        }
    }
}

impl std::error::Error for ProfileStorageError {}

#[cfg(target_arch = "wasm32")]
mod backend {
    use super::{import_profile_json, ProfileStorageError, ProfileV1, PROFILE_STORAGE_KEY};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = elykStorageGet, catch)]
        async fn storage_get(key: &str) -> Result<JsValue, JsValue>;

        #[wasm_bindgen(js_name = elykStorageSet, catch)]
        async fn storage_set(key: &str, value: &str) -> Result<JsValue, JsValue>;
    }

    fn describe(err: JsValue) -> ProfileStorageError {
        ProfileStorageError::Storage(format!("{err:?}"))
    }

    pub async fn load_profile() -> Result<ProfileV1, ProfileStorageError> {
        let value = storage_get(PROFILE_STORAGE_KEY).await.map_err(describe)?;
        match value.as_string() {
            Some(raw) => import_profile_json(&raw).map_err(ProfileStorageError::Import),
            None => Ok(ProfileV1::default()),
        }
    }

    pub async fn save_profile(profile: &ProfileV1) -> Result<(), ProfileStorageError> {
        profile.validate().map_err(ProfileStorageError::Import)?;
        let json = serde_json::to_string(profile).map_err(ProfileStorageError::Json)?;
        storage_set(PROFILE_STORAGE_KEY, &json)
            .await
            .map(|_| ())
            .map_err(describe)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::{ProfileStorageError, ProfileV1};

    pub async fn load_profile() -> Result<ProfileV1, ProfileStorageError> {
        Ok(ProfileV1::default())
    }

    pub async fn save_profile(profile: &ProfileV1) -> Result<(), ProfileStorageError> {
        profile.validate().map_err(ProfileStorageError::Import)
    }
}

pub use backend::{load_profile, save_profile};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_import_without_returning_profile() {
        let err = import_profile_json(r#"{"version": 2}"#).unwrap_err();
        assert!(matches!(err, ProfileImportError::UnsupportedVersion(2)));
    }

    #[test]
    fn validates_import_overlay_against_bundled_tree() {
        // A path declared as both a directory and a file is contradictory.
        let err = import_profile_json(
            r#"{"version":1,"fs_overlay":{"directories":["/x"],"files":{"/x":"y"}},"settings":{"theme":"default"}}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ProfileImportError::InvalidShape(message) if message.contains("/x")));

        // A file whose parent directory does not exist is rejected.
        let err = import_profile_json(
            r#"{"version":1,"fs_overlay":{"files":{"/missing/child.md":"bad"}},"settings":{"theme":"default"}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(err, ProfileImportError::InvalidShape(message) if message.contains("/missing/child.md"))
        );
    }

    #[test]
    fn exports_compact_roundtrippable_profile_json() {
        let mut profile = ProfileV1::default();
        profile
            .fs_overlay
            .directories
            .insert("/scratch".to_string());
        profile
            .fs_overlay
            .files
            .insert("/scratch/local.md".to_string(), "hello".to_string());

        let exported = export_profile_json(&profile).unwrap();
        assert!(!exported.contains('\n'));

        let imported = import_profile_json(&exported).unwrap();
        assert_eq!(imported, profile);
    }

    #[test]
    fn site_backup_round_trips_profile_and_known_browser_storage() {
        let profile = ProfileV1::default();
        let storage = BTreeMap::from([
            (
                "elyk.textropolis.progress.v1".to_string(),
                r#"{"guessed":{}}"#.to_string(),
            ),
            ("unknown".to_string(), "ignored".to_string()),
        ]);

        let exported = export_site_backup_json(&profile, storage).unwrap();
        let imported = import_site_backup_json(&exported).unwrap();

        assert_eq!(imported.profile, profile);
        assert_eq!(imported.local_storage.len(), 1);
        assert!(imported
            .local_storage
            .contains_key("elyk.textropolis.progress.v1"));
    }

    #[test]
    fn profile_import_rejects_full_site_backup_json() {
        let exported = export_site_backup_json(&ProfileV1::default(), BTreeMap::new()).unwrap();

        let err = import_profile_json(&exported).unwrap_err();

        assert!(
            matches!(err, ProfileImportError::InvalidShape(message) if message.contains("save box"))
        );
    }

    #[test]
    fn site_backup_rejects_unknown_storage_keys_on_import() {
        let err = import_site_backup_json(
            r#"{"version":1,"profile":{"version":1,"settings":{"theme":"default"}},"local_storage":{"unknown":"bad"}}"#,
        )
        .unwrap_err();

        assert!(
            matches!(err, ProfileImportError::InvalidShape(message) if message.contains("unknown"))
        );
    }
}
