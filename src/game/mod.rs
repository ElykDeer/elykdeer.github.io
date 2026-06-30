pub mod core;
pub mod textropolis;

#[cfg(target_arch = "wasm32")]
mod canvas;

#[cfg(target_arch = "wasm32")]
pub use canvas::{clear_high_score, BubblesGame};

pub use textropolis::TextropolisGame;

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_high_score() {}

#[cfg(not(target_arch = "wasm32"))]
use leptos::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn BubblesGame(#[prop(default = false)] hard: bool) -> impl IntoView {
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
