use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use super::super::word_data::parse_word_list;
pub(super) use super::super::word_data::Definition;

#[derive(Deserialize)]
struct ScopedWordList {
    wordhunt: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WordHuntData {
    definitions: HashMap<String, Vec<Definition>>,
    words: Vec<String>,
    word_set: HashSet<String>,
    letter_weights: [u32; 26],
    max_word_len: usize,
}

impl WordHuntData {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn from_word_list_json(json: &str) -> Result<Self, String> {
        let words = parse_word_list::<ScopedWordList>(json, |payload| payload.wordhunt)?;
        Ok(Self::from_words(words))
    }

    #[cfg(test)]
    pub(super) fn from_definitions(definitions: HashMap<String, Vec<Definition>>) -> Self {
        let words = definitions.keys().cloned().collect::<Vec<_>>();
        let mut data = Self::from_words(words);
        data.definitions = definitions;
        data
    }

    pub(super) fn from_words(words: Vec<String>) -> Self {
        let mut words = words
            .into_iter()
            .filter_map(|word| clean_word(&word))
            .filter(|word| word.len() >= 3)
            .collect::<Vec<_>>();
        words.sort();
        words.dedup();
        let word_set = words.iter().cloned().collect::<HashSet<_>>();

        let mut letter_weights = [1_u32; 26];
        for word in &words {
            for byte in word.bytes() {
                letter_weights[(byte - b'a') as usize] += 1;
            }
        }

        let max_word_len = words.iter().map(String::len).max().unwrap_or(0);

        Self {
            definitions: HashMap::new(),
            words,
            word_set,
            letter_weights,
            max_word_len,
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn load_definitions(&mut self, definitions: HashMap<String, Vec<Definition>>) {
        self.definitions = definitions;
    }

    pub(super) fn definitions_loaded(&self) -> bool {
        !self.definitions.is_empty()
    }

    pub(super) fn seed_words_for_size(&self, size: usize) -> Vec<&str> {
        self.words
            .iter()
            .map(String::as_str)
            .filter(|word| word.len() <= size)
            .collect()
    }

    pub(super) fn is_word(&self, word: &str) -> bool {
        self.word_set.contains(word)
    }

    pub(super) fn definitions_for(&self, word: &str) -> Vec<Definition> {
        self.definitions.get(word).cloned().unwrap_or_default()
    }

    pub(super) fn letter_weights(&self) -> &[u32; 26] {
        &self.letter_weights
    }

    pub(super) fn max_scan_len(&self, size: usize) -> usize {
        self.max_word_len.min(size)
    }
}

pub(super) fn clean_word(raw: &str) -> Option<String> {
    raw.chars()
        .all(|ch| ch.is_ascii_alphabetic())
        .then(|| raw.to_ascii_lowercase())
}
