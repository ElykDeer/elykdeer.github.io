mod app;
pub mod commands;
pub mod content;
pub mod fs;
pub mod game;
pub mod nano_editor;
pub mod net;
pub mod python;
pub mod save_manager;
pub mod storage;
pub mod terminal;

use app::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    remove_boot_profile_overlay();
    leptos::mount::mount_to_body(|| view! { <App /> });
}

#[cfg(target_arch = "wasm32")]
fn remove_boot_profile_overlay() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    if let Some(element) = document.get_element_by_id("boot-profile-overlay") {
        element.remove();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_boot_profile_overlay() {}
