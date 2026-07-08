#[cfg(target_arch = "wasm32")]
use super::state::SavedProgress;
use super::state::WordHuntState;

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "elyk.wordhunt.progress.v1";

#[cfg(target_arch = "wasm32")]
pub(super) fn load_saved_progress() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(STORAGE_KEY)
        .ok()
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub(super) fn load_saved_progress() -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
pub(super) fn save_progress(state: &WordHuntState) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    if let Ok(json) = serde_json::to_string(&SavedProgress::from(state)) {
        let _ = storage.set_item(STORAGE_KEY, &json);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn save_progress(_state: &WordHuntState) {}

#[cfg(target_arch = "wasm32")]
pub fn clear_progress() {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let _ = storage.remove_item(STORAGE_KEY);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_progress() {}
