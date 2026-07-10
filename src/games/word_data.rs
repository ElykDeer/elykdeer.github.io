#[cfg(any(target_arch = "wasm32", test))]
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct Definition {
    pub definition: String,
    pub part_of_speech: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WordListPayload<T> {
    Scoped(T),
    Flat(Vec<String>),
}

pub(super) fn parse_word_list<T>(
    json: &str,
    scoped_words: impl FnOnce(T) -> Vec<String>,
) -> Result<Vec<String>, String>
where
    T: DeserializeOwned,
{
    match serde_json::from_str(json).map_err(|err| err.to_string())? {
        WordListPayload::Scoped(scoped) => Ok(scoped_words(scoped)),
        WordListPayload::Flat(words) => Ok(words),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn parse_definitions(json: &str) -> Result<HashMap<String, Vec<Definition>>, String> {
    serde_json::from_str(json).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{parse_definitions, parse_word_list, Definition};

    #[derive(Deserialize)]
    struct ScopedWords {
        words: Vec<String>,
    }

    #[test]
    fn parses_flat_and_scoped_word_lists() {
        let flat =
            parse_word_list::<ScopedWords>(r#"["alpha","beta"]"#, |value| value.words).unwrap();
        let scoped =
            parse_word_list::<ScopedWords>(r#"{"words":["gamma"]}"#, |value| value.words).unwrap();

        assert_eq!(flat, ["alpha", "beta"]);
        assert_eq!(scoped, ["gamma"]);
    }

    #[test]
    fn parses_definition_dictionary() {
        let definitions =
            parse_definitions(r#"{"alpha":[{"definition":"first","part_of_speech":"adjective"}]}"#)
                .unwrap();

        assert_eq!(
            definitions["alpha"],
            [Definition {
                definition: "first".to_string(),
                part_of_speech: "adjective".to_string(),
            }]
        );
    }

    #[test]
    fn rejects_a_scoped_payload_without_the_expected_field() {
        assert!(
            parse_word_list::<ScopedWords>(r#"{"other":["alpha"]}"#, |value| value.words).is_err()
        );
    }
}
