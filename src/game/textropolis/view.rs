use std::collections::BTreeMap;
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
use super::state::{GuessResult, HintResult, TextropolisState, HINT_POPULATION_COST};
use super::storage::save_progress;

const SCROLL_DURATION_SECONDS: usize = 52;
const BANNER_BASE_DURATION_MS: u64 = SCROLL_DURATION_SECONDS as u64 * 1_000;
const BANNER_SLOW_DURATION_MS: u64 = BANNER_BASE_DURATION_MS + 6_000;
const BANNER_FASTEST_DURATION_MS: u64 = 32_000;
const BANNER_QUEUE_STEP_MS: u64 = 3_500;
const BANNER_SPEED_VARIATION_MS: i64 = 1_400;
const BANNER_STAGGER_MS: u64 = 2_500;
const BANNER_MIN_LANE_GAP_MS: u64 = 12_000;
const BANNER_MAX_LANE_GAP_MS: u64 = 44_000;
const BANNER_VIEWPORT_TRAVEL_UNITS: u64 = 160;
const BANNER_REFERENCE_TRAVEL_UNITS: u64 = 240;
const BANNER_TEXT_TRAVEL_CAP: usize = 120;
const BANNER_PLANE_CLEARANCE_UNITS: u64 = 70;
const BANNER_RANDOM_OFFSET_MS: u64 = 1_500;
const BANNER_LANES: usize = 4;
const MAX_DEFINITION_PLANES: usize = 5;
const TICKER_TARGET_CHARS: usize = 280;
const TICKER_REFERENCE_CHARS: usize = TICKER_TARGET_CHARS;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug)]
enum DictionaryLoad {
    Loading,
    Ready(TextropolisState),
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
enum FlyingDefinitionKind {
    Definition,
    Hint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WordListEntryKind {
    Found,
    Hint,
    Placeholder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FlyingDefinition {
    id: u64,
    kind: FlyingDefinitionKind,
    lane: usize,
    started_ms: u64,
    duration_ms: u64,
    travel_units: u64,
    text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WordListEntry {
    kind: WordListEntryKind,
    word: String,
    label: String,
    detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TickerItem {
    key: String,
    entry: WordListEntry,
    separator_after: bool,
}

fn format_population(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted.chars().rev().collect()
}

fn format_people(value: usize) -> String {
    format!("{} ppl", format_population(value))
}

fn feedback_for_guess(result: &GuessResult) -> (String, FeedbackTone) {
    match result {
        GuessResult::Accepted { word, .. } => (
            format!(
                "+{}",
                format_people(TextropolisState::word_population(word))
            ),
            FeedbackTone::Good,
        ),
        GuessResult::AlreadyFound { .. } => (String::new(), FeedbackTone::Neutral),
        GuessResult::Empty => (String::new(), FeedbackTone::Neutral),
        GuessResult::TooShort | GuessResult::NotFromCity | GuessResult::Unknown => {
            (String::new(), FeedbackTone::Error)
        }
    }
}

fn feedback_for_hint(result: &HintResult) -> (String, FeedbackTone) {
    match result {
        HintResult::Purchased { cost, .. } => (
            format!("Hint bought for {}. Clue incoming.", format_people(*cost)),
            FeedbackTone::Good,
        ),
        HintResult::NotEnoughPopulation { available, cost } => (
            format!(
                "Need {} for a hint. Population: {}.",
                format_people(*cost),
                format_people(*available)
            ),
            FeedbackTone::Warn,
        ),
        HintResult::NoWordsAvailable => (
            "No hidden words left to hint.".to_string(),
            FeedbackTone::Neutral,
        ),
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

fn lane_launch_gap_ms(duration_ms: u64, travel_units: u64) -> u64 {
    let banner_units = travel_units.saturating_sub(BANNER_VIEWPORT_TRAVEL_UNITS);
    let clearance_units = (banner_units + BANNER_PLANE_CLEARANCE_UNITS)
        .min(travel_units.saturating_sub(20))
        .max(1);
    duration_for_speed(clearance_units, duration_ms, travel_units)
        .clamp(BANNER_MIN_LANE_GAP_MS, BANNER_MAX_LANE_GAP_MS)
}

fn duration_for_speed(travel_units: u64, speed_duration_ms: u64, speed_travel_units: u64) -> u64 {
    let speed_travel_units = speed_travel_units.max(1);
    (travel_units * speed_duration_ms).div_ceil(speed_travel_units)
}

fn lane_schedule_from(
    definitions: &[FlyingDefinition],
    base_ms: u64,
    include_future: bool,
) -> (Vec<u64>, Vec<Option<(u64, u64)>>) {
    let mut next_launch_ms = vec![base_ms; BANNER_LANES];
    let mut lane_speed = vec![None; BANNER_LANES];
    let mut relevant = definitions
        .iter()
        .filter(|definition| {
            base_ms < definition.started_ms + definition.duration_ms
                && (include_future || definition.started_ms <= base_ms)
        })
        .collect::<Vec<_>>();
    relevant.sort_by_key(|definition| definition.started_ms);

    for definition in relevant {
        let Some(next_launch) = next_launch_ms.get_mut(definition.lane) else {
            continue;
        };
        *next_launch = (*next_launch).max(
            definition.started_ms
                + lane_launch_gap_ms(definition.duration_ms, definition.travel_units),
        );
        if let Some(speed) = lane_speed.get_mut(definition.lane) {
            *speed = Some((definition.duration_ms, definition.travel_units));
        };
    }

    (next_launch_ms, lane_speed)
}

fn resolve_definition_lanes(definitions: &mut [FlyingDefinition], base_ms: u64) -> Vec<u64> {
    let mut next_launch_ms = vec![base_ms; BANNER_LANES];
    for (lane, next_launch) in next_launch_ms.iter_mut().enumerate().take(BANNER_LANES) {
        let mut lane_indices = definitions
            .iter()
            .enumerate()
            .filter_map(|(index, definition)| (definition.lane == lane).then_some(index))
            .collect::<Vec<_>>();
        lane_indices.sort_by_key(|index| {
            let definition = &definitions[*index];
            let priority = match definition.kind {
                FlyingDefinitionKind::Hint => 0,
                FlyingDefinitionKind::Definition => 1,
            };
            (definition.started_ms, priority)
        });

        let mut available_ms = base_ms;
        let mut inherited_speed = None;
        for index in lane_indices {
            let definition = &mut definitions[index];
            if definition.started_ms <= base_ms {
                inherited_speed = Some((definition.duration_ms, definition.travel_units));
                available_ms = available_ms.max(
                    definition.started_ms
                        + lane_launch_gap_ms(definition.duration_ms, definition.travel_units),
                );
                continue;
            }
            if definition.started_ms < available_ms {
                definition.started_ms = available_ms;
            }
            if let Some((duration_ms, travel_units)) = inherited_speed {
                definition.duration_ms =
                    duration_for_speed(definition.travel_units, duration_ms, travel_units);
            }
            inherited_speed = Some((definition.duration_ms, definition.travel_units));
            available_ms = definition.started_ms
                + lane_launch_gap_ms(definition.duration_ms, definition.travel_units);
        }
        *next_launch = available_ms;
    }
    next_launch_ms
}

fn queue_definitions(
    definitions: &RwSignal<Vec<FlyingDefinition>>,
    next_banner_id: &RwSignal<u64>,
    word: &str,
    entries: &[Definition],
    kind: FlyingDefinitionKind,
    jump_queue: bool,
) {
    let base_ms = now_ms();
    let mut next_id = next_banner_id.get_untracked();
    let current_definitions = definitions
        .get_untracked()
        .into_iter()
        .filter(|definition| base_ms < definition.started_ms + definition.duration_ms)
        .collect::<Vec<_>>();
    let (mut lanes, mut lane_speed) =
        lane_schedule_from(&current_definitions, base_ms, !jump_queue);
    let queue_pressure = current_definitions.len() + entries.len().min(MAX_DEFINITION_PLANES);

    let mut cursor_ms = base_ms;
    let mut queued = Vec::new();
    for entry in entries.iter().take(MAX_DEFINITION_PLANES) {
        let text = definition_banner_text(word, entry, kind);
        let travel_units = banner_travel_units(&text);
        let duration_ms = banner_duration_ms(&text, queue_pressure, next_id);
        let (lane, available_ms) = lanes
            .iter()
            .enumerate()
            .min_by_key(|(_, available_ms)| **available_ms)
            .map(|(lane, available_ms)| (lane, *available_ms))
            .unwrap_or((0, base_ms));
        let offset_ms = if jump_queue { 0 } else { random_offset_ms() };
        let started_ms = cursor_ms.max(available_ms).max(base_ms) + offset_ms;
        let duration_ms = lane_speed
            .get(lane)
            .and_then(|speed| *speed)
            .map(|(lane_duration_ms, lane_travel_units)| {
                duration_for_speed(travel_units, lane_duration_ms, lane_travel_units)
            })
            .unwrap_or(duration_ms);
        lanes[lane] = started_ms + lane_launch_gap_ms(duration_ms, travel_units);
        if let Some(speed) = lane_speed.get_mut(lane) {
            *speed = Some((duration_ms, travel_units));
        }
        cursor_ms = started_ms + BANNER_STAGGER_MS;
        queued.push(FlyingDefinition {
            id: next_id,
            kind,
            lane,
            started_ms,
            duration_ms,
            travel_units,
            text,
        });
        next_id += 1;
    }

    next_banner_id.set(next_id);
    definitions.update(|definitions| {
        definitions.retain(|definition| base_ms < definition.started_ms + definition.duration_ms);
        definitions.extend(queued);
        resolve_definition_lanes(definitions, base_ms);
    });
}

fn ticker_duration_for_items(items: &[TickerItem]) -> usize {
    let estimated_chars = items
        .iter()
        .map(|item| {
            let separator_chars = if item.separator_after { 4 } else { 0 };
            item.entry.label.chars().count().max(1) + separator_chars + 2
        })
        .sum::<usize>()
        .max(TICKER_REFERENCE_CHARS);

    (SCROLL_DURATION_SECONDS * estimated_chars).div_ceil(TICKER_REFERENCE_CHARS)
}

fn definition_banner_text(word: &str, entry: &Definition, kind: FlyingDefinitionKind) -> String {
    match kind {
        FlyingDefinitionKind::Definition => format!(
            "{} ({}): {}",
            word.to_ascii_uppercase(),
            entry.part_of_speech,
            entry.definition
        ),
        FlyingDefinitionKind::Hint => {
            format!("HINT ({}): {}", entry.part_of_speech, entry.definition)
        }
    }
}

fn banner_travel_units(text: &str) -> u64 {
    BANNER_VIEWPORT_TRAVEL_UNITS + text.chars().count().clamp(40, BANNER_TEXT_TRAVEL_CAP) as u64
}

fn reference_duration_ms(queue_pressure: usize, id: u64) -> u64 {
    let pressure_duration_ms = match queue_pressure {
        0 | 1 => BANNER_SLOW_DURATION_MS,
        2 => BANNER_BASE_DURATION_MS,
        _ => BANNER_BASE_DURATION_MS.saturating_sub(
            ((queue_pressure - 2) as u64 * BANNER_QUEUE_STEP_MS)
                .min(BANNER_BASE_DURATION_MS - BANNER_FASTEST_DURATION_MS),
        ),
    };
    let variation_ms = ((id % 5) as i64 - 2) * (BANNER_SPEED_VARIATION_MS / 2);
    pressure_duration_ms
        .saturating_add_signed(variation_ms)
        .max(BANNER_FASTEST_DURATION_MS)
}

fn banner_duration_ms(text: &str, queue_pressure: usize, id: u64) -> u64 {
    duration_for_speed(
        banner_travel_units(text),
        reference_duration_ms(queue_pressure, id),
        BANNER_REFERENCE_TRAVEL_UNITS,
    )
    .max(BANNER_FASTEST_DURATION_MS)
}

fn word_list_entries(state: &TextropolisState, filter_by_selected: bool) -> Vec<WordListEntry> {
    let mut entries = if filter_by_selected {
        state.filtered_found_words()
    } else {
        state.found_words()
    }
    .into_iter()
    .map(|word| WordListEntry {
        kind: WordListEntryKind::Found,
        label: word.clone(),
        word,
        detail: None,
    })
    .collect::<Vec<_>>();

    let selected_prefix = state.selected_word();
    if !filter_by_selected || selected_prefix.is_empty() {
        entries.extend(
            state
                .hinted_words()
                .into_iter()
                .enumerate()
                .map(|(index, word)| {
                    let detail = state
                        .data
                        .definitions_for(&word)
                        .and_then(|definitions| definitions.into_iter().next())
                        .map(|definition| {
                            format!("{}: {}", definition.part_of_speech, definition.definition)
                        });
                    WordListEntry {
                        kind: WordListEntryKind::Hint,
                        label: format!("hint {}", index + 1),
                        word,
                        detail,
                    }
                }),
        );
    }

    entries
}

fn ticker_entries(entries: &[WordListEntry]) -> Vec<WordListEntry> {
    let gap = empty_ticker_entry();
    let seed = if entries.is_empty() {
        vec![gap.clone()]
    } else {
        let mut seed = entries.to_vec();
        seed.push(gap);
        seed
    };

    let mut repeated = Vec::new();
    while repeated
        .iter()
        .map(|entry: &WordListEntry| entry.label.len().max(1))
        .sum::<usize>()
        < TICKER_TARGET_CHARS
        || repeated.len() < 8
    {
        repeated.extend(seed.iter().cloned());
    }
    repeated
}

fn ticker_signature(entries: &[WordListEntry]) -> String {
    if entries.is_empty() {
        return "empty".to_string();
    }

    entries
        .iter()
        .map(ticker_entry_identity)
        .collect::<Vec<_>>()
        .join("|")
}

fn ticker_items(entries: &[WordListEntry]) -> Vec<TickerItem> {
    let mut seen = BTreeMap::<String, usize>::new();
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let separator_after = matches!(
                (entry.kind, entries.get(index + 1).map(|next| next.kind)),
                (
                    WordListEntryKind::Found | WordListEntryKind::Hint,
                    Some(WordListEntryKind::Found | WordListEntryKind::Hint)
                )
            );
            let identity = ticker_entry_identity(entry);
            let occurrence = seen.entry(identity.clone()).or_default();
            let key = format!("{identity}#{}", *occurrence);
            *occurrence += 1;

            TickerItem {
                key,
                entry: entry.clone(),
                separator_after,
            }
        })
        .collect()
}

fn ticker_entry_identity(entry: &WordListEntry) -> String {
    match entry.kind {
        WordListEntryKind::Found => format!("found:{}", entry.word),
        WordListEntryKind::Hint => format!("hint:{}:{}", entry.word, entry.label),
        WordListEntryKind::Placeholder => "gap".to_string(),
    }
}

fn empty_ticker_entry() -> WordListEntry {
    WordListEntry {
        kind: WordListEntryKind::Placeholder,
        word: String::new(),
        label: "·".to_string(),
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;

    use super::super::data::{Definition, TextropolisData};
    use super::super::state::{SavedProgress, TextropolisState};
    use super::*;

    fn found_entry(word: &str) -> WordListEntry {
        WordListEntry {
            kind: WordListEntryKind::Found,
            word: word.to_string(),
            label: word.to_string(),
            detail: None,
        }
    }

    fn flying_definition(
        id: u64,
        lane: usize,
        started_ms: u64,
        duration_ms: u64,
    ) -> FlyingDefinition {
        FlyingDefinition {
            id,
            kind: FlyingDefinitionKind::Definition,
            lane,
            started_ms,
            duration_ms,
            travel_units: BANNER_REFERENCE_TRAVEL_UNITS,
            text: format!("definition {id}"),
        }
    }

    fn word_list_test_state(selected_prefix: bool) -> TextropolisState {
        let mut definitions = HashMap::new();
        for word in ["phone", "peon"] {
            definitions.insert(
                word.to_string(),
                vec![Definition {
                    part_of_speech: "noun".to_string(),
                    definition: format!("{word} definition"),
                }],
            );
        }
        let data = Arc::new(TextropolisData::from_definitions(definitions));
        let progress = SavedProgress {
            guessed: BTreeMap::from([("Phoenix".to_string(), vec!["phone".to_string()])]),
            hints: BTreeMap::from([("Phoenix".to_string(), vec!["peon".to_string()])]),
            ..SavedProgress::default()
        };
        let mut state = TextropolisState::with_progress(data, progress);
        state.enter_city(0);
        if selected_prefix {
            state.pick_letter(1);
        }
        state
    }

    #[test]
    fn ticker_repeats_filter_then_dot_gap() {
        let entries = ticker_entries(&[found_entry("phone")]);
        let items = ticker_items(&entries);

        let labels = items
            .iter()
            .take(4)
            .map(|item| item.entry.label.as_str())
            .collect::<Vec<_>>();
        let separators = items
            .iter()
            .take(4)
            .map(|item| item.separator_after)
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["phone", "·", "phone", "·"]);
        assert_eq!(separators, vec![false, false, false, false]);
    }

    #[test]
    fn ticker_keys_follow_entry_identity_not_position() {
        let alpha = found_entry("alpha");
        let beta = found_entry("beta");

        let sorted_keys = ticker_items(&[alpha.clone(), beta.clone()])
            .into_iter()
            .map(|item| item.key)
            .collect::<Vec<_>>();
        let resorted_keys = ticker_items(&[beta, alpha])
            .into_iter()
            .map(|item| item.key)
            .collect::<Vec<_>>();

        assert!(sorted_keys.contains(&"found:alpha#0".to_string()));
        assert!(resorted_keys.contains(&"found:alpha#0".to_string()));
    }

    #[test]
    fn unfiltered_word_list_shows_hints() {
        let state = word_list_test_state(false);
        let entries = word_list_entries(&state, true);

        assert!(entries
            .iter()
            .any(|entry| entry.kind == WordListEntryKind::Hint && entry.word == "peon"));
    }

    #[test]
    fn selected_prefix_filters_found_words_and_hides_hints() {
        let state = word_list_test_state(true);
        let entries = word_list_entries(&state, true);

        assert!(!entries
            .iter()
            .any(|entry| entry.kind == WordListEntryKind::Found && entry.word == "phone"));
        assert!(!entries
            .iter()
            .any(|entry| entry.kind == WordListEntryKind::Hint && entry.word == "peon"));
    }

    #[test]
    fn lane_resolution_uses_launch_gap_instead_of_full_duration() {
        let mut definitions = vec![
            flying_definition(1, 0, 0, BANNER_BASE_DURATION_MS),
            flying_definition(2, 0, 2_000, BANNER_FASTEST_DURATION_MS),
        ];

        let next_launch_ms = resolve_definition_lanes(&mut definitions, 1_000);
        let expected_start =
            lane_launch_gap_ms(BANNER_BASE_DURATION_MS, BANNER_REFERENCE_TRAVEL_UNITS);

        assert_eq!(definitions[1].started_ms, expected_start);
        assert!(expected_start < BANNER_BASE_DURATION_MS);
        assert_eq!(next_launch_ms[0], expected_start + expected_start);
    }

    #[test]
    fn lane_resolution_never_makes_later_plane_faster() {
        let mut definitions = vec![
            flying_definition(1, 0, 0, BANNER_FASTEST_DURATION_MS),
            flying_definition(2, 0, 2_000, BANNER_SLOW_DURATION_MS),
        ];

        resolve_definition_lanes(&mut definitions, 1_000);

        assert_eq!(definitions[1].duration_ms, BANNER_FASTEST_DURATION_MS);
    }

    #[test]
    fn lane_resolution_scales_duration_for_longer_later_planes() {
        let mut definitions = vec![
            flying_definition(1, 0, 0, BANNER_FASTEST_DURATION_MS),
            FlyingDefinition {
                travel_units: BANNER_REFERENCE_TRAVEL_UNITS * 2,
                ..flying_definition(2, 0, 2_000, BANNER_FASTEST_DURATION_MS)
            },
        ];

        resolve_definition_lanes(&mut definitions, 1_000);

        assert_eq!(definitions[1].duration_ms, BANNER_FASTEST_DURATION_MS * 2);
    }

    #[test]
    fn lane_launch_gap_grows_for_longer_banners() {
        let short_gap = lane_launch_gap_ms(BANNER_BASE_DURATION_MS, BANNER_REFERENCE_TRAVEL_UNITS);
        let long_gap = lane_launch_gap_ms(
            BANNER_BASE_DURATION_MS * 2,
            BANNER_REFERENCE_TRAVEL_UNITS * 2,
        );

        assert!(long_gap > short_gap);
        assert!(long_gap < BANNER_BASE_DURATION_MS * 2);
    }
}

#[component]
pub fn TextropolisGame() -> impl IntoView {
    let section_ref = NodeRef::<Section>::new();
    let load = RwSignal::new(DictionaryLoad::Loading);
    let feedback = RwSignal::new(String::new());
    let feedback_tone = RwSignal::new(FeedbackTone::Neutral);
    let feedback_tick = RwSignal::new(0_u64);
    let guess_flash = RwSignal::new(0_u64);
    let word_list_open = RwSignal::new(false);
    let hint_confirming = RwSignal::new(false);
    let definitions = RwSignal::new(Vec::<FlyingDefinition>::new());
    let next_banner_id = RwSignal::new(0_u64);

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
        hint_confirming.set(false);
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
                        state.clear_selection();
                    }
                }
            });
            queue_definitions(
                &definitions,
                &next_banner_id,
                word,
                entries,
                FlyingDefinitionKind::Definition,
                false,
            );
        }
        let (message, tone) = feedback_for_guess(&result);
        feedback.set(message);
        feedback_tone.set(tone);
        match result {
            GuessResult::Accepted { .. } => {
                feedback_tick.update(|tick| *tick = tick.wrapping_add(1));
            }
            GuessResult::TooShort | GuessResult::NotFromCity | GuessResult::Unknown => {
                guess_flash.update(|tick| *tick = tick.wrapping_add(1));
            }
            GuessResult::Empty | GuessResult::AlreadyFound { .. } => {}
        }
    };

    let focus_game = move || {
        if let Some(section) = section_ref.get_untracked() {
            let _ = section.focus();
        }
    };

    let pick_letter = move |index: usize| {
        hint_confirming.set(false);
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.pick_letter(index);
            }
        });
    };

    let backspace = move |_| {
        hint_confirming.set(false);
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.backspace();
            }
        });
    };

    let clear = move |_| {
        hint_confirming.set(false);
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.clear_selection();
            }
        });
    };

    let replay_word = move |word: String| {
        hint_confirming.set(false);
        let entries = load.with_untracked(|load| match load {
            DictionaryLoad::Ready(state) => state.data.definitions_for(&word),
            _ => None,
        });
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.select_word(&word);
            }
        });
        if let Some(entries) = entries {
            queue_definitions(
                &definitions,
                &next_banner_id,
                &word,
                &entries,
                FlyingDefinitionKind::Definition,
                false,
            );
            feedback.set(String::new());
            feedback_tone.set(FeedbackTone::Neutral);
        }
    };

    let replay_hint = move |word: String| {
        hint_confirming.set(false);
        let entries = load.with_untracked(|load| match load {
            DictionaryLoad::Ready(state) => state.data.definitions_for(&word),
            _ => None,
        });
        if let Some(entries) = entries {
            queue_definitions(
                &definitions,
                &next_banner_id,
                &word,
                &entries,
                FlyingDefinitionKind::Hint,
                true,
            );
            feedback.set(String::new());
            feedback_tone.set(FeedbackTone::Neutral);
        }
    };

    let buy_hint = move |_| {
        if !hint_confirming.get_untracked() {
            let (available, remaining) = load.with_untracked(|load| match load {
                DictionaryLoad::Ready(state) => (
                    state
                        .current_city_index()
                        .map(|index| state.city_population(index))
                        .unwrap_or(0),
                    state.remaining_hint_count(),
                ),
                _ => (0, 0),
            });
            if remaining == 0 {
                let result = HintResult::NoWordsAvailable;
                let (message, tone) = feedback_for_hint(&result);
                feedback.set(message);
                feedback_tone.set(tone);
                feedback_tick.update(|tick| *tick = tick.wrapping_add(1));
                return;
            }
            if available < HINT_POPULATION_COST {
                let result = HintResult::NotEnoughPopulation {
                    available,
                    cost: HINT_POPULATION_COST,
                };
                let (message, tone) = feedback_for_hint(&result);
                feedback.set(message);
                feedback_tone.set(tone);
                feedback_tick.update(|tick| *tick = tick.wrapping_add(1));
                return;
            }
            hint_confirming.set(true);
            feedback.set(format!(
                "Tap hint again to spend {}.",
                format_people(HINT_POPULATION_COST)
            ));
            feedback_tone.set(FeedbackTone::Warn);
            feedback_tick.update(|tick| *tick = tick.wrapping_add(1));
            return;
        }

        hint_confirming.set(false);
        let mut result = HintResult::NoWordsAvailable;
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                result = state.buy_hint();
                if matches!(result, HintResult::Purchased { .. }) {
                    save_progress(state);
                }
            }
        });

        if let HintResult::Purchased {
            word,
            definitions: hint_definitions,
            ..
        } = &result
        {
            word_list_open.set(false);
            queue_definitions(
                &definitions,
                &next_banner_id,
                word,
                hint_definitions,
                FlyingDefinitionKind::Hint,
                true,
            );
        }

        let (message, tone) = feedback_for_hint(&result);
        feedback.set(message);
        feedback_tone.set(tone);
        feedback_tick.update(|tick| *tick = tick.wrapping_add(1));
    };

    let leave_city = move || {
        load.update(|load| {
            if let DictionaryLoad::Ready(state) = load {
                state.leave_city();
                save_progress(state);
            }
        });
        feedback.set(String::new());
        feedback_tone.set(FeedbackTone::Neutral);
        word_list_open.set(false);
        hint_confirming.set(false);
        definitions.set(Vec::new());
        next_banner_id.set(0);
    };

    let leave = move |_| leave_city();

    view! {
        <section
            class="terminal-game textropolis-game"
            node_ref=section_ref
            tabindex="0"
            on:mousedown=move |_| focus_game()
            on:click=move |_| focus_game()
            on:keydown=move |ev| {
                let key = ev.key();
                match key.as_str() {
                    "Enter" => {
                        ev.prevent_default();
                        submit();
                    }
                    "Backspace" => {
                        ev.prevent_default();
                        hint_confirming.set(false);
                        load.update(|load| {
                            if let DictionaryLoad::Ready(state) = load {
                                state.backspace();
                            }
                        });
                    }
                    "Escape" => {
                        ev.prevent_default();
                        hint_confirming.set(false);
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
                            hint_confirming.set(false);
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
                            feedback_tone=feedback_tone
                            feedback_tick=feedback_tick
                            guess_flash=guess_flash
                            word_list_open=word_list_open
                            hint_confirming=hint_confirming
                            definitions=definitions
                            submit=submit
                            pick_letter=pick_letter
                            replay_word=replay_word
                            replay_hint=replay_hint
                            buy_hint=buy_hint
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
        <div class="terminal-game-header textropolis-select-header">
            <span class="textropolis-select-progress">{move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    format!(
                        "{}/{} ({}%) | Population: {}",
                        state.total_found_count(),
                        state.total_word_count(),
                        state.total_percent(),
                        format_population(state.available_population())
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
                        let population = state.city_population(index);
                        let hint = state.unlock_hint(index);
                        let detail = if unlocked {
                            format!("{found}/{total} ({pct}%)")
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
                                <span class="textropolis-city-population-summary">
                                    {if population == 0 {
                                        String::new()
                                    } else {
                                        format_people(population)
                                    }}
                                </span>
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
    feedback_tone: RwSignal<FeedbackTone>,
    feedback_tick: RwSignal<u64>,
    guess_flash: RwSignal<u64>,
    word_list_open: RwSignal<bool>,
    hint_confirming: RwSignal<bool>,
    definitions: RwSignal<Vec<FlyingDefinition>>,
    submit: impl Fn() + Copy + Send + Sync + 'static,
    pick_letter: impl Fn(usize) + Copy + Send + Sync + 'static,
    replay_word: impl Fn(String) + Copy + Send + Sync + 'static,
    replay_hint: impl Fn(String) + Copy + Send + Sync + 'static,
    buy_hint: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    backspace: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    clear: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
    leave: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="terminal-game-header textropolis-city-header">
            <span class="textropolis-city-title">
                <button
                    type="button"
                    class="textropolis-exit-button"
                    aria-label="Back to city select"
                    on:click=leave
                >
                    <svg class="textropolis-exit-icon" viewBox="-5 0 29 24" aria-hidden="true" focusable="false">
                        <path
                            d="M14 4h5v16h-5"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linejoin="miter"
                        />
                        <circle cx="8" cy="6.2" r="1.55" fill="currentColor" />
                        <path
                            d="M8.1 8.4 6.3 12l3.1 1 1.9 3.5"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        />
                        <path
                            d="M7.1 10.3 10.4 9l2.6 2.1"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        />
                        <path
                            d="M5.9 16.8 9.4 13"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                        />
                        <path
                            d="M2.8 12H-3.8m0 0 2-2m-2 2 2 2"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.7"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        />
                    </svg>
                </button>
                <span class="textropolis-city-name">{move || match load.get() {
                    DictionaryLoad::Ready(state) => state.active_city_name().to_string(),
                    _ => String::new(),
                }}</span>
            </span>
            <span class="textropolis-city-stats">{move || match load.get() {
                DictionaryLoad::Ready(state) => {
                    let index = state.current_city_index().unwrap_or(0);
                    view! {
                        <span>
                            {format!(
                                "{}/{} ({}%)",
                                state.city_found_count(index),
                                state.city_total_count(index),
                                state.city_percent(index)
                            )}
                        </span>
                        <span class="textropolis-city-population">
                            {format_people(state.city_population(index))}
                        </span>
                    }.into_any()
                }
                _ => ().into_any(),
            }}</span>
        </div>

        <div class="textropolis-definition-sky">
            {move || {
                if !word_list_open.get() {
                    return ().into_any();
                }

                match load.get() {
                    DictionaryLoad::Ready(state) => {
                        let entries = word_list_entries(&state, true);
                        let has_entries = !entries.is_empty();
                        view! {
                            <div class="textropolis-word-list-overlay">
                                <div class="textropolis-word-list-scroll">
                                    {if has_entries {
                                        view! {
                                            <div class="textropolis-word-list-grid">
                                                {entries
                                                    .into_iter()
                                                    .map(|entry| {
                                                        let selected_word = entry.word.clone();
                                                        let entry_kind = entry.kind;
                                                        let detail = entry.detail.clone();
                                                        view! {
                                                            <button
                                                                type="button"
                                                                class="textropolis-word-list-word"
                                                                class:hint=entry_kind == WordListEntryKind::Hint
                                                                on:click=move |_| {
                                                                    word_list_open.set(false);
                                                                    match entry_kind {
                                                                        WordListEntryKind::Found => replay_word(selected_word.clone()),
                                                                        WordListEntryKind::Hint => replay_hint(selected_word.clone()),
                                                                        WordListEntryKind::Placeholder => {}
                                                                    }
                                                                }
                                                            >
                                                                <span>{entry.label}</span>
                                                                {detail
                                                                    .map(|detail| view! { <small>{detail}</small> }.into_any())
                                                                    .unwrap_or_else(|| ().into_any())}
                                                            </button>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="textropolis-word-list-empty">
                                                "No words found yet."
                                            </div>
                                        }.into_any()
                                    }}
                                </div>
                            </div>
                        }.into_any()
                    }
                    _ => ().into_any(),
                }
            }}
            <For
                each={move || definitions
                    .get()
                    .into_iter()
                    .filter(|definition| now_ms() < definition.started_ms + definition.duration_ms)
                    .collect::<Vec<_>>()}
                key=|definition| {
                    (
                        definition.id,
                        definition.lane,
                        definition.started_ms,
                        definition.duration_ms,
                    )
                }
                children={move |definition| {
                    let definition_id = definition.id;
                    let delay = definition.started_ms as i64 - now_ms() as i64;
                    view! {
                        <div
                            class="textropolis-banner definition"
                            class:hint=definition.kind == FlyingDefinitionKind::Hint
                            style=format!(
                                "--lane:{}; --duration:{}ms; --delay:{}ms;",
                                definition.lane,
                                definition.duration_ms,
                                delay
                            )
                        >
                            <div
                                class="textropolis-banner-flight"
                                on:animationend=move |_| {
                                    definitions.update(|definitions| {
                                        definitions.retain(|definition| definition.id != definition_id);
                                    });
                                }
                            >
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

        <div class=move || {
            let tick = guess_flash.get();
            if tick == 0 {
                "textropolis-current-word".to_string()
            } else if tick & 1 == 0 {
                "textropolis-current-word invalid-a".to_string()
            } else {
                "textropolis-current-word invalid-b".to_string()
            }
        }>
            {move || match load.get() {
                DictionaryLoad::Ready(state) => state.selected_word(),
                _ => String::new(),
            }}
        </div>
        <div class=move || {
            let pulse = if feedback_tick.get() & 1 == 0 { "pulse-a" } else { "pulse-b" };
            format!("textropolis-feedback {} {}", feedback_tone.get().class_name(), pulse)
        }>
            {move || feedback.get()}
        </div>

        <div class="textropolis-keyboard">
            <div class="textropolis-word-strip">
                <div class="textropolis-word-ticker" aria-live="polite">
                    <For
                        each=move || match load.get() {
                            DictionaryLoad::Ready(state) => {
                                let entries = word_list_entries(&state, true);
                                let signature = ticker_signature(&entries);
                                let loop_entries = ticker_entries(&entries);
                                vec![(signature, ticker_items(&loop_entries))]
                            }
                            _ => Vec::<(String, Vec<TickerItem>)>::new(),
                        }
                        key=|(signature, _)| signature.clone()
                        children=move |(_, loop_items)| {
                            let duplicate_items = loop_items.clone();
                            let ticker_duration = ticker_duration_for_items(&loop_items);
                            view! {
                                <div
                                    class="textropolis-ticker-track"
                                    style=format!("--ticker-duration:{}s;", ticker_duration)
                                >
                                    <span class="textropolis-ticker-loop">
                                        <For
                                            each=move || loop_items.clone()
                                            key=|item| item.key.clone()
                                            children=move |item| {
                                                let entry = item.entry;
                                                let selected_word = entry.word.clone();
                                                let entry_kind = entry.kind;
                                                match entry_kind {
                                                    WordListEntryKind::Placeholder => view! {
                                                        <span class="textropolis-ticker-dot">{entry.label}</span>
                                                    }.into_any(),
                                                    WordListEntryKind::Found | WordListEntryKind::Hint => view! {
                                                        <span class="textropolis-ticker-item">
                                                            <button
                                                                type="button"
                                                                class="textropolis-ticker-word"
                                                                class:hint=entry_kind == WordListEntryKind::Hint
                                                                on:click=move |_| {
                                                                    match entry_kind {
                                                                        WordListEntryKind::Found => replay_word(selected_word.clone()),
                                                                        WordListEntryKind::Hint => replay_hint(selected_word.clone()),
                                                                        WordListEntryKind::Placeholder => {}
                                                                    }
                                                                }
                                                            >
                                                                {entry.label}
                                                            </button>
                                                            {if item.separator_after {
                                                                view! { <span class="textropolis-ticker-separator">"//"</span> }.into_any()
                                                            } else {
                                                                ().into_any()
                                                            }}
                                                        </span>
                                                    }.into_any(),
                                                }
                                            }
                                        />
                                    </span>
                                    <span class="textropolis-ticker-loop duplicate" aria-hidden="true">
                                        <For
                                            each=move || duplicate_items.clone()
                                            key=|item| item.key.clone()
                                            children=move |item| {
                                                let entry = item.entry;
                                                let entry_kind = entry.kind;
                                                match entry_kind {
                                                    WordListEntryKind::Placeholder => view! {
                                                        <span class="textropolis-ticker-dot">{entry.label}</span>
                                                    }.into_any(),
                                                    WordListEntryKind::Found | WordListEntryKind::Hint => view! {
                                                        <span
                                                            class="textropolis-ticker-word"
                                                            class:hint=entry_kind == WordListEntryKind::Hint
                                                            aria-hidden="true"
                                                        >
                                                            {entry.label}
                                                        </span>
                                                        {if item.separator_after {
                                                            view! { <span class="textropolis-ticker-separator">"//"</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                    }.into_any(),
                                                }
                                            }
                                        />
                                    </span>
                                </div>
                            }
                        }
                    />
                </div>
                <button
                    type="button"
                    class="textropolis-word-list-toggle"
                    aria-label=move || if word_list_open.get() {
                        "Hide word list".to_string()
                    } else {
                        "Show word list".to_string()
                    }
                    aria-expanded=move || word_list_open.get().to_string()
                    on:click=move |_| word_list_open.update(|open| *open = !*open)
                >
                    <span aria-hidden="true"></span>
                </button>
            </div>
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
                <button
                    type="button"
                    disabled=move || match load.get() {
                        DictionaryLoad::Ready(state) => state.remaining_hint_count() == 0,
                        _ => true,
                    }
                    on:click=buy_hint
                >
                    {move || if hint_confirming.get() {
                        "confirm".to_string()
                    } else {
                        format!("hint -{}", format_population(HINT_POPULATION_COST))
                    }}
                </button>
                <button type="button" on:click=clear>
                    "clear"
                </button>
                <button type="button" on:click=backspace>
                    "backspace"
                </button>
                <button type="button" on:click=move |_| submit()>
                    {move || match load.get() {
                        DictionaryLoad::Ready(state) if state.selected_word_is_found() => {
                            "define".to_string()
                        }
                        _ => "submit".to_string(),
                    }}
                </button>
            </div>
        </div>
    }
}
