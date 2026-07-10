pub mod bubbles;
#[cfg(debug_assertions)]
pub mod snek;
pub mod textropolis;
mod word_data;
pub mod wordhunt;

use crate::terminal::GameLaunch;
use leptos::prelude::*;

pub use bubbles::{clear_high_score, BubblesGame};
#[cfg(debug_assertions)]
pub use snek::SnekGame;
pub use textropolis::TextropolisGame;
pub use wordhunt::WordHuntGame;

pub fn clear_wordhunt_progress() {
    wordhunt::clear_progress();
}

#[cfg(debug_assertions)]
pub fn clear_snek_save() {
    snek::clear_save();
}

#[component]
pub fn GameLaunchView(launch: GameLaunch) -> impl IntoView {
    match launch {
        GameLaunch::Bubbles { hard, cheat } => {
            view! { <BubblesGame hard=hard cheat=cheat /> }.into_any()
        }
        #[cfg(debug_assertions)]
        GameLaunch::Snek { cheat } => view! { <SnekGame cheat=cheat /> }.into_any(),
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
