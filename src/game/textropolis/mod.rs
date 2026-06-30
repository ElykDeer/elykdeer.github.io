mod data;
mod state;
mod storage;
mod view;

pub use view::TextropolisGame;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;

    use super::data::{is_subanagram, Definition, TextropolisData};
    use super::state::{GuessResult, SavedProgress, TextropolisState};

    fn test_data() -> Arc<TextropolisData> {
        let mut definitions = HashMap::new();
        for word in ["phone", "phoenix", "hone", "hoe", "one", "new", "york"] {
            definitions.insert(
                word.to_string(),
                vec![Definition {
                    part_of_speech: "noun".to_string(),
                    definition: format!("{word} definition"),
                }],
            );
        }
        Arc::new(TextropolisData::from_definitions(definitions))
    }

    #[test]
    fn subanagram_accounts_for_repeated_letters() {
        assert!(is_subanagram("BARCELONA", "cable"));
        assert!(!is_subanagram("NEWYORK", "rook"));
    }

    #[test]
    fn totals_are_recomputed_from_dictionary() {
        let data = test_data();

        assert_eq!(data.city(0).name, "Phoenix");
        assert_eq!(data.city(0).total_words, 3);
    }

    #[test]
    fn guesses_must_be_city_words() {
        let data = test_data();
        let mut game = TextropolisState::with_progress(Arc::clone(&data), SavedProgress::default());
        game.enter_city(0);

        assert!(matches!(
            game.submit_word("phone"),
            GuessResult::Accepted { .. }
        ));
        assert_eq!(game.city_found_count(0), 1);
        assert!(matches!(
            game.submit_word("phone"),
            GuessResult::AlreadyFound { word, definitions }
                if word == "phone" && definitions.len() == 1
        ));
        assert_eq!(game.submit_word("ox"), GuessResult::TooShort);
        assert_eq!(game.submit_word("hoe"), GuessResult::TooShort);
        assert_eq!(game.submit_word("house"), GuessResult::NotFromCity);
        assert_eq!(game.submit_word("noph"), GuessResult::Unknown);
    }

    #[test]
    fn accepted_selection_clears() {
        let data = test_data();
        let mut game = TextropolisState::with_progress(Arc::clone(&data), SavedProgress::default());
        game.enter_city(0);

        for index in [0, 1, 2, 4, 3] {
            game.pick_letter(index);
        }

        assert_eq!(game.selected_word(), "PHONE");
        assert!(matches!(
            game.submit_word("phone"),
            GuessResult::Accepted { .. }
        ));
        assert_eq!(game.selected_word(), "");
    }

    #[test]
    fn already_found_selection_clears_and_returns_definitions() {
        let data = test_data();
        let mut game = TextropolisState::with_progress(Arc::clone(&data), SavedProgress::default());
        game.enter_city(0);
        game.submit_word("phone");
        game.select_word("phone");

        assert_eq!(game.selected_word(), "PHONE");
        assert!(matches!(
            game.submit_word("phone"),
            GuessResult::AlreadyFound { word, definitions }
                if word == "phone" && definitions.len() == 1
        ));
        assert_eq!(game.selected_word(), "");
    }

    #[test]
    fn select_word_populates_matching_letters() {
        let data = test_data();
        let mut game = TextropolisState::with_progress(Arc::clone(&data), SavedProgress::default());
        game.enter_city(0);

        game.select_word("hone");

        assert_eq!(game.selected_word(), "HONE");
    }

    #[test]
    fn saved_progress_is_validated_against_current_dictionary() {
        let data = test_data();
        let progress = SavedProgress {
            guessed: BTreeMap::from([(
                "Phoenix".to_string(),
                vec!["phone".to_string(), "missing".to_string()],
            )]),
        };

        let game = TextropolisState::with_progress(data, progress);

        assert_eq!(game.city_found_count(0), 1);
    }

    #[test]
    fn unlocks_next_city_after_twenty_percent() {
        let data = test_data();
        let mut game = TextropolisState::with_progress(Arc::clone(&data), SavedProgress::default());
        game.enter_city(0);
        game.submit_word("phone");

        assert!(game.is_unlocked(1));
    }
}
