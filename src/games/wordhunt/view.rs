#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

use leptos::html::{Div, Section};
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{KeyboardEvent, MouseEvent, PointerEvent};

#[cfg(target_arch = "wasm32")]
use super::super::word_data::parse_definitions;
use super::data::Definition;
#[cfg(target_arch = "wasm32")]
use super::data::WordHuntData;
#[cfg(target_arch = "wasm32")]
use super::state::RevealResult;
use super::state::{BoardShape, Coord, GuessResult, WordHuntDifficulty, WordHuntState};
use super::storage::save_progress;
use crate::terminal::WordHuntLaunchBoard;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug)]
enum DictionaryLoad {
    Loading,
    Ready(Box<WordHuntState>),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeedbackTone {
    Neutral,
    Good,
    Warn,
    Error,
}

impl FeedbackTone {
    fn class_name(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Good => "good",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointerPhase {
    Start,
    Preview,
    End,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoardRequest {
    Max,
    Shape(BoardShape),
}

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
#[cfg(target_arch = "wasm32")]
const MIN_BOARD_SIDE: usize = 5;
#[cfg(target_arch = "wasm32")]
const MIN_MOBILE_CELL_PX: f64 = 18.0;
#[cfg(target_arch = "wasm32")]
const MIN_DESKTOP_CELL_PX: f64 = 28.0;
#[cfg(target_arch = "wasm32")]
const DEFAULT_MOBILE_CELL_PX: f64 = 33.0;
#[cfg(target_arch = "wasm32")]
const DEFAULT_DESKTOP_CELL_PX: f64 = 40.0;
#[cfg(target_arch = "wasm32")]
const DEFAULT_MAX_ROW_COL_RATIO: f64 = 1.7;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykLoadWordList)]
    fn load_word_list_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykLoadWordHuntDictionary)]
    fn load_full_dictionary_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykDictionaryLoadStatus)]
    fn dictionary_load_status_js() -> String;
}

#[cfg(target_arch = "wasm32")]
async fn load_wordhunt_data() -> Result<Arc<WordHuntData>, String> {
    let value = JsFuture::from(load_word_list_js())
        .await
        .map_err(describe_js_error)?;
    let json = value
        .as_string()
        .ok_or_else(|| "word list loader returned a non-string value".to_string())?;
    WordHuntData::from_word_list_json(&json).map(Arc::new)
}

