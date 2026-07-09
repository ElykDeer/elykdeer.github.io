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
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykRunPostInitialPreloads)]
    fn run_post_initial_preloads_js();
}

fn main() {
    console_error_panic_hook::set_once();
    set_stable_app_height();
    leptos::mount::mount_to_body(|| view! { <App /> });
    remove_boot_elements();
    run_post_initial_preloads();
}

#[cfg(target_arch = "wasm32")]
fn set_stable_app_height() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(height) = window
        .inner_height()
        .ok()
        .and_then(|height| height.as_f64())
    else {
        return;
    };
    let Some(root) = window
        .document()
        .and_then(|document| document.document_element())
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let _ = root
        .style()
        .set_property("--app-height", &format!("{height}px"));
}

#[cfg(not(target_arch = "wasm32"))]
fn set_stable_app_height() {}

#[cfg(target_arch = "wasm32")]
fn remove_boot_elements() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    for id in ["boot-terminal-shell", "boot-profile-overlay"] {
        if let Some(element) = document.get_element_by_id(id) {
            element.remove();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_boot_elements() {}

#[cfg(target_arch = "wasm32")]
fn run_post_initial_preloads() {
    run_post_initial_preloads_js();
}

#[cfg(not(target_arch = "wasm32"))]
fn run_post_initial_preloads() {}
