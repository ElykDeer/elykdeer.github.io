use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::data::{is_subanagram, percent, sanitize, Definition, TextropolisData};
use super::storage::load_saved_progress;

pub(super) const HINT_COST: usize = 30;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct SavedProgress {
    #[serde(default)]
    pub(super) guessed: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(super) hints: BTreeMap<String, Vec<String>>,
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
    NotEnoughPoints {
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
    guessed: BTreeMap<String, BTreeSet<String>>,
    hints: BTreeMap<String, BTreeSet<String>>,
    selected_letters: Vec<usize>,
}

impl SavedProgress {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn load(
        data: &TextropolisData,
    ) -> (
        BTreeMap<String, BTreeSet<String>>,
        BTreeMap<String, BTreeSet<String>>,
    ) {
        load_saved_progress()
            .and_then(|json| serde_json::from_str::<SavedProgress>(&json).ok())
            .map(|progress| progress.validated(data))
            .unwrap_or_default()
    }

    fn validated_words(
        data: &TextropolisData,
        saved_words: BTreeMap<String, Vec<String>>,
    ) -> BTreeMap<String, BTreeSet<String>> {
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

    fn validated(
        self,
        data: &TextropolisData,
    ) -> (
        BTreeMap<String, BTreeSet<String>>,
        BTreeMap<String, BTreeSet<String>>,
    ) {
        let guessed = Self::validated_words(data, self.guessed);
        let hints = Self::validated_words(data, self.hints);

        (guessed, hints)
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
        }
    }
}

impl TextropolisState {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn new(data: Arc<TextropolisData>) -> Self {
        let (guessed, hints) = SavedProgress::load(&data);
        Self {
            data,
            screen: Screen::Select,
            guessed,
            hints,
            selected_letters: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_progress(data: Arc<TextropolisData>, progress: SavedProgress) -> Self {
        let (guessed, hints) = progress.validated(&data);
        Self {
            data,
            screen: Screen::Select,
            guessed,
            hints,
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

    pub(super) fn city_points(&self, index: usize) -> usize {
        self.guessed
            .get(self.city_name(index))
            .into_iter()
            .flat_map(|words| words.iter())
            .map(|word| Self::word_score(word))
            .sum()
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

    pub(super) fn word_score(word: &str) -> usize {
        let len = word.chars().count();
        len * len
    }

    pub(super) fn earned_points(&self) -> usize {
        self.guessed
            .values()
            .flat_map(|words| words.iter())
            .map(|word| Self::word_score(word))
            .sum()
    }

    pub(super) fn spent_points(&self) -> usize {
        self.hints.values().map(BTreeSet::len).sum::<usize>() * HINT_COST
    }

    pub(super) fn available_points(&self) -> usize {
        self.earned_points().saturating_sub(self.spent_points())
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

    fn active_city_words(&self) -> Vec<String> {
        let Some(city_index) = self.current_city_index() else {
            return Vec::new();
        };
        let city = &self.data.city(city_index).sanitized;
        let mut words = self
            .data
            .definitions
            .keys()
            .filter(|word| word.len() > 3 && is_subanagram(city, word))
            .cloned()
            .collect::<Vec<_>>();
        words.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        words
    }

    pub(super) fn remaining_hint_count(&self) -> usize {
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
            .count()
    }

    pub(super) fn buy_hint(&mut self) -> HintResult {
        let available = self.available_points();
        if available < HINT_COST {
            return HintResult::NotEnoughPoints {
                available,
                cost: HINT_COST,
            };
        }

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
        let Some(word) = self
            .active_city_words()
            .into_iter()
            .find(|word| !found.contains(word) && !hinted.contains(word))
        else {
            return HintResult::NoWordsAvailable;
        };

        let definitions = self.data.definitions_for(&word).unwrap_or_default();
        self.hints
            .entry(self.active_city_name().to_string())
            .or_default()
            .insert(word.clone());

        HintResult::Purchased {
            word,
            definitions,
            cost: HINT_COST,
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
        match &result {
            GuessResult::Accepted { word, .. } => {
                self.record_accepted(word);
                self.selected_letters.clear();
            }
            _ => {}
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
        let Some(definitions) = self.data.definitions_for(&word) else {
            return GuessResult::Unknown;
        };
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
}