#[cfg(target_arch = "wasm32")]
async fn load_wordhunt_definitions() -> Result<HashMap<String, Vec<Definition>>, String> {
    let value = JsFuture::from(load_full_dictionary_js())
        .await
        .map_err(describe_js_error)?;
    let json = value
        .as_string()
        .ok_or_else(|| "dictionary loader returned a non-string value".to_string())?;
    parse_definitions(&json)
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
fn dictionary_waiting_message() -> String {
    let status = dictionary_load_status_js();
    let percent = js_sys::JSON::parse(&status)
        .ok()
        .and_then(|value| js_sys::Reflect::get(&value, &JsValue::from_str("percent")).ok())
        .and_then(|value| value.as_f64())
        .map(|value| value as usize);
    match percent {
        Some(percent) if percent < 100 => format!("Loading dictionary ({percent}%)."),
        _ => "Loading dictionary...".to_string(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dictionary_waiting_message() -> String {
    "Loading dictionary...".to_string()
}

#[cfg(target_arch = "wasm32")]
fn start_dictionary_load(
    load: RwSignal<DictionaryLoad>,
    dictionary_started: RwSignal<bool>,
    dictionary_loading: RwSignal<bool>,
    dictionary_pending_open: RwSignal<bool>,
    word_dialog_open: RwSignal<bool>,
    feedback: RwSignal<String>,
    feedback_tone: RwSignal<FeedbackTone>,
) {
    if dictionary_started.get_untracked() {
        return;
    }
    dictionary_started.set(true);
    dictionary_loading.set(true);
    spawn_local(async move {
        match load_wordhunt_definitions().await {
            Ok(definitions) => {
                load.update(|load| {
                    if let DictionaryLoad::Ready(state) = load {
                        state.load_definitions(definitions);
                    }
                });
                dictionary_loading.set(false);
                if dictionary_pending_open.get_untracked() {
                    dictionary_pending_open.set(false);
                    word_dialog_open.set(true);
                    feedback.set(String::new());
                    feedback_tone.set(FeedbackTone::Neutral);
                }
            }
            Err(err) => {
                dictionary_started.set(false);
                dictionary_loading.set(false);
                if dictionary_pending_open.get_untracked() {
                    feedback.set(format!("Dictionary failed: {err}"));
                    feedback_tone.set(FeedbackTone::Error);
                }
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn random_seed() -> u64 {
    let random_bits = (js_sys::Math::random() * u64::MAX as f64) as u64;
    random_bits ^ js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn random_seed() -> u64 {
    0
}

#[cfg(target_arch = "wasm32")]
fn viewport_board_shape(requested: Option<BoardRequest>) -> Result<BoardShape, String> {
    let Some(window) = web_sys::window() else {
        return Ok(match requested {
            Some(BoardRequest::Shape(shape)) => shape,
            _ => BoardShape::square(12),
        });
    };
    let width = window
        .inner_width()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(800.0);
    let height = window
        .inner_height()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(700.0);
    let mobile = width < 720.0;
    if mobile {
        return fitted_board_shape(
            width,
            stable_screen_height(&window, height),
            true,
            requested,
        );
    }
    fitted_board_shape(width, height, false, requested)
}

#[cfg(target_arch = "wasm32")]
fn fitted_board_shape(
    width: f64,
    height: f64,
    mobile: bool,
    requested: Option<BoardRequest>,
) -> Result<BoardShape, String> {
    let board_width = if mobile {
        mobile_board_width(width)
    } else {
        desktop_board_width(width, height)
    };
    let min_cell = if mobile {
        MIN_MOBILE_CELL_PX
    } else {
        MIN_DESKTOP_CELL_PX
    };
    let default_cell = if mobile {
        DEFAULT_MOBILE_CELL_PX
    } else {
        DEFAULT_DESKTOP_CELL_PX
    };
    let vertical_reserve = if mobile { 128.0 } else { 142.0 };
    let available_height = (height - vertical_reserve).max(MIN_BOARD_SIDE as f64 * min_cell);
    let max_cols = ((board_width / min_cell).floor() as usize).max(MIN_BOARD_SIDE);
    let square_cell = board_width / max_cols as f64;
    let max_square_rows = ((available_height / square_cell).floor() as usize).max(MIN_BOARD_SIDE);
    let max_square = max_cols.min(max_square_rows).max(MIN_BOARD_SIDE);

    match requested {
        Some(BoardRequest::Shape(shape)) => {
            if shape.cols < MIN_BOARD_SIDE || shape.rows < MIN_BOARD_SIDE {
                return Err(format!(
                    "Minimum board size is {MIN_BOARD_SIDE}x{MIN_BOARD_SIDE}."
                ));
            }
            if shape.cols > max_cols {
                return Err(format!(
                    "{}x{} will not fit. Max square: {max_square}x{max_square}; max width here is {max_cols} columns.",
                    shape.cols, shape.rows
                ));
            }
            let cell_size = board_width / shape.cols as f64;
            let max_rows = ((available_height / cell_size).floor() as usize).max(MIN_BOARD_SIDE);
            if shape.rows > max_rows {
                return Err(format!(
                    "{}x{} will not fit. Max square: {max_square}x{max_square}; with {} columns, max height is {max_rows} rows.",
                    shape.cols, shape.rows, shape.cols
                ));
            }
            Ok(shape)
        }
        Some(BoardRequest::Max) => Ok(BoardShape::square(max_square)),
        None => {
            let cols =
                ((board_width / default_cell).floor() as usize).clamp(MIN_BOARD_SIDE, max_cols);
            let cell_size = board_width / cols as f64;
            let fit_rows =
                ((available_height / cell_size).floor() as usize).clamp(MIN_BOARD_SIDE, usize::MAX);
            let balanced_rows =
                ((cols as f64 * DEFAULT_MAX_ROW_COL_RATIO).round() as usize).max(MIN_BOARD_SIDE);
            Ok(BoardShape::new(fit_rows.min(balanced_rows), cols))
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn mobile_board_width(width: f64) -> f64 {
    (width - 28.0).min(430.0).max(220.0)
}

#[cfg(target_arch = "wasm32")]
fn desktop_board_width(width: f64, height: f64) -> f64 {
    (width - 58.0).min(height - 126.0).max(320.0)
}

#[cfg(target_arch = "wasm32")]
fn stable_screen_height(window: &web_sys::Window, viewport_height: f64) -> f64 {
    let Some(screen_height) = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("screen"))
        .ok()
        .and_then(|screen| js_sys::Reflect::get(&screen, &JsValue::from_str("height")).ok())
        .and_then(|height| height.as_f64())
    else {
        return viewport_height;
    };
    if viewport_height < screen_height * 0.68 {
        screen_height * 0.82
    } else {
        viewport_height
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn viewport_board_shape(requested: Option<BoardRequest>) -> Result<BoardShape, String> {
    Ok(match requested {
        Some(BoardRequest::Shape(shape)) => shape,
        _ => BoardShape::square(12),
    })
}

fn launch_board_request(board: WordHuntLaunchBoard) -> BoardRequest {
    match board {
        WordHuntLaunchBoard::Max => BoardRequest::Max,
        WordHuntLaunchBoard::Shape { cols, rows } => {
            BoardRequest::Shape(BoardShape::new(rows, cols))
        }
    }
}

fn stage_style(rows: usize, cols: usize) -> String {
    format!(
        "--wordhunt-rows:{};--wordhunt-cols:{};--wordhunt-fixed-width:{:.3}rem;--wordhunt-fixed-width-mobile:{:.3}rem;",
        rows,
        cols,
        cols as f32 * 2.25,
        cols as f32 * 2.07,
    )
}

fn is_wordhunt_background_event(ev: &PointerEvent) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(target) = ev
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        else {
            return true;
        };
        return target
            .closest(
                ".wordhunt-grid, .wordhunt-dialog-backdrop, button, a, input, textarea, select",
            )
            .ok()
            .flatten()
            .is_none();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ev;
        false
    }
}

fn is_wordhunt_background_click(ev: &MouseEvent) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(target) = ev
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        else {
            return true;
        };
        return target
            .closest(
                ".wordhunt-grid, .wordhunt-dialog-backdrop, button, a, input, textarea, select",
            )
            .ok()
            .flatten()
            .is_none();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ev;
        false
    }
}

#[cfg(target_arch = "wasm32")]
fn focus_terminal_command_input() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(input) = document
        .get_element_by_id("terminal-command")
        .and_then(|element| element.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    else {
        return;
    };
    let _ = input.focus();
    let len = input.value().len() as u32;
    let _ = input.set_selection_range(len, len);
}

#[cfg(not(target_arch = "wasm32"))]
fn focus_terminal_command_input() {}

fn show_word_highlight(
    highlighted_path: RwSignal<Option<Vec<Coord>>>,
    path: Option<Vec<Coord>>,
    temporary: bool,
) {
    highlighted_path.set(path.clone());
    if temporary {
        #[cfg(target_arch = "wasm32")]
        if let (Some(window), Some(expected)) = (web_sys::window(), path) {
            let callback = Closure::once_into_js(move || {
                if highlighted_path.get_untracked().as_ref() == Some(&expected) {
                    highlighted_path.set(None);
                }
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                1_200,
            );
        }
    }
}

fn feedback_for_guess(result: &GuessResult) -> (String, FeedbackTone, Option<String>) {
    match result {
        GuessResult::Accepted { word, .. } => (
            format!("Found {}", word.to_ascii_uppercase()),
            FeedbackTone::Good,
            Some(word.clone()),
        ),
        GuessResult::AlreadyFound { word, .. } => (
            format!("Already found {}", word.to_ascii_uppercase()),
            FeedbackTone::Warn,
            Some(word.clone()),
        ),
        GuessResult::Subword { word, .. } => (
            format!("Subword found: {}", word.to_ascii_uppercase()),
            FeedbackTone::Warn,
            None,
        ),
        GuessResult::AlreadySubword { word, .. } => (
            format!("Already found subword: {}", word.to_ascii_uppercase()),
            FeedbackTone::Warn,
            None,
        ),
        GuessResult::Bonus { word, .. } => (
            format!("Bonus found: {}", word.to_ascii_uppercase()),
            FeedbackTone::Good,
            None,
        ),
        GuessResult::AlreadyBonus { word, .. } => (
            format!("Already found bonus: {}", word.to_ascii_uppercase()),
            FeedbackTone::Warn,
            None,
        ),
        GuessResult::Empty => (String::new(), FeedbackTone::Neutral, None),
        GuessResult::TooShort { word } => {
            let message = if word.is_empty() {
                "4+ letters.".to_string()
            } else {
                format!("\"{}\" needs 4+ letters.", word.to_ascii_uppercase())
            };
            (message, FeedbackTone::Warn, None)
        }
        GuessResult::NotInPuzzle { word } => (
            format!("\"{}\" does not match.", word.to_ascii_uppercase()),
            FeedbackTone::Error,
            None,
        ),
    }
}

fn short_definition(definitions: &[Definition]) -> String {
    if definitions.is_empty() {
        return "No definition available.".to_string();
    }
    definitions
        .iter()
        .take(2)
        .map(|definition| definition.definition.clone())
        .collect::<Vec<_>>()
        .join(" / ")
}

fn definition_lines(definitions: &[Definition]) -> Vec<String> {
    if definitions.is_empty() {
        return vec!["No definition available.".to_string()];
    }
    definitions
        .iter()
        .map(|definition| {
            if definition.part_of_speech.is_empty() {
                definition.definition.clone()
            } else {
                format!("{}: {}", definition.part_of_speech, definition.definition)
            }
        })
        .collect()
}

#[component]
pub fn WordHuntGame(
    #[prop(default = false)] reveal: bool,
    #[prop(optional)] board: Option<WordHuntLaunchBoard>,
) -> impl IntoView {
    let requested_board = board.map(launch_board_request);
    let difficulty = if matches!(board, Some(WordHuntLaunchBoard::Max)) {
        WordHuntDifficulty::Hard
    } else {
        WordHuntDifficulty::Standard
    };
    #[cfg(not(target_arch = "wasm32"))]
    let _ = difficulty;
    let fixed_size = matches!(board, Some(WordHuntLaunchBoard::Shape { .. }));
    let section_ref = NodeRef::<Section>::new();
    let load = RwSignal::new(DictionaryLoad::Loading);
    let feedback = RwSignal::new(String::new());
    let feedback_tone = RwSignal::new(FeedbackTone::Neutral);
    let word_dialog_open = RwSignal::new(false);
    let win_dialog_open = RwSignal::new(false);
    let resume_available = RwSignal::new(false);
    let dictionary_started = RwSignal::new(false);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = dictionary_started;
    let dictionary_loading = RwSignal::new(false);
    let dictionary_pending_open = RwSignal::new(false);
    let selected_detail = RwSignal::new(None::<String>);
    let highlighted_path = RwSignal::new(None::<Vec<Coord>>);
    let pointer_down = RwSignal::new(false);
    let pointer_moved = RwSignal::new(false);
    let game_active = RwSignal::new(false);
    let focused_coord = RwSignal::new(Coord::new(0, 0));

    #[cfg(target_arch = "wasm32")]
    spawn_local(async move {
        match load_wordhunt_data().await {
            Ok(data) => {
                let can_resume = !reveal
                    && WordHuntState::saved(Arc::clone(&data), difficulty)
                        .as_ref()
                        .is_some_and(WordHuntState::is_resumable);
                let shape = match viewport_board_shape(requested_board) {
                    Ok(shape) => shape,
                    Err(err) => {
                        load.set(DictionaryLoad::Failed(err));
                        return;
                    }
                };
                let mut state = if reveal {
                    WordHuntState::new(data, shape, random_seed(), difficulty)
                } else {
                    WordHuntState::fresh(data, shape, random_seed(), difficulty)
                };
                if reveal {
                    match state.reveal_all() {
                        RevealResult::Revealed { count } => {
                            feedback.set(format!("Revealed {count} words."));
                            feedback_tone.set(FeedbackTone::Good);
                            word_dialog_open.set(true);
                        }
                        RevealResult::AlreadyComplete => {
                            feedback.set("Everything is already revealed.".to_string());
                            feedback_tone.set(FeedbackTone::Neutral);
                            word_dialog_open.set(true);
                        }
                    }
                } else {
                    feedback.set("New board ready.".to_string());
                    feedback_tone.set(FeedbackTone::Neutral);
                }
                resume_available.set(can_resume);
                if reveal || !can_resume {
                    save_progress(&state);
                }
                load.set(DictionaryLoad::Ready(Box::new(state)));
            }
            Err(err) => load.set(DictionaryLoad::Failed(err)),
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = reveal;
        load.set(DictionaryLoad::Failed(
            "Word Hunt dictionary loading is only available in the browser build.".to_string(),
        ));
    }

    let focus_game = move || {
        game_active.set(true);
        if let Some(section) = section_ref.get_untracked() {
            let _ = section.focus();
        }
    };

    let set_feedback = move |message: String, tone: FeedbackTone| {
        feedback.set(message);
        feedback_tone.set(tone);
    };

    let save_unless_resume_available = move |state: &WordHuntState| {
        if !resume_available.get_untracked() {
            save_progress(state);
        }
    };

    let open_dictionary = move |word: Option<String>| {
        if let Some(word) = word {
            selected_detail.set(Some(word));
        }
        win_dialog_open.set(false);
        let definitions_ready = load.with_untracked(
            |load| matches!(load, DictionaryLoad::Ready(state) if state.definitions_loaded()),
        );
        if !definitions_ready {
            #[cfg(target_arch = "wasm32")]
            start_dictionary_load(
                load,
                dictionary_started,
                dictionary_loading,
                dictionary_pending_open,
                word_dialog_open,
                feedback,
                feedback_tone,
            );
            dictionary_pending_open.set(true);
            word_dialog_open.set(false);
            set_feedback(
                if dictionary_loading.get_untracked() {
                    dictionary_waiting_message()
                } else {
                    "Dictionary not ready.".to_string()
                },
                FeedbackTone::Warn,
            );
            return;
        }
        word_dialog_open.set(true);
    };

    let continue_board = move || {
        selected_detail.set(None);
        highlighted_path.set(None);
        word_dialog_open.set(false);
        win_dialog_open.set(false);
        pointer_down.set(false);
        pointer_moved.set(false);
        focused_coord.set(Coord::new(0, 0));
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                match viewport_board_shape(requested_board) {
                    Ok(shape) => {
                        state.continue_board(shape, random_seed());
                        resume_available.set(false);
                        save_progress(state);
                    }
                    Err(err) => {
                        set_feedback(err, FeedbackTone::Error);
                    }
                }
            }
        });
        if feedback_tone.get_untracked() != FeedbackTone::Error {
            set_feedback("New board ready.".to_string(), FeedbackTone::Neutral);
        }
    };

    let resume_last_board = move || {
        selected_detail.set(None);
        highlighted_path.set(None);
        word_dialog_open.set(false);
        win_dialog_open.set(false);
        pointer_down.set(false);
        pointer_moved.set(false);
        focused_coord.set(Coord::new(0, 0));
        let mut restored = false;
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                restored = state.restore_saved();
                if restored {
                    save_progress(state);
                }
            }
        });
        resume_available.set(false);
        if restored {
            set_feedback("Resumed.".to_string(), FeedbackTone::Neutral);
        } else {
            set_feedback("No saved board.".to_string(), FeedbackTone::Warn);
        }
    };

    let submit_current = move || {
        let mut result = GuessResult::Empty;
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                result = state.submit_selection();
                if !matches!(result, GuessResult::Empty) {
                    if matches!(result, GuessResult::Accepted { .. }) {
                        resume_available.set(false);
                        save_progress(state);
                    } else {
                        save_unless_resume_available(state);
                    }
                }
            }
        });
        let (message, tone, detail) = feedback_for_guess(&result);
        if let GuessResult::Bonus { path, .. } | GuessResult::AlreadyBonus { path, .. } = &result {
            show_word_highlight(highlighted_path, Some(path.clone()), true);
        }
        if let Some(word) = detail {
            selected_detail.set(Some(word));
        }
        if matches!(result, GuessResult::Accepted { .. })
            && load.with_untracked(
                |load| matches!(load, DictionaryLoad::Ready(state) if state.is_complete()),
            )
        {
            win_dialog_open.set(true);
        }
        set_feedback(message, tone);
    };

    let begin_selection = move |coord: Coord| {
        selected_detail.set(None);
        highlighted_path.set(None);
        focused_coord.set(coord);
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.begin_selection(coord);
                save_unless_resume_available(state);
            }
        });
    };

    let preview_selection = move |coord: Coord| -> bool {
        focused_coord.set(coord);
        let mut accepted = false;
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                accepted = state.preview_selection(coord);
                if accepted {
                    save_unless_resume_available(state);
                }
            }
        });
        accepted
    };

    let handle_pointer = move |coord: Coord, phase: PointerPhase| {
        focus_game();
        match phase {
            PointerPhase::Start => {
                let same_start = load.with_untracked(|load| {
                    matches!(
                        load,
                        DictionaryLoad::Ready(state)
                            if state.selection().is_some_and(|selection| {
                                selection.start == coord && selection.coords.len() == 1
                            })
                    )
                });
                if same_start {
                    selected_detail.set(None);
                    load.update(|load| {
                        if let DictionaryLoad::Ready(state) = load {
                            state.clear_selection();
                            save_unless_resume_available(state);
                        }
                    });
                    set_feedback(String::new(), FeedbackTone::Neutral);
                    pointer_down.set(false);
                    pointer_moved.set(false);
                    return;
                }
                let endpoint_click = load.with_untracked(|load| {
                    matches!(
                        load,
                        DictionaryLoad::Ready(state)
                            if state.selection().is_some_and(|selection| selection.start != coord)
                    )
                });
                if endpoint_click {
                    preview_selection(coord);
                    submit_current();
                    pointer_down.set(false);
                    pointer_moved.set(false);
                    return;
                }
                pointer_down.set(true);
                pointer_moved.set(false);
                begin_selection(coord);
            }
            PointerPhase::Preview => {
                if pointer_down.get_untracked() && preview_selection(coord) {
                    pointer_moved.set(true);
                }
            }
            PointerPhase::End => {
                if !pointer_down.get_untracked() {
                    return;
                }
                let moved = pointer_moved.get_untracked();
                pointer_down.set(false);
                preview_selection(coord);
                if moved {
                    submit_current();
                }
                pointer_moved.set(false);
            }
            PointerPhase::Cancel => {
                if !pointer_down.get_untracked() {
                    return;
                }
                pointer_down.set(false);
                pointer_moved.set(false);
            }
        }
    };

    let clear_selection = move || {
        selected_detail.set(None);
        highlighted_path.set(None);
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.clear_selection();
                save_unless_resume_available(state);
            }
        });
        set_feedback("Selection cleared.".to_string(), FeedbackTone::Neutral);
    };

    let handle_keydown = move |ev: KeyboardEvent| {
        if ev.ctrl_key() || ev.alt_key() || ev.meta_key() {
            return;
        }
        let key = ev.key();
        if win_dialog_open.get_untracked() {
            match key.as_str() {
                "Escape" => {
                    ev.prevent_default();
                    win_dialog_open.set(false);
                }
                " " | "Enter" => {
                    ev.prevent_default();
                    continue_board();
                }
                _ => {}
            }
            return;
        }
        if word_dialog_open.get_untracked() {
            if key == "Escape" {
                ev.prevent_default();
                word_dialog_open.set(false);
            }
            return;
        }
        match key.as_str() {
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" => {
                ev.prevent_default();
                let (dr, dc) = match key.as_str() {
                    "ArrowUp" => (-1, 0),
                    "ArrowDown" => (1, 0),
                    "ArrowLeft" => (0, -1),
                    _ => (0, 1),
                };
                let (rows, cols) = load.with_untracked(|load| match load {
                    DictionaryLoad::Ready(state) => (state.puzzle().rows(), state.puzzle().cols()),
                    _ => (1, 1),
                });
                let current = focused_coord.get_untracked();
                let row = (current.row as isize + dr).clamp(0, rows.saturating_sub(1) as isize);
                let col = (current.col as isize + dc).clamp(0, cols.saturating_sub(1) as isize);
                let next = Coord::new(row as usize, col as usize);
                if ev.shift_key() {
                    let has_selection = load.with_untracked(|load| {
                        matches!(load, DictionaryLoad::Ready(state) if state.selection().is_some())
                    });
                    if !has_selection {
                        begin_selection(current);
                    }
                    preview_selection(next);
                } else {
                    focused_coord.set(next);
                }
            }
            " " | "Enter" => {
                ev.prevent_default();
                let coord = focused_coord.get_untracked();
                let has_selection = load.with_untracked(|load| {
                    matches!(load, DictionaryLoad::Ready(state) if state.selection().is_some())
                });
                if has_selection {
                    preview_selection(coord);
                    submit_current();
                } else {
                    begin_selection(coord);
                }
            }
            "Backspace" => {
                ev.prevent_default();
                load.update(|load| {
                    if let DictionaryLoad::Ready(state) = load {
                        state.retract_selection();
                        save_unless_resume_available(state);
                    }
                });
            }
            "Escape" => {
                ev.prevent_default();
                clear_selection();
            }
            _ if key.len() == 1 => {
                let Some(letter) = key.chars().next().map(|ch| ch.to_ascii_uppercase()) else {
                    return;
                };
                if !letter.is_ascii_alphabetic() {
                    return;
                }
                if let Some(coord) =
                    next_matching_letter(load, focused_coord.get_untracked(), letter)
                {
                    focused_coord.set(coord);
                }
            }
            _ => {}
        }
    };

    let handle_background_pointerdown = move |ev: PointerEvent| {
        if is_wordhunt_background_event(&ev) {
            ev.stop_propagation();
        }
    };

    let handle_background_click = move |ev: MouseEvent| {
        if is_wordhunt_background_click(&ev) {
            ev.stop_propagation();
            game_active.set(false);
            highlighted_path.set(None);
            focus_terminal_command_input();
        }
    };

    view! {
        <section
            class="terminal-game wordhunt-game"
            class:wordhunt-fixed-size=fixed_size
            node_ref=section_ref
            tabindex="0"
            on:pointerdown=handle_background_pointerdown
            on:click=handle_background_click
            on:keydown=handle_keydown
        >
            {move || match load.get() {
                DictionaryLoad::Loading => view! {
                    <div class="terminal-game-header">
                        <span>"wordhunt"</span>
                        <span>"loading words"</span>
                    </div>
                    <div class="wordhunt-loading">"Loading words..."</div>
                }.into_any(),
                DictionaryLoad::Failed(err) => view! {
                    <div class="terminal-game-header">
                        <span>"wordhunt"</span>
                        <span>"setup error"</span>
                    </div>
                    <div class="wordhunt-error">
                        {format!("Could not start Word Hunt: {err}")}
                    </div>
                }.into_any(),
                DictionaryLoad::Ready(state) => {
                    let found = state.found_count();
                    let total = state.total_count();
                    let revealed = state.revealed_count();
                    let subwords = state.subword_count();
                    let bonuses = state.bonus_count();
                    let rows = state.puzzle().rows();
                    let cols = state.puzzle().cols();
                    view! {
                        <div class="wordhunt-stage" style=stage_style(rows, cols)>
                            <div class="terminal-game-header wordhunt-header">
                                <span class="wordhunt-count">
                                    <span>
                                        {if revealed == 0 {
                                            format!("{found}/{total}")
                                        } else {
                                            format!("{found}/{total} | {revealed} revealed")
                                        }}
                                    </span>
                                    {if subwords == 0 {
                                        ().into_any()
                                    } else {
                                        view! {
                                            <span class="wordhunt-subword-count">
                                                {format!("+{subwords}")}
                                            </span>
                                        }.into_any()
                                    }}
                                    {if bonuses == 0 {
                                        ().into_any()
                                    } else {
                                        view! {
                                            <span class="wordhunt-bonus-count" style="color:#69d17d;">
                                                {format!("★{bonuses}")}
                                            </span>
                                        }.into_any()
                                    }}
                                </span>
                                <span class=move || {
                                    format!(
                                        "wordhunt-feedback {}",
                                        feedback_tone.get().class_name()
                                    )
                                } aria-live="polite">
                                    {move || feedback.get()}
                                </span>
                                <div class="wordhunt-actions">
                                    {move || {
                                        if resume_available.get() {
                                            view! {
                                                <button
                                                    type="button"
                                                    aria-label="Reload last board"
                                                    on:click=move |_| resume_last_board()
                                                >
                                                    "Resume"
                                                </button>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }
                                    }}
                                    {move || {
                                        let complete = load.with(|load| {
                                            matches!(
                                                load,
                                                DictionaryLoad::Ready(state) if state.is_complete()
                                            )
                                        });
                                        if !resume_available.get() && complete {
                                            view! {
                                                <button
                                                    type="button"
                                                    aria-label="Show win screen"
                                                    on:click=move |_| win_dialog_open.set(true)
                                                >
                                                    "Win"
                                                </button>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }
                                    }}
                                    <button type="button" on:click=move |_| open_dictionary(None)>"Dictionary"</button>
                                </div>
                            </div>

                            <WordHuntBoard
                                load=load
                                highlighted_path=highlighted_path
                                focused_coord=focused_coord
                                game_active=game_active
                                handle_pointer=handle_pointer
                            />

                            <WordHuntInfoLine
                                load=load
                                selected_detail=selected_detail
                                open_dictionary=open_dictionary
                            />
                        </div>

                        {move || {
                            if word_dialog_open.get() {
                                view! {
                                    <WordHuntWordDialog
                                        load=load
                                        selected_detail=selected_detail
                                        highlighted_path=highlighted_path
                                        close=move |_| word_dialog_open.set(false)
                                    />
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                        {move || {
                            if win_dialog_open.get() {
                                view! {
                                    <WordHuntWinDialog
                                        continue_board=move |_| continue_board()
                                        open_dictionary=move |_| open_dictionary(None)
                                        close=move |_| win_dialog_open.set(false)
                                    />
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                    }.into_any()
                }
            }}
        </section>
    }
}

#[component]
fn WordHuntBoard(
    load: RwSignal<DictionaryLoad>,
    highlighted_path: RwSignal<Option<Vec<Coord>>>,
    focused_coord: RwSignal<Coord>,
    game_active: RwSignal<bool>,
    handle_pointer: impl Fn(Coord, PointerPhase) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="wordhunt-board-wrap">
            {move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    let selected_path = state.selected_path();
                    let selected = selected_path
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>();
                    let found_cells = state.found_cells();
                    let revealed_cells = state.revealed_cells();
                    let found_paths = state.found_paths();
                    let revealed_paths = state.revealed_paths();
                    let subword_paths = state.subword_paths();
                    let detail_path = highlighted_path.get();
                    let rows = state.puzzle().rows();
                    let cols = state.puzzle().cols();
                    view! {
                        <div
                            class="wordhunt-grid"
                            style=format!("--wordhunt-rows:{};--wordhunt-cols:{};", rows, cols)
                            aria-label="Word Hunt letter grid"
                            on:pointerdown=move |ev: PointerEvent| {
                                ev.prevent_default();
                                if let Some(coord) = coord_from_pointer_event(&ev) {
                                    capture_pointer(&ev);
                                    handle_pointer(coord, PointerPhase::Start);
                                }
                            }
                            on:pointermove=move |ev: PointerEvent| {
                                ev.prevent_default();
                                if let Some(coord) = coord_from_pointer_event(&ev) {
                                    handle_pointer(coord, PointerPhase::Preview);
                                }
                            }
                            on:pointerup=move |ev: PointerEvent| {
                                ev.prevent_default();
                                release_pointer(&ev);
                                if let Some(coord) = coord_from_pointer_event(&ev)
                                    .or_else(|| current_selection_end(load))
                                {
                                    handle_pointer(coord, PointerPhase::End);
                                }
                            }
                            on:pointercancel=move |ev: PointerEvent| {
                                ev.prevent_default();
                                release_pointer(&ev);
                                handle_pointer(Coord::new(0, 0), PointerPhase::Cancel);
                            }
                        >
                            <svg
                                class="wordhunt-path-layer"
                                viewBox=format!("0 0 {cols} {rows}")
                                aria-hidden="true"
                                focusable="false"
                            >
                                {revealed_paths
                                    .iter()
                                    .map(|path| view! {
                                        <polyline
                                            class="wordhunt-path-mark wordhunt-path-mark-revealed"
                                            style="stroke-width:0.70;"
                                            points=path_points(path)
                                        />
                                    })
                                    .collect_view()}
                                {found_paths
                                    .iter()
                                    .map(|path| view! {
                                        <polyline
                                            class="wordhunt-path-mark wordhunt-path-mark-found"
                                            style="stroke-width:0.82;"
                                            points=path_points(path)
                                        />
                                    })
                                    .collect_view()}
                                {subword_paths
                                    .iter()
                                    .map(|path| view! {
                                        <polyline
                                            class="wordhunt-path-mark wordhunt-path-mark-subword"
                                            points=path_points(path)
                                        />
                                    })
                                    .collect_view()}
                                {detail_path
                                    .as_ref()
                                    .map(|path| view! {
                                        <polyline
                                            class="wordhunt-path-mark wordhunt-path-mark-selected"
                                            style="stroke-width:0.94;"
                                            points=path_points(path)
                                        />
                                    })}
                                {if selected_path.len() > 1 {
                                    view! {
                                        <polyline
                                            class="wordhunt-path-mark wordhunt-path-mark-selected"
                                            style="stroke-width:0.94;"
                                            points=path_points(&selected_path)
                                        />
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </svg>
                            {(0..rows)
                                .flat_map(|row| (0..cols).map(move |col| Coord::new(row, col)))
                                .map(|coord| {
                                    let letter = state.board_char(coord).unwrap_or(' ');
                                    let is_selected = selected.contains(&coord);
                                    let is_found = found_cells.contains(&coord);
                                    let is_revealed = !is_found && revealed_cells.contains(&coord);
                                    let selected_index = selected_path
                                        .iter()
                                        .position(|candidate| *candidate == coord)
                                        .map(|index| index + 1);
                                    let is_focused = game_active.get() && focused_coord.get() == coord;
                                    let label = cell_label(letter, coord, is_selected, selected_index);
                                    view! {
                                        <button
                                            type="button"
                                            class="wordhunt-cell"
                                            class:selected=is_selected
                                            class:found=is_found
                                            class:revealed=is_revealed
                                            class:focused=is_focused
                                            data-wordhunt-cell=format!("{},{}", coord.row, coord.col)
                                            aria-label=label
                                            title=selected_index
                                                .map(|index| format!("{letter} - selection {index}"))
                                                .unwrap_or_else(|| letter.to_string())
                                        >
                                            <span>{letter}</span>
                                            {selected_index
                                                .map(|index| view! { <small>{index}</small> }.into_any())
                                                .unwrap_or_else(|| ().into_any())}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }.into_any()
                }
                _ => ().into_any(),
            }}
        </div>
    }
}

#[component]
fn WordHuntInfoLine(
    load: RwSignal<DictionaryLoad>,
    selected_detail: RwSignal<Option<String>>,
    open_dictionary: impl Fn(Option<String>) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="wordhunt-info-line" aria-live="polite">
            {move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    let selected_word = state.selected_word();
                    if !selected_word.is_empty() {
                        let selected_len = selected_word.chars().count();
                        view! {
                            <strong>{format!("Selecting {selected_len}")}</strong>
                            <span>{selected_word}</span>
                        }.into_any()
                    } else if let Some(word) = selected_detail.get() {
                        let definitions = state.definitions_for(&word);
                        let more_definitions = definitions.len().saturating_sub(2);
                        let dictionary_word = word.clone();
                        view! {
                            <strong>{word.to_ascii_uppercase()}</strong>
                            <button
                                type="button"
                                class="wordhunt-info-definition"
                                on:click=move |_| open_dictionary(Some(dictionary_word.clone()))
                            >
                                {short_definition(&definitions)}
                            </button>
                            <button
                                type="button"
                                class="wordhunt-info-more"
                                on:click=move |_| open_dictionary(Some(word.clone()))
                            >
                                {if more_definitions == 0 {
                                    "📖".to_string()
                                } else {
                                    format!("+{more_definitions} 📖")
                                }}
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <span class="wordhunt-info-empty">"Find a word."</span>
                        }.into_any()
                    }
                }
                _ => ().into_any(),
            }}
        </div>
    }
}

#[component]
fn WordHuntWordDialog(
    load: RwSignal<DictionaryLoad>,
    selected_detail: RwSignal<Option<String>>,
    highlighted_path: RwSignal<Option<Vec<Coord>>>,
    close: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let word_list_ref = NodeRef::<Div>::new();
    let jump_to_letter = move |letter: char| {
        let Some(list) = word_list_ref.get_untracked() else {
            return;
        };
        if let Ok(Some(target)) = list.query_selector(&format!("#wordhunt-letter-{letter}")) {
            target.scroll_into_view();
        }
    };
    let handle_alphabet_pointer = move |ev: PointerEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(letter) = alphabet_letter_from_pointer_event(&ev) {
            jump_to_letter(letter);
        }
    };

    view! {
        <div class="wordhunt-dialog-backdrop">
            <section
                class="wordhunt-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="wordhunt-dictionary-title"
            >
                <div class="wordhunt-dialog-header">
                    <h2 id="wordhunt-dictionary-title">"Dictionary"</h2>
                    <button type="button" aria-label="Close dictionary" on:click=close>"Close"</button>
                </div>
                {move || match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let mut seen_letters = std::collections::BTreeSet::new();
                        let mut word_rows = state.puzzle().words
                            .iter()
                            .map(|entry| {
                                let word = entry.word.clone();
                                let found = state.is_found(&word);
                                let revealed = state.is_revealed(&word) && !found;
                                (word, found, revealed, false, false)
                            })
                            .collect::<Vec<_>>();
                        word_rows.extend(
                            state
                                .subwords()
                                .into_iter()
                                .map(|word| (word, false, false, true, false)),
                        );
                        word_rows.extend(
                            state
                                .bonus_words()
                                .into_iter()
                                .map(|word| (word, false, false, false, true)),
                        );
                        word_rows.sort_by(|(a, ..), (b, ..)| a.cmp(b));
                        let word_rows = word_rows
                            .into_iter()
                            .map(|(word, found, revealed, subword, bonus)| {
                                let letter = word.chars().next().unwrap_or('a').to_ascii_uppercase();
                                let anchor = seen_letters
                                    .insert(letter)
                                    .then(|| format!("wordhunt-letter-{letter}"));
                                (word, letter, anchor, found, revealed, subword, bonus)
                            })
                            .collect::<Vec<_>>();
                        let present_letters = word_rows
                            .iter()
                            .map(|(_, letter, _, _, _, _, _)| *letter)
                            .collect::<std::collections::BTreeSet<_>>();
                        let fallback_word = state.puzzle().words.first().map(|entry| entry.word.clone());
                        view! {
                            <div class="wordhunt-dictionary-page">
                                <nav
                                    class="wordhunt-alphabet-scrollbar"
                                    aria-label="Dictionary letters"
                                    on:pointerdown=move |ev: PointerEvent| {
                                        capture_pointer(&ev);
                                        handle_alphabet_pointer(ev);
                                    }
                                    on:pointermove=handle_alphabet_pointer
                                    on:pointerup=move |ev: PointerEvent| {
                                        ev.prevent_default();
                                        ev.stop_propagation();
                                        release_pointer(&ev);
                                    }
                                    on:pointercancel=move |ev: PointerEvent| {
                                        ev.prevent_default();
                                        ev.stop_propagation();
                                        release_pointer(&ev);
                                    }
                                >
                                    {ALPHABET
                                        .chars()
                                        .map(|letter| {
                                            let available = present_letters.contains(&letter);
                                            if available {
                                                view! {
                                                    <button
                                                        type="button"
                                                        data-wordhunt-letter=letter.to_string()
                                                        on:click=move |_| jump_to_letter(letter)
                                                    >
                                                        {letter}
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <span
                                                        class="disabled"
                                                        aria-disabled="true"
                                                        data-wordhunt-letter=letter.to_string()
                                                    >
                                                        {letter}
                                                    </span>
                                                }.into_any()
                                            }
                                        })
                                        .collect_view()}
                                </nav>
                                <div
                                    class="wordhunt-word-list"
                                    node_ref=word_list_ref
                                    aria-label="Board dictionary words"
                                >
                                    {word_rows
                                        .into_iter()
                                        .map(|(word, _, anchor, found, revealed, subword, bonus)| {
                                            let row_id = anchor.unwrap_or_else(|| format!("wordhunt-word-{word}"));
                                            let selected_word = word.clone();
                                            let click_word = word.clone();
                                            let click_path = (found || subword || bonus)
                                                .then(|| state.path_for_word(&word))
                                                .flatten();
                                            view! {
                                                <button
                                                    type="button"
                                                    id=row_id
                                                    class="wordhunt-word-entry"
                                                    class:found=found
                                                    class:revealed=revealed
                                                    class:subword=subword
                                                    class:bonus=bonus
                                                    class:selected=move || {
                                                        selected_detail.get().as_deref() == Some(selected_word.as_str())
                                                    }
                                                    aria-label=format!(
                                                        "{}: {}",
                                                        word.to_ascii_uppercase(),
                                                        if found {
                                                            "found"
                                                        } else if subword {
                                                            "subword found"
                                                        } else if bonus {
                                                            "bonus found"
                                                        } else if revealed {
                                                            "revealed"
                                                        } else {
                                                            "not found"
                                                        },
                                                    )
                                                    on:click=move |ev| {
                                                        let navigate = selected_detail
                                                            .get_untracked()
                                                            .as_deref()
                                                            == Some(click_word.as_str())
                                                            && click_path.is_some();
                                                        selected_detail.set(Some(click_word.clone()));
                                                        if navigate {
                                                            show_word_highlight(
                                                                highlighted_path,
                                                                click_path.clone(),
                                                                subword || bonus,
                                                            );
                                                            close(ev);
                                                        }
                                                    }
                                                >
                                                    <span class="wordhunt-word-status">
                                                        {if found { "✓" } else if subword { "◇" } else if bonus { "★" } else if revealed { "!" } else { "·" }}
                                                    </span>
                                                    <span class="wordhunt-word-text">{word.to_ascii_uppercase()}</span>
                                                </button>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                                <WordHuntDefinitionPage
                                    load=load
                                    selected_detail=selected_detail
                                    fallback_word=fallback_word
                                />
                            </div>
                        }.into_any()
                    },
                    _ => ().into_any(),
                }}
            </section>
        </div>
    }
}

#[component]
fn WordHuntDefinitionPage(
    load: RwSignal<DictionaryLoad>,
    selected_detail: RwSignal<Option<String>>,
    fallback_word: Option<String>,
) -> impl IntoView {
    view! {
        <article class="wordhunt-definition-page-panel">
            {move || {
                let word = selected_detail.get().or_else(|| fallback_word.clone());
                let Some(word) = word else {
                    return ().into_any();
                };
                match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let definitions = definition_lines(&state.definitions_for(&word));
                        view! {
                            <header class="wordhunt-definition-title">
                                <strong>{word.to_ascii_uppercase()}</strong>
                            </header>
                            <div class="wordhunt-definition-scroll">
                                {definitions
                                    .into_iter()
                                    .map(|definition| view! {
                                        <p>{definition}</p>
                                    })
                                    .collect_view()}
                            </div>
                        }.into_any()
                    }
                    _ => ().into_any(),
                }
            }}
        </article>
    }
}

#[component]
fn WordHuntWinDialog(
    continue_board: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    open_dictionary: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    close: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="wordhunt-dialog-backdrop wordhunt-win-backdrop">
            <section
                class="wordhunt-dialog wordhunt-win-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="wordhunt-win-title"
            >
                <div class="wordhunt-win-body">
                    <h2 id="wordhunt-win-title">"Board complete"</h2>
                    <p>"All words found."</p>
                    <div class="wordhunt-win-actions">
                        <button type="button" on:click=continue_board>"Continue"</button>
                        <button type="button" on:click=open_dictionary>"Dictionary"</button>
                        <button type="button" on:click=close>"View board"</button>
                    </div>
                </div>
            </section>
        </div>
    }
}

fn path_points(path: &[Coord]) -> String {
    path.iter()
        .map(|coord| {
            format!(
                "{:.2},{:.2}",
                coord.col as f32 + 0.5,
                coord.row as f32 + 0.5
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn cell_label(letter: char, coord: Coord, selected: bool, selected_index: Option<usize>) -> String {
    let state = selected_index
        .map(|index| format!("selected position {index}"))
        .or_else(|| selected.then(|| "selected".to_string()))
        .unwrap_or_else(|| "not selected".to_string());
    format!(
        "{letter}, row {}, column {}, {state}",
        coord.row + 1,
        coord.col + 1
    )
}

fn next_matching_letter(
    load: RwSignal<DictionaryLoad>,
    current: Coord,
    letter: char,
) -> Option<Coord> {
    load.with_untracked(|load| {
        let DictionaryLoad::Ready(state) = load else {
            return None;
        };
        let rows = state.puzzle().rows();
        let cols = state.puzzle().cols();
        let cell_count = rows * cols;
        let start = current.row * cols + current.col + 1;
        (0..cell_count).find_map(|offset| {
            let index = (start + offset) % cell_count;
            let coord = Coord::new(index / cols, index % cols);
            (state.board_char(coord) == Some(letter)).then_some(coord)
        })
    })
}

fn coord_from_pointer_event(ev: &PointerEvent) -> Option<Coord> {
    coord_from_point(ev.client_x(), ev.client_y())
}

fn alphabet_letter_from_pointer_event(ev: &PointerEvent) -> Option<char> {
    alphabet_letter_from_point(ev.client_x(), ev.client_y())
}

fn current_selection_end(load: RwSignal<DictionaryLoad>) -> Option<Coord> {
    load.with_untracked(|load| match load {
        DictionaryLoad::Ready(state) => state.selection().map(|selection| selection.end),
        _ => None,
    })
}

#[cfg(target_arch = "wasm32")]
fn capture_pointer(ev: &PointerEvent) {
    if let Some(element) = ev
        .current_target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    {
        let _ = element.set_pointer_capture(ev.pointer_id());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn capture_pointer(_ev: &PointerEvent) {}

#[cfg(target_arch = "wasm32")]
fn release_pointer(ev: &PointerEvent) {
    if let Some(element) = ev
        .current_target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    {
        let _ = element.release_pointer_capture(ev.pointer_id());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn release_pointer(_ev: &PointerEvent) {}

#[cfg(target_arch = "wasm32")]
fn coord_from_point(x: i32, y: i32) -> Option<Coord> {
    let element = web_sys::window()?
        .document()?
        .element_from_point(x as f32, y as f32)?;
    let cell = element
        .closest("[data-wordhunt-cell]")
        .ok()
        .flatten()
        .unwrap_or(element);
    parse_coord(&cell.get_attribute("data-wordhunt-cell")?)
}

#[cfg(target_arch = "wasm32")]
fn alphabet_letter_from_point(x: i32, y: i32) -> Option<char> {
    let element = web_sys::window()?
        .document()?
        .element_from_point(x as f32, y as f32)?;
    let letter = element
        .closest("[data-wordhunt-letter]")
        .ok()
        .flatten()
        .unwrap_or(element);
    letter
        .get_attribute("data-wordhunt-letter")?
        .chars()
        .next()
        .filter(char::is_ascii_uppercase)
}

#[cfg(not(target_arch = "wasm32"))]
fn coord_from_point(_x: i32, _y: i32) -> Option<Coord> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn alphabet_letter_from_point(_x: i32, _y: i32) -> Option<char> {
    None
}

#[cfg(target_arch = "wasm32")]
fn parse_coord(raw: &str) -> Option<Coord> {
    let (row, col) = raw.split_once(',')?;
    Some(Coord::new(row.parse().ok()?, col.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_word_feedback_says_already_found() {
        let (message, _, _) = feedback_for_guess(&GuessResult::AlreadyFound {
            word: "able".to_string(),
            path: Vec::new(),
            definitions: Vec::<Definition>::new(),
        });

        assert_eq!(message, "Already found ABLE");
    }

    #[test]
    fn repeated_subword_feedback_says_already_found_subword() {
        let (message, _, _) = feedback_for_guess(&GuessResult::AlreadySubword {
            word: "able".to_string(),
            path: Vec::new(),
        });

        assert_eq!(message, "Already found subword: ABLE");
    }
}
