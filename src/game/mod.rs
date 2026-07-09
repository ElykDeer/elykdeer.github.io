pub mod core;
pub mod textropolis;
pub mod wordhunt;

use crate::terminal::GameLaunch;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
mod canvas;

#[cfg(target_arch = "wasm32")]
pub use canvas::{clear_high_score, BubblesGame};

pub use textropolis::TextropolisGame;
pub use wordhunt::WordHuntGame;

pub fn clear_wordhunt_progress() {
    wordhunt::clear_progress();
}

#[component]
pub fn GameLaunchView(launch: GameLaunch) -> impl IntoView {
    match launch {
        GameLaunch::Bubbles { hard, cheat } => {
            view! { <BubblesGame hard=hard cheat=cheat /> }.into_any()
        }
        GameLaunch::Textropolis => view! { <TextropolisGame /> }.into_any(),
        GameLaunch::WordHunt {
            reveal,
            board: Some(board),
        } => view! { <WordHuntGame reveal=reveal board=board /> }.into_any(),
        GameLaunch::WordHunt {
            reveal,
            board: None,
        } => view! { <WordHuntGame reveal=reveal /> }.into_any(),
    }
}

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
