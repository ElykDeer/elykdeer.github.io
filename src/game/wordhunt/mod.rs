mod data;
mod state;
mod storage;
mod view;

pub use storage::clear_progress;
pub use view::WordHuntGame;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::data::{Definition, WordHuntData};
    use super::state::{BoardShape, Coord, GuessResult, Puzzle, SavedProgress, WordHuntState};

    fn test_data() -> Arc<WordHuntData> {
        let mut definitions = HashMap::new();
        for word in [
            "able", "baker", "cable", "cared", "deal", "dear", "read", "reads", "seal", "seed",
            "tread",
        ] {
            definitions.insert(
                word.to_string(),
                vec![Definition {
                    part_of_speech: "noun".to_string(),
                    definition: format!("{word} definition"),
                }],
            );
        }
        Arc::new(WordHuntData::from_definitions(definitions))
    }

    fn length_mix_data() -> Arc<WordHuntData> {
        let mut definitions = HashMap::new();
        for word in [
            "able",
            "deal",
            "read",
            "seal",
            "baker",
            "cable",
            "light",
            "plane",
            "stone",
            "tread",
            "bridge",
            "candle",
            "garden",
            "planet",
            "silver",
            "stream",
            "blanket",
            "cabinet",
            "harvest",
            "lantern",
            "orchard",
            "station",
            "calendar",
            "mountain",
            "notebook",
            "shoulder",
            "triangle",
            "blueprint",
            "coastline",
            "framework",
            "sandstone",
            "starlight",
        ] {
            definitions.insert(
                word.to_string(),
                vec![Definition {
                    part_of_speech: "noun".to_string(),
                    definition: format!("{word} definition"),
                }],
            );
        }
        Arc::new(WordHuntData::from_definitions(definitions))
    }

    fn data_for_words(words: &[&str]) -> Arc<WordHuntData> {
        let mut definitions = HashMap::new();
        for word in words {
            definitions.insert(
                (*word).to_string(),
                vec![Definition {
                    part_of_speech: "noun".to_string(),
                    definition: format!("{word} definition"),
                }],
            );
        }
        Arc::new(WordHuntData::from_definitions(definitions))
    }

    fn diagonal_word_total(puzzle: &Puzzle) -> usize {
        puzzle
            .words
            .iter()
            .filter(|word| {
                word.paths.iter().any(|path| {
                    let (Some(first), Some(last)) = (path.first(), path.last()) else {
                        return false;
                    };
                    first.row != last.row && first.col != last.col
                })
            })
            .count()
    }

    #[test]
    fn straight_line_selection_includes_diagonals_and_rejects_bends() {
        assert_eq!(
            WordHuntState::line_between(Coord::new(1, 1), Coord::new(1, 4)).unwrap(),
            vec![
                Coord::new(1, 1),
                Coord::new(1, 2),
                Coord::new(1, 3),
                Coord::new(1, 4)
            ]
        );
        assert_eq!(
            WordHuntState::line_between(Coord::new(0, 0), Coord::new(3, 3)).unwrap(),
            vec![
                Coord::new(0, 0),
                Coord::new(1, 1),
                Coord::new(2, 2),
                Coord::new(3, 3)
            ]
        );
        assert!(WordHuntState::line_between(Coord::new(0, 0), Coord::new(2, 3)).is_none());
    }

    #[test]
    fn solver_discovers_accidental_words_and_filters_short_words() {
        let data = test_data();
        let rows = vec![
            "RABLE".to_string(),
            "XEXXX".to_string(),
            "XXAXX".to_string(),
            "XXXDX".to_string(),
            "XXXXX".to_string(),
        ];
        let puzzle = WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap();
        let words = puzzle
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert!(words.contains(&"able"));
        assert!(words.contains(&"read"));
        assert!(!words.contains(&"dear"));
        assert!(words.iter().all(|word| word.len() >= 4));
    }

    #[test]
    fn solver_drops_strict_subset_words_on_the_same_path() {
        let data = test_data();
        let rows = vec![
            "CABLE".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let puzzle = WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap();
        let words = puzzle
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert!(words.contains(&"cable"));
        assert!(!words.contains(&"able"));
    }

    #[test]
    fn solver_drops_reversed_strict_subset_words_on_the_same_path() {
        let data = data_for_words(&["abcde", "dcba"]);
        let rows = vec![
            "ABCDE".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let puzzle = WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap();
        let words = puzzle
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>();

        assert!(words.contains(&"abcde"));
        assert!(!words.contains(&"dcba"));
    }

    #[test]
    fn word_must_be_selected_in_dictionary_order() {
        let data = test_data();
        let rows = vec![
            "ELBAX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        state.begin_selection(Coord::new(0, 0));
        state.preview_selection(Coord::new(0, 3));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::NotInPuzzle { word } if word == "elba"
        ));
        assert!(!state.is_found("able"));

        state.begin_selection(Coord::new(0, 3));
        state.preview_selection(Coord::new(0, 0));

        assert!(matches!(
            state.submit_selection(),
            GuessResult::Accepted { word, .. } if word == "able"
        ));
        assert!(state.is_found("able"));
    }

    #[test]
    fn resume_candidate_requires_found_words_and_an_unfinished_board() {
        let data = test_data();
        let rows = vec![
            "ABLEX".to_string(),
            "READX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        assert!(!state.is_resumable());

        state.begin_selection(Coord::new(0, 0));
        state.preview_selection(Coord::new(0, 3));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::Accepted { word, .. } if word == "able"
        ));

        assert!(state.is_resumable());
    }

    #[test]
    fn generated_boards_are_deterministic_for_tests() {
        let data = test_data();
        let first =
            WordHuntState::generate_puzzle(Arc::clone(&data), BoardShape::square(8), 14, 12345);
        let second = WordHuntState::generate_puzzle(data, BoardShape::square(8), 14, 12345);

        assert_eq!(first.board, second.board);
        assert_eq!(first.words, second.words);
        assert!(first.words.iter().all(|word| word.word.len() >= 4));
    }

    #[test]
    fn generated_boards_keep_seeded_words_available() {
        let data = test_data();
        let puzzle = WordHuntState::generate_puzzle(data, BoardShape::square(8), 14, 98765);

        assert!(
            puzzle.words.len() >= 4,
            "expected at least a few words, got {:?}",
            puzzle.words
        );
    }

    #[test]
    fn generated_boards_can_be_rectangular_for_mobile() {
        let data = test_data();
        let puzzle = WordHuntState::generate_puzzle(data, BoardShape::new(13, 9), 24, 24680);

        assert_eq!(puzzle.rows(), 13);
        assert_eq!(puzzle.cols(), 9);
        assert_eq!(puzzle.board.len(), 13);
        assert!(puzzle.board.iter().all(|row| row.len() == 9));
        assert!(puzzle.words.iter().all(|word| {
            word.paths
                .iter()
                .flatten()
                .all(|coord| coord.row < 13 && coord.col < 9)
        }));
        assert!(
            diagonal_word_total(&puzzle) >= puzzle.words.len().div_ceil(4),
            "expected diagonal-heavy board, got {} diagonal words out of {}: {:?}",
            diagonal_word_total(&puzzle),
            puzzle.words.len(),
            puzzle.words
        );
    }

    #[test]
    fn generated_boards_include_long_words_when_available() {
        let data = length_mix_data();
        let puzzle = WordHuntState::generate_puzzle(data, BoardShape::new(12, 10), 30, 424242);
        let long_words = puzzle
            .words
            .iter()
            .filter(|word| word.word.len() >= 7)
            .count();
        let four_letter_words = puzzle
            .words
            .iter()
            .filter(|word| word.word.len() == 4)
            .count();

        assert!(
            long_words >= 2,
            "expected long words, got {long_words} in {:?}",
            puzzle.words
        );
        assert!(
            four_letter_words * 2 <= puzzle.words.len(),
            "expected 4-letter words not to dominate, got {four_letter_words} of {} in {:?}",
            puzzle.words.len(),
            puzzle.words
        );
    }

    #[test]
    fn failed_submission_clears_selection_for_next_attempt() {
        let data = test_data();
        let rows = vec![
            "ABLEX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        state.begin_selection(Coord::new(1, 0));
        state.preview_selection(Coord::new(1, 2));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::TooShort { word } if word == "xxx"
        ));
        assert!(state.selection().is_none());

        state.begin_selection(Coord::new(0, 0));
        state.preview_selection(Coord::new(0, 3));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::Accepted { word, .. } if word == "able"
        ));
    }

    #[test]
    fn strict_subset_words_mark_as_subwords_without_counting() {
        let data = test_data();
        let rows = vec![
            "CABLE".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        assert_eq!(state.total_count(), 1);
        assert!(!state
            .puzzle()
            .words
            .iter()
            .any(|entry| entry.word == "able"));

        state.begin_selection(Coord::new(0, 1));
        state.preview_selection(Coord::new(0, 4));

        assert!(matches!(
            state.submit_selection(),
            GuessResult::Subword { word, path } if word == "able"
                && path == vec![
                    Coord::new(0, 1),
                    Coord::new(0, 2),
                    Coord::new(0, 3),
                    Coord::new(0, 4)
                ]
        ));
        assert_eq!(state.found_count(), 0);
        assert_eq!(state.total_count(), 1);
        assert_eq!(
            state.subword_paths(),
            vec![vec![
                Coord::new(0, 1),
                Coord::new(0, 2),
                Coord::new(0, 3),
                Coord::new(0, 4)
            ]]
        );

        state.begin_selection(Coord::new(0, 1));
        state.preview_selection(Coord::new(0, 4));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::AlreadySubword { word, path } if word == "able"
                && path == vec![
                    Coord::new(0, 1),
                    Coord::new(0, 2),
                    Coord::new(0, 3),
                    Coord::new(0, 4)
                ]
        ));
        assert_eq!(state.subword_count(), 1);

        state.begin_selection(Coord::new(0, 0));
        state.preview_selection(Coord::new(0, 4));
        assert!(matches!(
            state.submit_selection(),
            GuessResult::Accepted { word, .. } if word == "cable"
        ));
        assert_eq!(state.found_count(), 1);
        assert_eq!(state.subword_count(), 1);
        assert_eq!(state.subwords(), vec!["able".to_string()]);
        assert!(state.subword_paths().is_empty());
    }

    #[test]
    fn reversed_subset_words_mark_as_subwords_without_counting() {
        let data = data_for_words(&["abcde", "dcba"]);
        let rows = vec![
            "ABCDE".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        assert_eq!(state.total_count(), 1);
        assert!(!state
            .puzzle()
            .words
            .iter()
            .any(|entry| entry.word == "dcba"));

        state.begin_selection(Coord::new(0, 3));
        state.preview_selection(Coord::new(0, 0));

        assert!(matches!(
            state.submit_selection(),
            GuessResult::Subword { word, path } if word == "dcba"
                && path == vec![
                    Coord::new(0, 3),
                    Coord::new(0, 2),
                    Coord::new(0, 1),
                    Coord::new(0, 0)
                ]
        ));
        assert_eq!(state.found_count(), 0);
        assert_eq!(state.subwords(), vec!["dcba".to_string()]);
    }

    #[test]
    fn embedded_word_that_is_also_an_answer_does_not_become_duplicate_subword() {
        let data = data_for_words(&["abcde", "dcba"]);
        let rows = vec![
            "ABCDE".to_string(),
            "DCBAX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        assert!(state
            .puzzle()
            .words
            .iter()
            .any(|entry| entry.word == "dcba"));

        state.begin_selection(Coord::new(0, 3));
        state.preview_selection(Coord::new(0, 0));

        assert!(matches!(
            state.submit_selection(),
            GuessResult::NotInPuzzle { word } if word == "dcba"
        ));
        assert_eq!(state.subword_count(), 0);
        assert!(state.subwords().is_empty());
    }

    #[test]
    fn saved_progress_does_not_persist_partial_selection() {
        let data = test_data();
        let rows = vec![
            "ABLEX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
            "XXXXX".to_string(),
        ];
        let mut state = WordHuntState::with_puzzle(
            Arc::clone(&data),
            WordHuntState::puzzle_from_rows(Arc::clone(&data), rows).unwrap(),
        );

        state.begin_selection(Coord::new(0, 0));
        state.preview_selection(Coord::new(0, 2));

        let saved = SavedProgress::from(&state);
        assert!(saved.selection.is_none());
    }
}
