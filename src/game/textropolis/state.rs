use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::data::{is_subanagram, percent, sanitize, Definition, TextropolisData};
use super::storage::load_saved_progress;

const WORD_POPULATION_MULTIPLIER: usize = 10;
const LEGACY_HINT_POINT_COST: usize = 30;
const LEGACY_HINT_POPULATION_COST: usize = LEGACY_HINT_POINT_COST * WORD_POPULATION_MULTIPLIER;
pub(super) const HINT_POPULATION_COST: usize = 800;

type WordSets = BTreeMap<String, BTreeSet<String>>;
type HintCosts = BTreeMap<String, BTreeMap<String, usize>>;
type ValidatedProgress = (WordSets, WordSets, HintCosts);

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct SavedProgress {
    #[serde(default)]
    pub(super) guessed: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(super) hints: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(super) hint_costs: BTreeMap<String, BTreeMap<String, usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GuessResult {
    Accepted {
        word: String,
        definitions: Vec<Definition>,
    },
    AlreadyFound {
        word: String,
        definitions: Vec<Definition>,
    },
    Empty,
    TooShort,
    NotFromCity,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HintResult {
    Purchased {
        word: String,
        definitions: Vec<Definition>,
        cost: usize,
    },
    NotEnoughPopulation {
        available: usize,
        cost: usize,
    },
    NoWordsAvailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Screen {
    Select,
    City(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TextropolisState {
    pub(super) data: Arc<TextropolisData>,
    screen: Screen,
    guessed: WordSets,
    hints: WordSets,
    hint_costs: HintCosts,
    selected_letters: Vec<usize>,
}

impl SavedProgress {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn load(data: &TextropolisData) -> ValidatedProgress {
        load_saved_progress()
            .and_then(|json| serde_json::from_str::<SavedProgress>(&json).ok())
            .map(|progress| progress.validated(data))
            .unwrap_or_default()
    }

    fn validated_words(
        data: &TextropolisData,
        saved_words: BTreeMap<String, Vec<String>>,
    ) -> WordSets {
        saved_words
            .into_iter()
            .filter_map(|(city, words)| {
                let city_data = data
                    .cities
                    .iter()
                    .find(|candidate| candidate.name == city)?;
                let words = words
                    .into_iter()
                    .map(|word| sanitize(&word).to_ascii_lowercase())
                    .filter(|word| {
                        word.len() > 3
                            && data.is_word(word)
                            && is_subanagram(&city_data.sanitized, word)
                    })
                    .collect::<BTreeSet<_>>();
                Some((city, words))
            })
            .collect()
    }

    fn validated(self, data: &TextropolisData) -> ValidatedProgress {
        let guessed = Self::validated_words(data, self.guessed);
        let hints = Self::validated_words(data, self.hints);
        let hint_costs = hints
            .iter()
            .map(|(city, words)| {
                let costs = words
                    .iter()
                    .map(|word| {
                        let cost = self
                            .hint_costs
                            .get(city)
                            .and_then(|costs| costs.get(word))
                            .copied()
                            .filter(|cost| *cost > 0)
                            .unwrap_or(LEGACY_HINT_POPULATION_COST);
                        (word.clone(), cost)
                    })
                    .collect();
                (city.clone(), costs)
            })
            .collect();

        (guessed, hints, hint_costs)
    }
}

impl From<&TextropolisState> for SavedProgress {
    fn from(state: &TextropolisState) -> Self {
        Self {
            guessed: state
                .guessed
                .iter()
                .map(|(city, words)| (city.clone(), words.iter().cloned().collect()))
                .collect(),
            hints: state
                .hints
                .iter()
                .map(|(city, words)| (city.clone(), words.iter().cloned().collect()))
                .collect(),
            hint_costs: state
                .hints
                .iter()
                .map(|(city, words)| {
                    let costs = words
                        .iter()
                        .map(|word| (word.clone(), state.hint_cost(city, word)))
                        .collect();
                    (city.clone(), costs)
                })
                .collect(),
        }
    }
}

impl TextropolisState {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn new(data: Arc<TextropolisData>) -> Self {
        let (guessed, hints, hint_costs) = SavedProgress::load(&data);
        Self {
            data,
            screen: Screen::Select,
            guessed,
            hints,
            hint_costs,
            selected_letters: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_progress(data: Arc<TextropolisData>, progress: SavedProgress) -> Self {
        let (guessed, hints, hint_costs) = progress.validated(&data);
        Self {
            data,
            screen: Screen::Select,
            guessed,
            hints,
            hint_costs,
            selected_letters: Vec::new(),
        }
    }

    pub(super) fn current_city_index(&self) -> Option<usize> {
        match self.screen {
            Screen::City(index) => Some(index),
            Screen::Select => None,
        }
    }

    pub(super) fn enter_city(&mut self, index: usize) {
        if self.is_unlocked(index) {
            self.screen = Screen::City(index);
            self.selected_letters.clear();
        }
    }

    pub(super) fn leave_city(&mut self) {
        self.screen = Screen::Select;
        self.selected_letters.clear();
    }

    pub(super) fn city_name(&self, index: usize) -> &'static str {
        self.data.city(index).name
    }

    pub(super) fn active_city_name(&self) -> &'static str {
        self.city_name(self.current_city_index().unwrap_or(0))
    }

    pub(super) fn active_city_letters(&self) -> Vec<char> {
        self.data
            .city(self.current_city_index().unwrap_or(0))
            .sanitized
            .chars()
            .collect()
    }

    pub(super) fn selected_letters(&self) -> &[usize] {
        &self.selected_letters
    }

    pub(super) fn selected_word(&self) -> String {
        let letters = self.active_city_letters();
        self.selected_letters
            .iter()
            .filter_map(|index| letters.get(*index))
            .collect::<String>()
    }

    pub(super) fn city_found_count(&self, index: usize) -> usize {
        self.guessed
            .get(self.city_name(index))
            .map(BTreeSet::len)
            .unwrap_or(0)
    }

    pub(super) fn city_total_count(&self, index: usize) -> usize {
        self.data.city(index).total_words
    }

    pub(super) fn city_percent(&self, index: usize) -> usize {
        percent(self.city_found_count(index), self.city_total_count(index))
    }

    pub(super) fn city_earned_population(&self, index: usize) -> usize {
        self.guessed
            .get(self.city_name(index))
            .into_iter()
            .flat_map(|words| words.iter())
            .map(|word| Self::word_population(word))
            .sum()
    }

    pub(super) fn city_spent_population(&self, index: usize) -> usize {
        self.hints
            .get(self.city_name(index))
            .map(|words| {
                words
                    .iter()
                    .map(|word| self.hint_cost(self.city_name(index), word))
                    .sum()
            })
            .unwrap_or(0)
    }

    pub(super) fn city_population(&self, index: usize) -> usize {
        self.city_earned_population(index)
            .saturating_sub(self.city_spent_population(index))
    }

    pub(super) fn total_found_count(&self) -> usize {
        self.guessed.values().map(BTreeSet::len).sum()
    }

    pub(super) fn total_word_count(&self) -> usize {
        self.data.cities.iter().map(|city| city.total_words).sum()
    }

    pub(super) fn total_percent(&self) -> usize {
        percent(self.total_found_count(), self.total_word_count())
    }

    pub(super) fn word_population(word: &str) -> usize {
        let len = word.chars().count();
        len * len * WORD_POPULATION_MULTIPLIER
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn earned_population(&self) -> usize {
        self.guessed
            .values()
            .flat_map(|words| words.iter())
            .map(|word| Self::word_population(word))
            .sum()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn spent_population(&self) -> usize {
        self.hints
            .iter()
            .flat_map(|(city, words)| words.iter().map(|word| self.hint_cost(city, word)))
            .sum()
    }

    pub(super) fn available_population(&self) -> usize {
        (0..self.data.cities.len())
            .map(|index| self.city_population(index))
            .sum()
    }

    pub(super) fn is_unlocked(&self, index: usize) -> bool {
        if index == 0 {
            return true;
        }
        self.city_percent(index - 1) >= 20
            || self.total_percent() >= (100 * index) / self.data.cities.len()
    }

    pub(super) fn unlock_hint(&self, index: usize) -> String {
        if self.is_unlocked(index) {
            return "open".to_string();
        }
        format!(
            "needs 20% in {} or {}% overall",
            self.city_name(index - 1),
            (100 * index) / self.data.cities.len()
        )
    }

    pub(super) fn found_words(&self) -> Vec<String> {
        self.guessed
            .get(self.active_city_name())
            .map(|words| words.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn hinted_words(&self) -> Vec<String> {
        let found = self
            .guessed
            .get(self.active_city_name())
            .cloned()
            .unwrap_or_default();
        self.hints
            .get(self.active_city_name())
            .map(|words| {
                words
                    .iter()
                    .filter(|word| !found.contains(*word))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn filtered_found_words(&self) -> Vec<String> {
        let prefix = self.selected_word().to_ascii_lowercase();
        let words = self.found_words();
        if prefix.is_empty() {
            words
        } else {
            words
                .into_iter()
                .filter(|word| word.starts_with(&prefix))
                .collect()
        }
    }

    pub(super) fn selected_word_is_found(&self) -> bool {
        let word = self.selected_word().to_ascii_lowercase();
        !word.is_empty()
            && self
                .guessed
                .get(self.active_city_name())
                .is_some_and(|words| words.contains(&word))
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn load_definitions(&mut self, definitions: HashMap<String, Vec<Definition>>) {
        Arc::make_mut(&mut self.data).load_definitions(definitions);
    }

    pub(super) fn definitions_loaded(&self) -> bool {
        self.data.definitions_loaded()
    }

    fn active_city_words(&self) -> Vec<String> {
        let Some(city_index) = self.current_city_index() else {
            return Vec::new();
        };
        let city = &self.data.city(city_index).sanitized;
        let mut words = self
            .data
            .words
            .iter()
            .filter(|word| word.len() > 3 && is_subanagram(city, word))
            .cloned()
            .collect::<Vec<_>>();
        words.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        words
    }

    fn remaining_hint_words(&self) -> Vec<String> {
        let found = self
            .guessed
            .get(self.active_city_name())
            .cloned()
            .unwrap_or_default();
        let hinted = self
            .hints
            .get(self.active_city_name())
            .cloned()
            .unwrap_or_default();
        self.active_city_words()
            .into_iter()
            .filter(|word| !found.contains(word) && !hinted.contains(word))
            .collect()
    }

    pub(super) fn remaining_hint_count(&self) -> usize {
        self.remaining_hint_words().len()
    }

    pub(super) fn buy_hint(&mut self) -> HintResult {
        let Some(city_index) = self.current_city_index() else {
            return HintResult::NoWordsAvailable;
        };
        let available = self.city_population(city_index);
        if available < HINT_POPULATION_COST {
            return HintResult::NotEnoughPopulation {
                available,
                cost: HINT_POPULATION_COST,
            };
        }

        let mut remaining = self.remaining_hint_words();
        if remaining.is_empty() {
            return HintResult::NoWordsAvailable;
        }
        let word = remaining.swap_remove(random_index(remaining.len()));

        let definitions = self.data.definitions_for(&word).unwrap_or_default();
        self.hints
            .entry(self.active_city_name().to_string())
            .or_default()
            .insert(word.clone());
        self.hint_costs
            .entry(self.active_city_name().to_string())
            .or_default()
            .insert(word.clone(), HINT_POPULATION_COST);

        HintResult::Purchased {
            word,
            definitions,
            cost: HINT_POPULATION_COST,
        }
    }

    pub(super) fn pick_letter(&mut self, index: usize) {
        if self.current_city_index().is_none() || self.selected_letters.contains(&index) {
            return;
        }
        if index < self.active_city_letters().len() {
            self.selected_letters.push(index);
        }
    }

    pub(super) fn type_letter(&mut self, letter: char) {
        let letter = letter.to_ascii_uppercase();
        if let Some((index, _)) =
            self.active_city_letters()
                .iter()
                .enumerate()
                .find(|(index, candidate)| {
                    !self.selected_letters.contains(index) && **candidate == letter
                })
        {
            self.selected_letters.push(index);
        }
    }

    pub(super) fn backspace(&mut self) {
        self.selected_letters.pop();
    }

    pub(super) fn clear_selection(&mut self) {
        self.selected_letters.clear();
    }

    pub(super) fn select_word(&mut self, raw: &str) {
        let word = sanitize(raw);
        let letters = self.active_city_letters();
        let mut selected = Vec::new();

        for ch in word.chars() {
            let Some((index, _)) = letters
                .iter()
                .enumerate()
                .find(|(index, candidate)| !selected.contains(index) && **candidate == ch)
            else {
                return;
            };
            selected.push(index);
        }

        self.selected_letters = selected;
    }

    #[cfg(test)]
    pub(super) fn submit_word(&mut self, raw: &str) -> GuessResult {
        let result = self.check_word(raw);
        if let GuessResult::Accepted { word, .. } = &result {
            self.record_accepted(word);
            self.selected_letters.clear();
        }
        result
    }

    pub(super) fn check_selected(&self) -> GuessResult {
        let word = self.selected_word().to_ascii_lowercase();
        self.check_word(&word)
    }

    fn check_word(&self, raw: &str) -> GuessResult {
        let Some(city_index) = self.current_city_index() else {
            return GuessResult::Empty;
        };
        let letters = sanitize(raw);
        if letters.is_empty() {
            return GuessResult::Empty;
        }
        if letters.len() < 4 {
            return GuessResult::TooShort;
        }
        if !is_subanagram(&self.data.city(city_index).sanitized, &letters) {
            return GuessResult::NotFromCity;
        }
        let word = letters.to_ascii_lowercase();
        if !self.data.is_word(&word) {
            return GuessResult::Unknown;
        }
        let definitions = self.data.definitions_for(&word).unwrap_or_default();
        if self
            .guessed
            .get(self.active_city_name())
            .is_some_and(|words| words.contains(&word))
        {
            return GuessResult::AlreadyFound { word, definitions };
        }

        GuessResult::Accepted { word, definitions }
    }

    pub(super) fn record_accepted(&mut self, word: &str) {
        let city = self.active_city_name().to_string();
        self.guessed
            .entry(city)
            .or_default()
            .insert(word.to_string());
    }

    fn hint_cost(&self, city: &str, word: &str) -> usize {
        self.hint_costs
            .get(city)
            .and_then(|costs| costs.get(word))
            .copied()
            .filter(|cost| *cost > 0)
            .unwrap_or(LEGACY_HINT_POPULATION_COST)
    }
}

#[cfg(target_arch = "wasm32")]
fn random_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((js_sys::Math::random() * len as f64).floor() as usize).min(len.saturating_sub(1))
}

#[cfg(not(target_arch = "wasm32"))]
fn random_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};

    if len == 0 {
        return 0;
    }
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    (duration.as_nanos() % len as u128) as usize
}
