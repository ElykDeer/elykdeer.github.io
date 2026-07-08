use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub(super) const CITIES: [&str; 27] = [
    "Phoenix",
    "Johannesburg",
    "Bangalore",
    "Buenos Aires",
    "Hyderabad",
    "Kuala Lumpur",
    "Mexico City",
    "Dusseldorf",
    "Khartoum",
    "Barcelona",
    "Philadelphia",
    "Rio de Janeiro",
    "Dar es Salaam",
    "Belo Horizonte",
    "Alexandria",
    "New York",
    "St. Petersburg",
    "Washington",
    "Istanbul",
    "Ho Chi Minh City",
    "Monterrey",
    "Los Angeles",
    "Melbourne",
    "Singapore",
    "Santiago",
    "San Francisco",
    "Houston",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Definition {
    pub definition: String,
    pub part_of_speech: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WordListPayload {
    Scoped { textropolis: Vec<String> },
    Flat(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CityData {
    pub(super) name: &'static str,
    pub(super) sanitized: String,
    pub(super) total_words: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TextropolisData {
    pub(super) definitions: HashMap<String, Vec<Definition>>,
    pub(super) words: Vec<String>,
    word_set: HashSet<String>,
    pub(super) cities: Vec<CityData>,
}

impl TextropolisData {
    #[allow(dead_code)]
    pub(super) fn from_json(json: &str) -> Result<Self, String> {
        Self::from_dictionary_json(json)
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn from_word_list_json(json: &str) -> Result<Self, String> {
        let words = match serde_json::from_str(json).map_err(|err| err.to_string())? {
            WordListPayload::Scoped { textropolis } => textropolis,
            WordListPayload::Flat(words) => words,
        };
        Ok(Self::from_words(words))
    }

    #[allow(dead_code)]
    pub(super) fn from_dictionary_json(json: &str) -> Result<Self, String> {
        let definitions: HashMap<String, Vec<Definition>> =
            serde_json::from_str(json).map_err(|err| err.to_string())?;
        Ok(Self::from_definitions(definitions))
    }

    #[allow(dead_code)]
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
            .filter(|word| word.len() > 3)
            .collect::<Vec<_>>();
        words.sort();
        words.dedup();
        let word_set = words.iter().cloned().collect::<HashSet<_>>();
        let cities = CITIES
            .iter()
            .map(|name| {
                let sanitized = sanitize(name);
                let total_words = words
                    .iter()
                    .filter(|word| word.len() > 3 && is_subanagram(&sanitized, word))
                    .count();
                CityData {
                    name,
                    sanitized,
                    total_words,
                }
            })
            .collect();

        Self {
            definitions: HashMap::new(),
            words,
            word_set,
            cities,
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn load_definitions(&mut self, definitions: HashMap<String, Vec<Definition>>) {
        self.definitions = definitions;
    }

    pub(super) fn definitions_loaded(&self) -> bool {
        !self.definitions.is_empty()
    }

    pub(super) fn city(&self, index: usize) -> &CityData {
        &self.cities[index]
    }

    pub(super) fn definitions_for(&self, word: &str) -> Option<Vec<Definition>> {
        self.definitions.get(word).cloned()
    }

    pub(super) fn is_word(&self, word: &str) -> bool {
        self.word_set.contains(word)
    }
}

fn clean_word(raw: &str) -> Option<String> {
    raw.chars()
        .all(|ch| ch.is_ascii_alphabetic())
        .then(|| raw.to_ascii_lowercase())
}

pub(super) fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .flat_map(|ch| ch.to_uppercase())
        .collect()
}

pub(super) fn is_subanagram(source: &str, test: &str) -> bool {
    let mut available = [0u16; 26];
    for byte in source.bytes().filter(|byte| byte.is_ascii_uppercase()) {
        available[(byte - b'A') as usize] += 1;
    }

    for byte in test.bytes().filter(|byte| byte.is_ascii_alphabetic()) {
        let byte = byte.to_ascii_uppercase();
        let slot = &mut available[(byte - b'A') as usize];
        if *slot == 0 {
            return false;
        }
        *slot -= 1;
    }

    true
}

pub(super) fn percent(found: usize, total: usize) -> usize {
    found.saturating_mul(100).checked_div(total).unwrap_or(0)
}
