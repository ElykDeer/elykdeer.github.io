use std::collections::BTreeMap;

use leptos::ev::{DragEvent, Event};
use leptos::html::Input;
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::HtmlInputElement;

use crate::storage::{export_site_backup_json, BACKUP_STORAGE_KEYS, CURRENT_BACKUP_VERSION};
#[cfg(target_arch = "wasm32")]
use crate::storage::{import_site_backup_json, save_profile};
use crate::terminal::TerminalContext;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykDownloadText)]
    fn download_text_file(filename: &str, content: &str, mime_type: &str);

    #[wasm_bindgen(js_name = elykDownloadPageSnapshot)]
    fn download_page_snapshot_file(filename: &str, backup_json: &str);

    #[wasm_bindgen(js_name = elykInputFileText)]
    fn input_file_text(input: &HtmlInputElement) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykDroppedFileText)]
    fn dropped_file_text(event: &web_sys::DragEvent) -> js_sys::Promise;
}

#[component]
pub fn SaveManagerPanel(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    on_system_error: Callback<String>,
) -> impl IntoView {
    let file_input_ref = NodeRef::<Input>::new();
    let message = RwSignal::new("Download or restore a site save.".to_string());
    let busy = RwSignal::new(false);

    let download_backup = move |_| match current_backup_json(ctx) {
        Ok(json) => {
            let filename = backup_filename();
            download_backup_file(&filename, &json);
            message.set(format!("Downloaded {filename}."));
        }
        Err(err) => message.set(format!("Backup failed: {err}")),
    };

    let download_page = move |_| match current_backup_json(ctx) {
        Ok(json) => {
            let filename = page_snapshot_filename();
            download_page_snapshot(&filename, &json);
            message.set(format!("Downloaded {filename}."));
        }
        Err(err) => message.set(format!("Page backup failed: {err}")),
    };

    let handle_file_change = move |_: Event| {
        let Some(input) = file_input_ref.get() else {
            return;
        };
        read_input_backup_file(input, set_ctx, message, busy, on_system_error);
    };

    let handle_drag_over = move |ev: DragEvent| {
        ev.prevent_default();
    };

    let handle_drop = move |ev: DragEvent| {
        ev.prevent_default();
        read_dropped_backup_file(ev, set_ctx, message, busy, on_system_error);
    };

    view! {
        <section class="save-manager" on:dragover=handle_drag_over on:drop=handle_drop>
            <div class="save-manager-title">"save"</div>
            <p>"Terminal and game state."</p>
            <div class="save-manager-actions">
                <button type="button" on:click=download_backup disabled=move || busy.get()>
                    "download save data"
                </button>
                <button type="button" on:click=download_page disabled=move || busy.get()>
                    "download page backup"
                </button>
                <label class="save-manager-upload">
                    <input
                        type="file"
                        accept="application/json,text/html,.json,.html,.htm"
                        node_ref=file_input_ref
                        on:change=handle_file_change
                        disabled=move || busy.get()
                    />
                    <span>"restore save"</span>
                </label>
            </div>
            <div class="save-manager-drop">"Drop save JSON or page backup here"</div>
            <div class="save-manager-message" role="status" aria-live="polite">
                {move || message.get()}
            </div>
        </section>
    }
}

fn backup_filename() -> String {
    format!(
        "elyk-dev-{}-save-v{}.json",
        env!("CARGO_PKG_VERSION"),
        CURRENT_BACKUP_VERSION
    )
}

fn page_snapshot_filename() -> String {
    format!(
        "elyk-dev-{}-page-save-v{}.html",
        env!("CARGO_PKG_VERSION"),
        CURRENT_BACKUP_VERSION
    )
}

fn current_backup_json(ctx: ReadSignal<TerminalContext>) -> Result<String, serde_json::Error> {
    let profile = ctx.with_untracked(|ctx| ctx.profile().clone());
    export_site_backup_json(&profile, read_backup_storage())
}

fn read_backup_storage() -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return values;
    };
    for key in BACKUP_STORAGE_KEYS {
        if let Ok(Some(value)) = storage.get_item(key) {
            values.insert(key.to_string(), value);
        }
    }
    values
}

