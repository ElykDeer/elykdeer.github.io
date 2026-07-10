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
    use super::state::{
        GuessResult, HintResult, SavedProgress, TextropolisState, HINT_POPULATION_COST,
    };

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

    fn funded_hint_data() -> Arc<TextropolisData> {
        let mut definitions = HashMap::new();
        for word in ["phone", "phoenix", "hone", "pheonix", "johannesburg"] {
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
    fn already_found_selection_stays_selected_and_returns_definitions() {
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
        assert_eq!(game.selected_word(), "PHONE");
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
            hints: BTreeMap::new(),
            ..SavedProgress::default()
        };

        let game = TextropolisState::with_progress(data, progress);

        assert_eq!(game.city_found_count(0), 1);
    }

    #[test]
    fn saved_progress_recomputes_population_from_guesses() {
        let data = test_data();
        let progress = SavedProgress {
            guessed: BTreeMap::from([(
                "Phoenix".to_string(),
                vec!["phone".to_string(), "hone".to_string()],
            )]),
            ..SavedProgress::default()
        };

        let game = TextropolisState::with_progress(data, progress);

        assert_eq!(game.earned_population(), 410);
        assert_eq!(game.spent_population(), 0);
        assert_eq!(game.available_population(), 410);
    }

    #[test]
    fn saved_hints_use_the_current_cost() {
        let data = funded_hint_data();
        let progress = SavedProgress {
            guessed: BTreeMap::from([(
                "Phoenix".to_string(),
                vec![
                    "phone".to_string(),
                    "hone".to_string(),
                    "phoenix".to_string(),
                ],
            )]),
            hints: BTreeMap::from([("Phoenix".to_string(), vec!["pheonix".to_string()])]),
            ..SavedProgress::default()
        };

        let game = TextropolisState::with_progress(data, progress);

        assert_eq!(game.city_earned_population(0), 900);
        assert_eq!(game.city_spent_population(0), HINT_POPULATION_COST);
        assert_eq!(game.city_population(0), 100);

        let saved = SavedProgress::from(&game);
        assert_eq!(
            saved.hints.get("Phoenix"),
            Some(&vec!["pheonix".to_string()])
        );
    }

    #[test]
    fn production_v1_payload_without_version_preserves_progress() {
        let data = funded_hint_data();
        let production_save = r#"{
            "guessed":{"Phoenix":["phone","hone","phoenix"]},
            "hints":{"Phoenix":["pheonix"]},
            "hint_costs":{"Phoenix":{"pheonix":300}}
        }"#;
        let progress = SavedProgress::from_json(production_save).expect("production v1 save");

        let game = TextropolisState::with_progress(data, progress);

        assert_eq!(game.city_found_count(0), 3);
        assert_eq!(game.hinted_words(), vec!["pheonix".to_string()]);
        assert_eq!(game.spent_population(), HINT_POPULATION_COST);
    }

    #[test]
    fn unsupported_explicit_save_version_is_rejected() {
        assert!(SavedProgress::from_json(r#"{"version":2,"guessed":{}}"#).is_none());
    }

    #[test]
    fn current_saves_are_explicit_and_omit_legacy_hint_costs() {
        let data = funded_hint_data();
        let production_save = r#"{
            "guessed":{"Phoenix":["phone","hone","phoenix"]},
            "hints":{"Phoenix":["pheonix"]},
            "hint_costs":{"Phoenix":{"pheonix":300}}
        }"#;
        let progress = SavedProgress::from_json(production_save).expect("production v1 save");
        let game = TextropolisState::with_progress(data, progress);

        let json = serde_json::to_value(SavedProgress::from(&game)).expect("serialize save");

        assert_eq!(json["version"], 1);
        assert!(json.get("hint_costs").is_none());
        assert_eq!(json["hints"]["Phoenix"][0], "pheonix");
    }

    #[test]
    fn hints_cost_city_population_and_remain_spent_after_guessing() {
        let data = funded_hint_data();
        let progress = SavedProgress {
            guessed: BTreeMap::from([(
                "Johannesburg".to_string(),
                vec!["johannesburg".to_string()],
            )]),
            hints: BTreeMap::new(),
            ..SavedProgress::default()
        };
        let mut game = TextropolisState::with_progress(Arc::clone(&data), progress);
        game.enter_city(0);

        assert!(matches!(
            game.submit_word("phone"),
            GuessResult::Accepted { .. }
        ));
        assert!(matches!(
            game.buy_hint(),
            HintResult::NotEnoughPopulation {
                available: 250,
                cost: HINT_POPULATION_COST
            }
        ));

        assert!(matches!(
            game.submit_word("hone"),
            GuessResult::Accepted { .. }
        ));
        assert!(matches!(
            game.submit_word("phoenix"),
            GuessResult::Accepted { .. }
        ));

        assert!(matches!(
            game.buy_hint(),
            HintResult::Purchased { word, cost, .. }
                if word == "pheonix" && cost == HINT_POPULATION_COST
        ));
        assert_eq!(game.hinted_words(), vec!["pheonix".to_string()]);
        assert_eq!(game.city_spent_population(0), HINT_POPULATION_COST);
        assert_eq!(game.spent_population(), HINT_POPULATION_COST);
        assert_eq!(game.city_population(0), 100);

        assert!(matches!(
            game.submit_word("pheonix"),
            GuessResult::Accepted { .. }
        ));
        assert!(game.hinted_words().is_empty());
        assert_eq!(game.city_spent_population(0), HINT_POPULATION_COST);
        assert_eq!(game.city_population(0), 590);

        let saved = SavedProgress::from(&game);
        assert_eq!(
            saved.hints.get("Phoenix"),
            Some(&vec!["pheonix".to_string()])
        );
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
