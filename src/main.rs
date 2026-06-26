mod app;
pub mod commands;
pub mod content;
pub mod fs;
pub mod game;
pub mod net;
pub mod python;
pub mod storage;
pub mod terminal;

use app::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| view! { <App /> });
}
