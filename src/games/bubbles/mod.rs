#[cfg(not(target_arch = "wasm32"))]
use leptos::prelude::*;

pub mod state;

#[cfg(target_arch = "wasm32")]
mod view;

#[cfg(target_arch = "wasm32")]
pub use view::{clear_high_score, BubblesGame};

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_high_score() {}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn BubblesGame(
    #[prop(default = false)] hard: bool,
    #[prop(default = false)] cheat: bool,
) -> impl IntoView {
    let _ = cheat;
    let mode = if hard { "hard mode" } else { "normal mode" };
    view! {
        <section class="terminal-game">
            <div class="terminal-game-header">
                <span>"bubbles"</span>
                <span>{mode}</span>
            </div>
            <div class="terminal-game-fallback">
                "The bubbles canvas is available in the browser build."
            </div>
        </section>
    }
}
