#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

use leptos::html::Section;
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::{spawn_local, JsFuture};

use super::data::Definition;
#[cfg(target_arch = "wasm32")]
use super::data::TextropolisData;
use super::state::{GuessResult, TextropolisState};
use super::storage::save_progress;

const SCROLL_DURATION_SECONDS: usize = 52;
const BANNER_DURATION_MS: u64 = SCROLL_DURATION_SECONDS as u64 * 1_000;
const BANNER_STAGGER_MS: u64 = 2_500;
const BANNER_LANE_REUSE_PERCENT: u64 = 55;
const BANNER_RANDOM_OFFSET_MS: u64 = 1_500;
const BANNER_LANES: usize = 6;
const TICKER_MIN_DURATION_SECONDS: usize = SCROLL_DURATION_SECONDS;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug)]
enum DictionaryLoad {
    Loading,
    Ready(TextropolisState),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FlyingDefinition {
    id: u64,
    lane: usize,
    started_ms: u64,
    duration_ms: u64,
    text: String,
}

fn feedback_text(result: &GuessResult) -> String {
    match result {
        GuessResult::Accepted { word, .. } => format!("Accepted {}.", word.to_ascii_uppercase()),
        GuessResult::AlreadyFound { .. } => String::new(),
        GuessResult::Empty => String::new(),
        GuessResult::TooShort => String::new(),
        GuessResult::NotFromCity => String::new(),
        GuessResult::Unknown => String::new(),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykLoadTextropolisDictionary)]
    fn load_textropolis_dictionary_js() -> js_sys::Promise;
}

#[cfg(target_arch = "wasm32")]
async fn load_textropolis_data() -> Result<Arc<TextropolisData>, String> {
    let value = JsFuture::from(load_textropolis_dictionary_js())
        .await
        .map_err(describe_js_error)?;
    let json = value
        .as_string()
        .ok_or_else(|| "dictionary loader returned a non-string value".to_string())?;
    TextropolisData::from_json(&json).map(Arc::new)
}

#[cfg(target_arch = "wasm32")]
fn describe_js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "JavaScript error".to_string())
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    0
}

