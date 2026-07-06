use leptos::html::Textarea;
use leptos::prelude::*;

use crate::storage::ProfileV1;
use crate::terminal::TerminalContext;

#[component]
pub fn NanoEditorPanel(
    path: String,
    initial_content: String,
    set_ctx: WriteSignal<TerminalContext>,
    on_error: Callback<String>,
    on_profile_changed: Callback<ProfileV1>,
) -> impl IntoView {
    let content = RwSignal::new(initial_content);
    let closed_message = RwSignal::new(None::<String>);
    let editor_ref = NodeRef::<Textarea>::new();
    let save_path = path.clone();
    let save_label_path = path.clone();
    let cancel_label_path = path.clone();

    Effect::new(move |_| {
        if closed_message.get().is_none() {
            if let Some(editor) = editor_ref.get() {
                let _ = editor.focus();
            }
        }
    });

    let save = move |_| {
        let new_content = content.get();
        let mut saved = None;
        let mut write_error = None;

        set_ctx.update(
            |ctx| match ctx.vfs_mut().write_file("/", &save_path, new_content) {
                Ok(()) => {
                    ctx.sync_profile_from_runtime();
                    saved = Some(ctx.profile().clone());
                }
                Err(err) => write_error = Some(err.to_string()),
            },
        );

        match write_error {
            Some(err) => on_error.run(err),
            None => {
                closed_message.set(Some("saved".to_string()));
                if let Some(profile) = saved {
                    on_profile_changed.run(profile);
                }
            }
        }
    };

    let cancel = move |_| {
        closed_message.set(Some("canceled".to_string()));
    };

    view! {
        <section class="nano-panel">
            <div class="nano-panel-header">
                <span>{path.clone()}</span>
                <span class="nano-panel-state">{move || closed_message.get().unwrap_or_else(|| "editing".to_string())}</span>
            </div>
            <textarea
                aria-label={format!("Editing {}", path)}
                class={move || {
                    if closed_message.get().is_none() {
                        "nano-editor"
                    } else {
                        "nano-editor nano-editor-hidden"
                    }
                }}
                disabled={move || closed_message.get().is_some()}
                node_ref=editor_ref
                spellcheck="false"
                prop:value={move || content.get()}
                on:input={move |ev| {
                    content.set(event_target_value(&ev));
                }}
            />
            <div class={move || {
                if closed_message.get().is_none() {
                    "nano-actions"
                } else {
                    "nano-actions nano-actions-hidden"
                }
            }}>
                <button
                    type="button"
                    aria-label={format!("Save {}", save_label_path)}
                    disabled={move || closed_message.get().is_some()}
                    on:click=save
                >
                    "Save"
                </button>
                <button
                    type="button"
                    aria-label={format!("Cancel editing {}", cancel_label_path)}
                    disabled={move || closed_message.get().is_some()}
                    on:click=cancel
                >
                    "Cancel"
                </button>
            </div>
        </section>
    }
}