#[cfg(target_arch = "wasm32")]
fn restore_backup_storage(values: BTreeMap<String, String>) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    for key in BACKUP_STORAGE_KEYS {
        let _ = storage.remove_item(key);
    }
    for (key, value) in values {
        if BACKUP_STORAGE_KEYS.contains(&key.as_str()) {
            let _ = storage.set_item(&key, &value);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn download_backup_file(filename: &str, json: &str) {
    download_text_file(filename, json, "application/json;charset=utf-8");
}

#[cfg(not(target_arch = "wasm32"))]
fn download_backup_file(_filename: &str, _json: &str) {}

#[cfg(target_arch = "wasm32")]
fn download_page_snapshot(filename: &str, json: &str) {
    download_page_snapshot_file(filename, json);
}

#[cfg(not(target_arch = "wasm32"))]
fn download_page_snapshot(_filename: &str, _json: &str) {}

fn read_input_backup_file(
    input: HtmlInputElement,
    set_ctx: WriteSignal<TerminalContext>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    on_system_error: Callback<String>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = input_file_text(&input);
        input.set_value("");
        import_backup_from_promise(promise, set_ctx, message, busy, on_system_error);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (input, set_ctx, on_system_error);
        busy.set(false);
        message.set("File import is available in the browser build.".to_string());
    }
}

fn read_dropped_backup_file(
    event: DragEvent,
    set_ctx: WriteSignal<TerminalContext>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    on_system_error: Callback<String>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = dropped_file_text(event.unchecked_ref());
        import_backup_from_promise(promise, set_ctx, message, busy, on_system_error);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (event, set_ctx, on_system_error);
        busy.set(false);
        message.set("Drop import is available in the browser build.".to_string());
    }
}

#[cfg(target_arch = "wasm32")]
fn import_backup_from_promise(
    promise: js_sys::Promise,
    set_ctx: WriteSignal<TerminalContext>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    on_system_error: Callback<String>,
) {
    busy.set(true);
    message.set("Reading backup...".to_string());
    spawn_local(async move {
        let text = match JsFuture::from(promise).await {
            Ok(value) => value.as_string(),
            Err(err) => {
                busy.set(false);
                message.set(format!("Could not read backup: {err:?}"));
                return;
            }
        };
        let Some(text) = text else {
            busy.set(false);
            message.set("No backup file selected.".to_string());
            return;
        };
        import_backup_text(text, set_ctx, message, busy, on_system_error).await;
    });
}

#[cfg(target_arch = "wasm32")]
async fn import_backup_text(
    text: String,
    set_ctx: WriteSignal<TerminalContext>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    on_system_error: Callback<String>,
) {
    let backup_text = backup_json_from_import_text(&text);
    let backup = match import_site_backup_json(backup_text) {
        Ok(backup) => backup,
        Err(err) => {
            busy.set(false);
            message.set(format!("Invalid backup: {err}"));
            return;
        }
    };

    let profile = backup.profile;
    let mut replace_error = None;
    set_ctx.update(|ctx| {
        if let Err(err) = ctx.replace_profile(profile.clone()) {
            replace_error = Some(err.to_string());
        }
    });
    if let Some(err) = replace_error {
        busy.set(false);
        message.set(format!("Could not apply backup: {err}"));
        return;
    }

    match save_profile(&profile).await {
        Ok(()) => {
            restore_backup_storage(backup.local_storage);
            busy.set(false);
            message.set(
                "Backup imported. Relaunch any open games to see restored game state.".to_string(),
            );
        }
        Err(err) => {
            busy.set(false);
            message.set(format!(
                "Backup imported in memory, but profile save failed: {err}"
            ));
            on_system_error.run(format!("Profile save failed: {err}"));
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn backup_json_from_import_text(input: &str) -> &str {
    const EMBEDDED_ID: &str = "elyk-embedded-site-backup";
    let Some(id_position) = input.find(EMBEDDED_ID) else {
        return input;
    };
    let Some(script_start) = input[..id_position].rfind("<script") else {
        return input;
    };
    let Some(json_start_offset) = input[script_start..].find('>') else {
        return input;
    };
    let json_start = script_start + json_start_offset + 1;
    let Some(json_end_offset) = input[json_start..].find("</script>") else {
        return input;
    };

    &input[json_start..json_start + json_end_offset]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_json_from_import_text_extracts_page_snapshot_payload() {
        let json = r#"{"version":1,"profile":{"version":1,"settings":{"theme":"default"}}}"#;
        let html = format!(
            r#"<!doctype html><html><head><script id="elyk-embedded-site-backup" type="application/json">{json}</script></head></html>"#
        );

        assert_eq!(backup_json_from_import_text(&html), json);
        assert_eq!(backup_json_from_import_text(json), json);
    }
}
