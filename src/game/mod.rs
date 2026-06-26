pub mod core;

#[cfg(target_arch = "wasm32")]
mod canvas;

#[cfg(target_arch = "wasm32")]
pub use canvas::{clear_high_score, BubblesGame};

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_high_score() {}

#[cfg(not(target_arch = "wasm32"))]
use leptos::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn BubblesGame() -> impl IntoView {
    view! {
        <section class="terminal-game">
            <div class="terminal-game-header">
                <span>"bubbles"</span>
                <span>"WASM canvas renderer"</span>
            </div>
            <div class="terminal-game-fallback">
                "The bubbles canvas is available in the browser build."
            </div>
        </section>
    }
}