#[cfg(target_arch = "wasm32")]
fn random_offset_ms() -> u64 {
    (js_sys::Math::random() * BANNER_RANDOM_OFFSET_MS as f64) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn random_offset_ms() -> u64 {
    BANNER_RANDOM_OFFSET_MS.saturating_sub(BANNER_RANDOM_OFFSET_MS)
}

fn queue_definitions(
    definitions: &RwSignal<Vec<FlyingDefinition>>,
    next_banner_id: &RwSignal<u64>,
    lane_available_ms: &RwSignal<Vec<u64>>,
    word: &str,
    entries: &[Definition],
) {
    let base_ms = now_ms();
    let mut next_id = next_banner_id.get_untracked();
    let mut lanes = lane_available_ms.get_untracked();
    if lanes.len() != BANNER_LANES {
        lanes = vec![base_ms; BANNER_LANES];
    }

    let mut cursor_ms = base_ms;
    let mut queued = Vec::new();
    for entry in entries.iter().take(BANNER_LANES) {
        let text = format!(
            "{} ({}): {}",
            word.to_ascii_uppercase(),
            entry.part_of_speech,
            entry.definition
        );
        let duration_ms = banner_duration_ms(&text);
        let (lane, available_ms) = lanes
            .iter()
            .enumerate()
            .min_by_key(|(_, available_ms)| **available_ms)
            .map(|(lane, available_ms)| (lane, *available_ms))
            .unwrap_or((0, base_ms));
        let started_ms = cursor_ms.max(available_ms).max(base_ms) + random_offset_ms();
        lanes[lane] = started_ms + duration_ms * BANNER_LANE_REUSE_PERCENT / 100;
        cursor_ms = started_ms + BANNER_STAGGER_MS;
        queued.push(FlyingDefinition {
            id: next_id,
            lane,
            started_ms,
            duration_ms,
            text,
        });
        next_id += 1;
    }

    next_banner_id.set(next_id);
    lane_available_ms.set(lanes);
    definitions.update(|definitions| {
        definitions.retain(|definition| base_ms < definition.started_ms + definition.duration_ms);
        definitions.extend(queued);
    });
}

fn ticker_duration(words: &[String]) -> usize {
    let total_chars = words.iter().map(String::len).sum::<usize>();
    TICKER_MIN_DURATION_SECONDS.max(28 + total_chars * 2 / 5)
}

fn banner_duration_ms(text: &str) -> u64 {
    let extra_ms = text.chars().count().saturating_sub(80).min(175) as u64 * 70;
    BANNER_DURATION_MS + extra_ms
}

#[component]
pub fn TextropolisGame() -> impl IntoView {
    let section_ref = NodeRef::<Section>::new();
    let load = RwSignal::new(DictionaryLoad::Loading);
    let feedback = RwSignal::new(String::new());
    let definitions = RwSignal::new(Vec::<FlyingDefinition>::new());
    let next_banner_id = RwSignal::new(0_u64);
    let lane_available_ms = RwSignal::new(vec![0; BANNER_LANES]);

    #[cfg(target_arch = "wasm32")]
    spawn_local(async move {
        match load_textropolis_data().await {
            Ok(data) => {
                let state = TextropolisState::new(Arc::clone(&data));
                load.set(DictionaryLoad::Ready(state));
            }
            Err(err) => load.set(DictionaryLoad::Failed(err)),
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    {
        load.set(DictionaryLoad::Failed(
            "Textropolis dictionary loading is only available in the browser build.".to_string(),
        ));
    }

    let submit = move || {
        let result = load.with(|load| match load {
            DictionaryLoad::Ready(state) => state.check_selected(),
            _ => GuessResult::Empty,
        });
        let definition_source = match &result {
            GuessResult::Accepted { word, definitions } => Some((word, definitions, true)),
            GuessResult::AlreadyFound { word, definitions } => Some((word, definitions, false)),
            _ => None,
        };
        if let Some((word, entries, should_record)) = definition_source {
            load.update(|load| {
                if let DictionaryLoad::Ready(state) = load {
                    if should_record {
                        state.record_accepted(word);
                        save_progress(state);
                    }
                    state.clear_selection();
                }
            });
            queue_definitions(
                &definitions,
                &next_banner_id,
                &lane_available_ms,
                word,
                entries,
            );
        }
        feedback.set(feedback_text(&result));
    };

    let focus_game = move || {
        if let Some(section) = section_ref.get_untracked() {
            let _ = section.focus();
        }
    };

    let pick_letter = move |index: usize| {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.pick_letter(index);
            }
        });
    };

    let backspace = move |_| {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.backspace();
            }
        });
    };

    let clear = move |_| {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.clear_selection();
            }
        });
    };

    let select_word = move |word: String| {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.select_word(&word);
            }
        });
    };

    let leave_city = move || {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.leave_city();
                save_progress(state);
            }
        });
        feedback.set(String::new());
        definitions.set(Vec::new());
        next_banner_id.set(0);
        lane_available_ms.set(vec![0; BANNER_LANES]);
    };

    let leave = move |_| leave_city();

    view! {
        <section
            class="terminal-game textropolis-game"
            node_ref=section_ref
            tabindex="0"
            on:mousedown=move |_| focus_game()
            on:keydown=move |ev| {
                let key = ev.key();
                match key.as_str() {
                    "Enter" => {
                        ev.prevent_default();
                        submit();
                    }
                    "Backspace" => {
                        ev.prevent_default();
                        load.update(|load| {
                            if let DictionaryLoad::Ready(state) = load {
                                state.backspace();
                            }
                        });
                    }
                    "Escape" => {
                        ev.prevent_default();
                        let should_leave = load.with_untracked(|load| {
                            matches!(
                                load,
                                DictionaryLoad::Ready(state) if state.selected_letters().is_empty()
                            )
                        });
                        if should_leave {
                            leave_city();
                        } else {
                            load.update(|load| {
                                if let DictionaryLoad::Ready(state) = load {
                                    state.clear_selection();
                                }
                            });
                        }
                    }
                    _ if key.len() == 1 => {
                        if let Some(ch) = key.chars().next().filter(|ch| ch.is_ascii_alphabetic()) {
                            ev.prevent_default();
                            load.update(|load| {
                                if let DictionaryLoad::Ready(state) = load {
                                    state.type_letter(ch);
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }
        >
            {move || match load.get() {
                DictionaryLoad::Loading => view! {
                    <div class="terminal-game-header">
                        <span>"textropolis"</span>
                        <span>"loading dictionary"</span>
                    </div>
                    <div class="textropolis-loading">"Loading dictionary..."</div>
                }.into_any(),
                DictionaryLoad::Failed(err) => view! {
                    <div class="terminal-game-header">
                        <span>"textropolis"</span>
                        <span>"dictionary error"</span>
                    </div>
                    <div class="textropolis-error">
                        {format!("Could not load textropolis-dictionary.json.gz: {err}")}
                    </div>
                }.into_any(),
                DictionaryLoad::Ready(state) => match state.current_city_index() {
                    None => view! {
                        <TextropolisCitySelect load=load />
                    }.into_any(),
                    Some(_) => view! {
                        <TextropolisCityScreen
                            load=load
                            feedback=feedback
                            definitions=definitions
                            submit=submit
                            pick_letter=pick_letter
                            select_word=select_word
                            backspace=backspace
                            clear=clear
                            leave=leave
                        />
                    }.into_any(),
                }
            }}
        </section>
    }
}

#[component]
fn TextropolisCitySelect(load: RwSignal<DictionaryLoad>) -> impl IntoView {
    view! {
        <div class="terminal-game-header">
            <span>"textropolis"</span>
            <span>{move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    format!(
                        "{}% overall - {} / {} total",
                        state.total_percent(),
                        state.total_found_count(),
                        state.total_word_count()
                    )
                }
                _ => String::new(),
            }}</span>
        </div>
        <div class="textropolis-city-list">
            {move || match load.get() {
                DictionaryLoad::Ready(state) => state
                    .data
                    .cities
                    .iter()
                    .enumerate()
                    .map(|(index, city)| {
                        let unlocked = state.is_unlocked(index);
                        let found = state.city_found_count(index);
                        let total = state.city_total_count(index);
                        let pct = state.city_percent(index);
                        let hint = state.unlock_hint(index);
                        let detail = if unlocked {
                            format!("{found} / {total}")
                        } else {
                            hint.clone()
                        };
                        view! {
                            <button
                                type="button"
                                class="textropolis-city-card"
                                class:locked=!unlocked
                                disabled=!unlocked
                                title=hint
                                style=format!("--progress:{}%;", pct)
                                on:click=move |_| {
                                    load.update(|load| {
                                        if let DictionaryLoad::Ready(state) = load {
                                            state.enter_city(index);
                                        }
                                    });
                                }
                            >
                                <span>{city.name}</span>
                                <span>{format!("{pct}%")}</span>
                                <small>{detail}</small>
                            </button>
                        }
                    })
                    .collect_view()
                    .into_any(),
                _ => ().into_any(),
            }}
        </div>
    }
}

#[component]
fn TextropolisCityScreen(
    load: RwSignal<DictionaryLoad>,
    feedback: RwSignal<String>,
    definitions: RwSignal<Vec<FlyingDefinition>>,
    submit: impl Fn() + Copy + Send + Sync + 'static,
    pick_letter: impl Fn(usize) + Copy + Send + Sync + 'static,
    select_word: impl Fn(String) + Copy + Send + Sync + 'static,
    backspace: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    clear: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    leave: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="textropolis-word-ticker" aria-live="polite">
            <div
                class="textropolis-ticker-track"
                style={move || match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let words = state.filtered_found_words();
                        format!("--ticker-duration:{}s;", ticker_duration(&words))
                    }
                    _ => String::new(),
                }}
            >
                {move || match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let words = state.filtered_found_words();
                        if words.is_empty() {
                            ().into_any()
                        } else {
                            words
                                .into_iter()
                                .map(|word| {
                                    let selected_word = word.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="textropolis-ticker-word"
                                            on:click=move |_| select_word(selected_word.clone())
                                        >
                                            {word}
                                        </button>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }
                    }
                    _ => ().into_any(),
                }}
            </div>
        </div>

        <div class="terminal-game-header">
            <span>{move || match load.get() {
                DictionaryLoad::Ready(state) => state.active_city_name().to_string(),
                _ => String::new(),
            }}</span>
            <span>{move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    let index = state.current_city_index().unwrap_or(0);
                    format!("{} / {}", state.city_found_count(index), state.city_total_count(index))
                }
                _ => String::new(),
            }}</span>
        </div>

        <div class="textropolis-definition-sky">
            <For
                each={move || definitions
                    .get()
                    .into_iter()
                    .filter(|definition| now_ms() < definition.started_ms + definition.duration_ms)
                    .collect::<Vec<_>>()}
                key=|definition| definition.id
                children={move |definition| {
                    let delay = definition.started_ms as i64 - now_ms() as i64;
                    view! {
                        <div
                            class="textropolis-banner definition"
                            style=format!(
                                "--lane:{}; --duration:{}ms; --delay:{}ms;",
                                definition.lane,
                                definition.duration_ms,
                                delay
                            )
                        >
                            <div class="textropolis-banner-flight">
                                <span class="textropolis-plane" aria-hidden="true"></span>
                                <span class="textropolis-rope" aria-hidden="true"></span>
                                <span class="textropolis-rope lower" aria-hidden="true"></span>
                                <span class="textropolis-ad">{definition.text}</span>
                            </div>
                        </div>
                    }
                }}
            />
        </div>

        <div class="textropolis-current-word">
            {move || match load.get() {
                DictionaryLoad::Ready(state) => state.selected_word(),
                _ => String::new(),
            }}
        </div>
        <div class="textropolis-feedback">{move || feedback.get()}</div>

        <div class="textropolis-keyboard">
            <div
                class="textropolis-letters"
                style={move || match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let cols = if state.active_city_letters().len() <= 9 { 3 } else { 4 };
                        format!("--letter-cols:{cols};")
                    }
                    _ => String::new(),
                }}
            >
                {move || match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let selected = state.selected_letters().to_vec();
                        state
                            .active_city_letters()
                            .into_iter()
                            .enumerate()
                            .map(|(index, ch)| {
                                view! {
                                    <button
                                        type="button"
                                        data-letter-index=index
                                        class:selected=selected.contains(&index)
                                        on:click=move |_| {
                                            pick_letter(index);
                                        }
                                    >
                                        {ch}
                                    </button>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }
                    _ => ().into_any(),
                }}
            </div>
            <div class="textropolis-controls">
                <button type="button" on:click=leave>
                    "leave"
                </button>
                <button type="button" on:click=clear>
                    "clear"
                </button>
                <button type="button" on:click=move |_| submit()>
                    "submit"
                </button>
                <button type="button" on:click=backspace>
                    "backspace"
                </button>
            </div>
        </div>
    }
}
