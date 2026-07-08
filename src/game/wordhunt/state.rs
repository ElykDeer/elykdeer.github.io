use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::data::{Definition, WordHuntData};
use super::storage::load_saved_progress;

const DIRECTIONS: [(isize, isize); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];
const DIAGONAL_DIRECTIONS: [(isize, isize); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const MIN_WORD_LEN: usize = 4;
const LONG_WORD_LEN: usize = 7;
const MIN_BOARD_SIDE: usize = 5;
const MAX_SEED_WORD_LEN: usize = 32;
const MAX_GENERATION_ROUNDS: usize = 30;
const WORD_PLACEMENT_TRIES: usize = 120;
const SHORT_SEED_LENGTH_WEIGHTS: [(usize, usize); 5] = [(4, 0), (5, 6), (6, 8), (7, 4), (8, 2)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoardShape {
    pub(super) rows: usize,
    pub(super) cols: usize,
}

impl BoardShape {
    pub(super) const fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols }
    }

    pub(super) const fn square(size: usize) -> Self {
        Self {
            rows: size,
            cols: size,
        }
    }

    fn normalized(self) -> Self {
        Self {
            rows: self.rows.max(MIN_BOARD_SIDE),
            cols: self.cols.max(MIN_BOARD_SIDE),
        }
    }

    fn area(self) -> usize {
        self.rows * self.cols
    }

    fn max_line_len(self) -> usize {
        self.rows.max(self.cols)
    }

    fn min_side(self) -> usize {
        self.rows.min(self.cols)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct Coord {
    pub(super) row: usize,
    pub(super) col: usize,
}

impl Coord {
    pub(super) const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct BoardWord {
    pub(super) word: String,
    pub(super) paths: Vec<Vec<Coord>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct Puzzle {
    #[serde(default)]
    pub(super) size: usize,
    #[serde(default)]
    pub(super) rows: usize,
    #[serde(default)]
    pub(super) cols: usize,
    pub(super) board: Vec<String>,
    pub(super) words: Vec<BoardWord>,
}

impl Puzzle {
    pub(super) fn rows(&self) -> usize {
        if self.rows == 0 {
            self.size
        } else {
            self.rows
        }
    }

    pub(super) fn cols(&self) -> usize {
        if self.cols == 0 {
            self.size
        } else {
            self.cols
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct SavedProgress {
    pub(super) puzzle: Option<Puzzle>,
    #[serde(default)]
    pub(super) found: BTreeMap<String, Vec<Coord>>,
    #[serde(default)]
    pub(super) revealed: BTreeMap<String, Vec<Coord>>,
    #[serde(default)]
    pub(super) subwords: BTreeMap<String, Vec<Coord>>,
    #[serde(default)]
    pub(super) selection: Option<Selection>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct Selection {
    pub(super) start: Coord,
    pub(super) end: Coord,
    pub(super) coords: Vec<Coord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GuessResult {
    Accepted {
        word: String,
        path: Vec<Coord>,
        definitions: Vec<Definition>,
    },
    AlreadyFound {
        word: String,
        path: Vec<Coord>,
        definitions: Vec<Definition>,
    },
    Subword {
        word: String,
        path: Vec<Coord>,
    },
    Empty,
    TooShort {
        word: String,
    },
    NotInPuzzle {
        word: String,
    },
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RevealResult {
    Revealed { count: usize },
    AlreadyComplete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WordHuntState {
    data: Arc<WordHuntData>,
    puzzle: Puzzle,
    found: BTreeMap<String, Vec<Coord>>,
    revealed: BTreeMap<String, Vec<Coord>>,
    subwords: BTreeMap<String, Vec<Coord>>,
    selection: Option<Selection>,
}

impl SavedProgress {
    fn load(data: Arc<WordHuntData>, shape: BoardShape, seed: u64) -> Self {
        Self::load_existing(&data).unwrap_or_else(|| SavedProgress {
            puzzle: Some(WordHuntState::generate_puzzle(
                data,
                shape,
                target_word_count(shape),
                seed,
            )),
            ..SavedProgress::default()
        })
    }

    fn load_existing(data: &WordHuntData) -> Option<Self> {
        load_saved_progress()
            .and_then(|json| serde_json::from_str::<SavedProgress>(&json).ok())
            .and_then(|progress| progress.validated(data))
    }

    fn validated(mut self, data: &WordHuntData) -> Option<Self> {
        let puzzle = self.puzzle.take()?;
        let puzzle = validate_puzzle(data, puzzle)?;
        let words = puzzle
            .words
            .iter()
            .map(|word| word.word.clone())
            .collect::<BTreeSet<_>>();

        self.found
            .retain(|word, path| words.contains(word) && path_belongs_to_word(&puzzle, word, path));
        self.revealed
            .retain(|word, path| words.contains(word) && path_belongs_to_word(&puzzle, word, path));
        self.subwords
            .retain(|word, path| subword_belongs_to_puzzle(data, &puzzle, word, path));
        if self
            .selection
            .as_ref()
            .is_some_and(|selection| !valid_selection_for_puzzle(&puzzle, selection))
        {
            self.selection = None;
        }
        self.puzzle = Some(puzzle);
        Some(self)
    }
}

impl From<&WordHuntState> for SavedProgress {
    fn from(state: &WordHuntState) -> Self {
        Self {
            puzzle: Some(state.puzzle.clone()),
            found: state.found.clone(),
            revealed: state.revealed.clone(),
            subwords: state.subwords.clone(),
            selection: state.selection.clone(),
        }
    }
}

impl WordHuntState {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn new(data: Arc<WordHuntData>, shape: BoardShape, seed: u64) -> Self {
        let progress = SavedProgress::load(Arc::clone(&data), shape, seed);
        Self {
            data,
            puzzle: progress
                .puzzle
                .expect("validated progress always has puzzle"),
            found: progress.found,
            revealed: progress.revealed,
            subwords: progress.subwords,
            selection: progress.selection,
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn saved(data: Arc<WordHuntData>) -> Option<Self> {
        let progress = SavedProgress::load_existing(&data)?;
        Some(Self {
            data,
            puzzle: progress
                .puzzle
                .expect("validated progress always has puzzle"),
            found: progress.found,
            revealed: progress.revealed,
            subwords: progress.subwords,
            selection: progress.selection,
        })
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn fresh(data: Arc<WordHuntData>, shape: BoardShape, seed: u64) -> Self {
        let puzzle =
            Self::generate_puzzle(Arc::clone(&data), shape, target_word_count(shape), seed);
        Self {
            data,
            puzzle,
            found: BTreeMap::new(),
            revealed: BTreeMap::new(),
            subwords: BTreeMap::new(),
            selection: None,
        }
    }

    #[cfg(test)]
    pub(super) fn with_puzzle(data: Arc<WordHuntData>, puzzle: Puzzle) -> Self {
        Self {
            data,
            puzzle,
            found: BTreeMap::new(),
            revealed: BTreeMap::new(),
            subwords: BTreeMap::new(),
            selection: None,
        }
    }

    pub(super) fn generate_puzzle(
        data: Arc<WordHuntData>,
        shape: BoardShape,
        target_words: usize,
        seed: u64,
    ) -> Puzzle {
        let shape = shape.normalized();
        let mut rng = Lcg::new(seed);
        let seed_words = data.seed_words_for_size(max_seed_word_len(shape));
        let minimum_seeded_words = minimum_seeded_word_count(shape, target_words, seed_words.len());
        let placement_target = seeded_placement_word_count(shape, target_words, seed_words.len());
        let length_plan =
            seed_length_allocation(placement_target, max_seed_word_len(shape), &seed_words);
        let minimum_diagonal_words = minimum_diagonal_word_count(shape, placement_target);
        let minimum_long_words = length_plan
            .iter()
            .filter(|len| **len >= LONG_WORD_LEN)
            .count();
        let mut best_puzzle = None::<(
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            Puzzle,
        )>;

        for _ in 0..MAX_GENERATION_ROUNDS {
            let mut board = vec![vec![None::<char>; shape.cols]; shape.rows];
            let mut candidates = seed_candidates_for_lengths(&seed_words, &length_plan, &mut rng);
            dedup_preserving_order(&mut candidates);
            remove_strict_word_subsets(&mut candidates);

            let mut placed_words = BTreeSet::new();
            let mut placed_diagonal_words = 0;
            let mut placed_long_words = 0;
            while !candidates.is_empty() {
                if placed_words.len() >= placement_target {
                    break;
                }
                let prefer_diagonal = placed_diagonal_words < minimum_diagonal_words;
                let prefer_long = placed_long_words < minimum_long_words;
                let index = if prefer_long {
                    candidates
                        .iter()
                        .position(|word| is_long_word(word))
                        .or_else(|| {
                            prefer_diagonal.then(|| {
                                candidates
                                    .iter()
                                    .position(|word| word.len() <= shape.min_side())
                                    .unwrap_or(0)
                            })
                        })
                        .unwrap_or(0)
                } else if prefer_diagonal {
                    candidates
                        .iter()
                        .position(|word| word.len() <= shape.min_side())
                        .unwrap_or(0)
                } else {
                    0
                };
                let word = candidates.remove(index);
                if let Some(diagonal) = place_word(&mut board, &word, &mut rng, prefer_diagonal) {
                    if diagonal {
                        placed_diagonal_words += 1;
                    }
                    if is_long_word(&word) {
                        placed_long_words += 1;
                    }
                    placed_words.insert(word);
                }
            }
            if placed_words.len() < minimum_seeded_words {
                continue;
            }

            fill_board(&mut board, data.letter_weights(), &mut rng);
            let rows = board
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|ch| ch.unwrap_or('E'))
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            let puzzle = solve_rows(&data, rows);
            let seeded_answers = seeded_answer_count(&puzzle, &placed_words);
            let diagonal_answers = diagonal_word_count(&puzzle);
            let long_seeded_answers = long_seeded_answer_count(&puzzle, &placed_words);
            let non_short_answers = non_short_word_count(&puzzle);
            let required_diagonal_answers =
                required_diagonal_answer_count(minimum_diagonal_words, puzzle.words.len());
            let constraints_met = seeded_answers >= minimum_seeded_words
                && diagonal_answers >= required_diagonal_answers
                && long_seeded_answers >= minimum_long_words.min(puzzle.words.len())
                && puzzle.words.len() >= seeded_answers;
            let target_fit = target_words.saturating_sub(puzzle.words.len().abs_diff(target_words));
            let diagonal_ratio = if puzzle.words.is_empty() {
                0
            } else {
                diagonal_answers * 100 / puzzle.words.len()
            };
            let quality = (
                constraints_met as usize,
                target_fit,
                long_seeded_answers,
                non_short_answers,
                diagonal_answers,
                seeded_answers,
                diagonal_ratio,
            );
            if best_puzzle.as_ref().is_none_or(
                |(
                    best_constraints,
                    best_target_fit,
                    best_long,
                    best_non_short,
                    best_diagonal,
                    best_seeded,
                    _best_total,
                    best_ratio,
                    _,
                )| {
                    quality
                        > (
                            *best_constraints,
                            *best_target_fit,
                            *best_long,
                            *best_non_short,
                            *best_diagonal,
                            *best_seeded,
                            *best_ratio,
                        )
                },
            ) {
                best_puzzle = Some((
                    constraints_met as usize,
                    target_fit,
                    long_seeded_answers,
                    non_short_answers,
                    diagonal_answers,
                    seeded_answers,
                    puzzle.words.len(),
                    diagonal_ratio,
                    puzzle,
                ));
            }
        }

        best_puzzle
            .filter(|(_, _, _, _, _, seeded, total, _, _)| *seeded > 0 && *total > 0)
            .map(|(_, _, _, _, _, _, _, _, puzzle)| puzzle)
            .unwrap_or_else(|| seeded_fallback_puzzle(&data, shape, seed))
    }

    #[cfg(test)]
    pub(super) fn puzzle_from_rows(
        data: Arc<WordHuntData>,
        rows: Vec<String>,
    ) -> Result<Puzzle, String> {
        let Some(cols) = rows.first().map(String::len) else {
            return Err("board must not be empty".to_string());
        };
        if cols == 0 || rows.iter().any(|row| row.len() != cols) {
            return Err("board rows must have the same width".to_string());
        }
        Ok(solve_rows(&data, rows))
    }

    pub(super) fn puzzle(&self) -> &Puzzle {
        &self.puzzle
    }

    pub(super) fn board_char(&self, coord: Coord) -> Option<char> {
        self.puzzle
            .board
            .get(coord.row)
            .and_then(|row| row.as_bytes().get(coord.col))
            .map(|byte| *byte as char)
    }

    pub(super) fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    pub(super) fn selected_path(&self) -> Vec<Coord> {
        self.selection
            .as_ref()
            .map(|selection| selection.coords.clone())
            .unwrap_or_default()
    }

    pub(super) fn selected_word(&self) -> String {
        self.selection
            .as_ref()
            .map(|selection| self.word_from_path(&selection.coords))
            .unwrap_or_default()
    }

    pub(super) fn found_count(&self) -> usize {
        self.found.len()
    }

    pub(super) fn subword_count(&self) -> usize {
        self.subwords.len()
    }

    pub(super) fn revealed_count(&self) -> usize {
        self.revealed
            .keys()
            .filter(|word| !self.found.contains_key(*word))
            .count()
    }

    pub(super) fn total_count(&self) -> usize {
        self.puzzle.words.len()
    }

    pub(super) fn is_complete(&self) -> bool {
        self.total_count() > 0 && self.found.len() == self.total_count()
    }

    pub(super) fn is_resumable(&self) -> bool {
        !self.found.is_empty() && !self.is_complete()
    }

    pub(super) fn restore_saved(&mut self) -> bool {
        let Some(saved) = Self::saved(Arc::clone(&self.data)) else {
            return false;
        };
        *self = saved;
        true
    }

    pub(super) fn continue_board(&mut self, shape: BoardShape, seed: u64) {
        self.puzzle = Self::generate_puzzle(
            Arc::clone(&self.data),
            shape,
            target_word_count(shape.normalized()),
            seed,
        );
        self.found.clear();
        self.revealed.clear();
        self.subwords.clear();
        self.selection = None;
    }

    pub(super) fn is_found(&self, word: &str) -> bool {
        self.found.contains_key(word)
    }

    pub(super) fn is_revealed(&self, word: &str) -> bool {
        self.revealed.contains_key(word)
    }

    pub(super) fn found_cells(&self) -> BTreeSet<Coord> {
        self.found
            .values()
            .flat_map(|path| path.iter().copied())
            .collect()
    }

    pub(super) fn found_paths(&self) -> Vec<Vec<Coord>> {
        self.found.values().cloned().collect()
    }

    pub(super) fn revealed_cells(&self) -> BTreeSet<Coord> {
        self.revealed
            .values()
            .flat_map(|path| path.iter().copied())
            .collect()
    }

    pub(super) fn revealed_paths(&self) -> Vec<Vec<Coord>> {
        self.revealed
            .iter()
            .filter(|(word, _)| !self.found.contains_key(*word))
            .map(|(_, path)| path.clone())
            .collect()
    }

    pub(super) fn subword_paths(&self) -> Vec<Vec<Coord>> {
        self.subwords
            .values()
            .filter(|path| {
                !self
                    .found
                    .values()
                    .any(|found_path| path_is_contiguous_subset(path, found_path))
            })
            .cloned()
            .collect()
    }

    pub(super) fn subwords(&self) -> Vec<String> {
        self.subwords.keys().cloned().collect()
    }

    pub(super) fn definitions_for(&self, word: &str) -> Vec<Definition> {
        self.data.definitions_for(word)
    }

    pub(super) fn definitions_loaded(&self) -> bool {
        self.data.definitions_loaded()
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn load_definitions(&mut self, definitions: HashMap<String, Vec<Definition>>) {
        Arc::make_mut(&mut self.data).load_definitions(definitions);
    }

    pub(super) fn begin_selection(&mut self, coord: Coord) {
        if !self.in_bounds(coord) {
            return;
        }
        self.selection = Some(Selection {
            start: coord,
            end: coord,
            coords: vec![coord],
        });
    }

    pub(super) fn preview_selection(&mut self, coord: Coord) -> bool {
        if !self.in_bounds(coord) {
            return false;
        }
        let Some(selection) = self.selection.clone() else {
            self.begin_selection(coord);
            return true;
        };
        let Some(coords) = Self::line_between(selection.start, coord) else {
            return false;
        };
        if coords.iter().any(|coord| !self.in_bounds(*coord)) {
            return false;
        }
        self.selection = Some(Selection {
            start: selection.start,
            end: coord,
            coords,
        });
        true
    }

    pub(super) fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub(super) fn retract_selection(&mut self) {
        let Some(selection) = self.selection.clone() else {
            return;
        };
        if selection.coords.len() <= 1 {
            self.selection = None;
            return;
        }
        let end = selection.coords[selection.coords.len() - 2];
        let coords =
            Self::line_between(selection.start, end).unwrap_or_else(|| vec![selection.start]);
        self.selection = Some(Selection {
            start: selection.start,
            end,
            coords,
        });
    }

    pub(super) fn submit_selection(&mut self) -> GuessResult {
        let Some(selection) = self.selection.clone() else {
            return GuessResult::Empty;
        };
        let selected = self.word_from_path(&selection.coords).to_ascii_lowercase();
        if selection.coords.len() < MIN_WORD_LEN {
            self.selection = None;
            return GuessResult::TooShort { word: selected };
        }

        let Some((word, path)) = self.matching_word_for_path(&selection.coords) else {
            self.selection = None;
            if let Some((word, path)) = self.subword_for_path(&selection.coords, &selected) {
                self.subwords.insert(word.clone(), path.clone());
                return GuessResult::Subword { word, path };
            }
            return GuessResult::NotInPuzzle { word: selected };
        };
        let definitions = self.data.definitions_for(&word);

        if self.found.contains_key(&word) {
            self.selection = None;
            return GuessResult::AlreadyFound {
                word,
                path,
                definitions,
            };
        }

        self.found.insert(word.clone(), path.clone());
        self.revealed.remove(&word);
        self.selection = None;

        GuessResult::Accepted {
            word,
            path,
            definitions,
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) fn reveal_all(&mut self) -> RevealResult {
        let mut count = 0;
        for entry in &self.puzzle.words {
            if self.found.contains_key(&entry.word) || self.revealed.contains_key(&entry.word) {
                continue;
            }
            if let Some(path) = entry.paths.first() {
                self.revealed.insert(entry.word.clone(), path.clone());
                count += 1;
            }
        }
        if count == 0 {
            RevealResult::AlreadyComplete
        } else {
            RevealResult::Revealed { count }
        }
    }

    pub(super) fn line_between(start: Coord, end: Coord) -> Option<Vec<Coord>> {
        let row_delta = end.row as isize - start.row as isize;
        let col_delta = end.col as isize - start.col as isize;
        let row_step = row_delta.signum();
        let col_step = col_delta.signum();
        if row_delta == 0 && col_delta == 0 {
            return Some(vec![start]);
        }
        if !(row_delta == 0 || col_delta == 0 || row_delta.abs() == col_delta.abs()) {
            return None;
        }
        let len = row_delta.abs().max(col_delta.abs()) as usize;
        Some(
            (0..=len)
                .map(|index| Coord {
                    row: (start.row as isize + row_step * index as isize) as usize,
                    col: (start.col as isize + col_step * index as isize) as usize,
                })
                .collect(),
        )
    }

    fn in_bounds(&self, coord: Coord) -> bool {
        coord.row < self.puzzle.rows() && coord.col < self.puzzle.cols()
    }

    fn word_from_path(&self, path: &[Coord]) -> String {
        path.iter()
            .filter_map(|coord| self.board_char(*coord))
            .collect()
    }

    fn matching_word_for_path(&self, path: &[Coord]) -> Option<(String, Vec<Coord>)> {
        self.puzzle.words.iter().find_map(|entry| {
            entry.paths.iter().find_map(|candidate| {
                if candidate == path {
                    Some((entry.word.clone(), candidate.clone()))
                } else {
                    None
                }
            })
        })
    }

    fn subword_for_path(&self, path: &[Coord], selected: &str) -> Option<(String, Vec<Coord>)> {
        if selected.len() < MIN_WORD_LEN || !self.data.is_word(selected) {
            return None;
        }
        self.puzzle.words.iter().find_map(|entry| {
            if entry.word.len() <= selected.len() {
                return None;
            }
            entry
                .paths
                .iter()
                .any(|candidate| path_is_contiguous_subset(path, candidate))
                .then(|| (selected.to_string(), path.to_vec()))
        })
    }
}

fn solve_rows(data: &WordHuntData, rows: Vec<String>) -> Puzzle {
    let row_count = rows.len();
    let col_count = rows.first().map(String::len).unwrap_or(0);
    let board = rows
        .iter()
        .map(|row| row.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut found = BTreeMap::<String, Vec<Vec<Coord>>>::new();
    let max_len = data.max_scan_len(row_count.max(col_count));

    for row in 0..row_count {
        for col in 0..col_count {
            for (dr, dc) in DIRECTIONS {
                let mut word = String::new();
                let mut path = Vec::new();
                for step in 0..max_len {
                    let Some(next_row) = (row as isize).checked_add(dr * step as isize) else {
                        break;
                    };
                    let Some(next_col) = (col as isize).checked_add(dc * step as isize) else {
                        break;
                    };
                    if next_row < 0
                        || next_col < 0
                        || next_row as usize >= row_count
                        || next_col as usize >= col_count
                    {
                        break;
                    }
                    let coord = Coord::new(next_row as usize, next_col as usize);
                    word.push(board[coord.row][coord.col].to_ascii_lowercase());
                    path.push(coord);
                    if word.len() >= MIN_WORD_LEN && data.is_word(&word) {
                        found.entry(word.clone()).or_default().push(path.clone());
                    }
                }
            }
        }
    }

    let mut words = found
        .into_iter()
        .map(|(word, mut paths)| {
            paths.sort();
            paths.dedup();
            BoardWord { word, paths }
        })
        .collect::<Vec<_>>();
    remove_strict_subset_paths(&mut words);

    Puzzle {
        size: row_count.max(col_count),
        rows: row_count,
        cols: col_count,
        board: rows,
        words,
    }
}

fn validate_puzzle(data: &WordHuntData, puzzle: Puzzle) -> Option<Puzzle> {
    let rows = puzzle.rows();
    let cols = puzzle.cols();
    if rows < 4 || cols < 4 || puzzle.board.len() != rows {
        return None;
    }
    if puzzle
        .board
        .iter()
        .any(|row| row.len() != cols || !row.chars().all(|ch| ch.is_ascii_uppercase()))
    {
        return None;
    }
    let solved = solve_rows(data, puzzle.board.clone());
    if solved.words.is_empty() {
        return None;
    }
    Some(solved)
}

fn valid_selection_for_puzzle(puzzle: &Puzzle, selection: &Selection) -> bool {
    WordHuntState::line_between(selection.start, selection.end)
        .is_some_and(|coords| coords == selection.coords)
        && selection
            .coords
            .iter()
            .all(|coord| coord.row < puzzle.rows() && coord.col < puzzle.cols())
}

fn path_belongs_to_word(puzzle: &Puzzle, word: &str, path: &[Coord]) -> bool {
    puzzle
        .words
        .iter()
        .find(|entry| entry.word == word)
        .is_some_and(|entry| entry.paths.iter().any(|candidate| candidate == path))
}

fn subword_belongs_to_puzzle(
    data: &WordHuntData,
    puzzle: &Puzzle,
    word: &str,
    path: &[Coord],
) -> bool {
    word.len() >= MIN_WORD_LEN
        && data.is_word(word)
        && word_from_puzzle_path(puzzle, path).as_deref() == Some(word)
        && puzzle.words.iter().any(|entry| {
            entry.word.len() > word.len()
                && entry
                    .paths
                    .iter()
                    .any(|candidate| path_is_contiguous_subset(path, candidate))
        })
}

fn word_from_puzzle_path(puzzle: &Puzzle, path: &[Coord]) -> Option<String> {
    path.iter()
        .map(|coord| {
            puzzle
                .board
                .get(coord.row)
                .and_then(|row| row.as_bytes().get(coord.col))
                .map(|byte| (*byte as char).to_ascii_lowercase())
        })
        .collect()
}

fn target_word_count(shape: BoardShape) -> usize {
    (shape.area() / 3).clamp(20, 56)
}

fn max_seed_word_len(shape: BoardShape) -> usize {
    shape
        .max_line_len()
        .min(MAX_SEED_WORD_LEN)
        .clamp(6, MAX_SEED_WORD_LEN)
}

fn seeded_placement_word_count(
    shape: BoardShape,
    target_words: usize,
    available_words: usize,
) -> usize {
    if available_words == 0 {
        return 0;
    }
    let minimum = minimum_seeded_word_count(shape, target_words, available_words);
    ((target_words * 3 + 3) / 4)
        .clamp(minimum, 40)
        .min((shape.area() / 4).max(minimum))
        .min(available_words)
}

fn minimum_seeded_word_count(
    shape: BoardShape,
    target_words: usize,
    available_words: usize,
) -> usize {
    if available_words == 0 {
        return 0;
    }
    (target_words / 3)
        .clamp(4, 10)
        .min(shape.min_side())
        .min(available_words)
}

fn seeded_answer_count(puzzle: &Puzzle, placed_words: &BTreeSet<String>) -> usize {
    puzzle
        .words
        .iter()
        .filter(|entry| placed_words.contains(&entry.word))
        .count()
}

fn long_seeded_answer_count(puzzle: &Puzzle, placed_words: &BTreeSet<String>) -> usize {
    puzzle
        .words
        .iter()
        .filter(|entry| placed_words.contains(&entry.word) && is_long_word(&entry.word))
        .count()
}

fn non_short_word_count(puzzle: &Puzzle) -> usize {
    puzzle
        .words
        .iter()
        .filter(|entry| entry.word.len() > MIN_WORD_LEN)
        .count()
}

fn diagonal_word_count(puzzle: &Puzzle) -> usize {
    puzzle
        .words
        .iter()
        .filter(|entry| entry.paths.iter().any(|path| is_diagonal_path(path)))
        .count()
}

fn is_diagonal_path(path: &[Coord]) -> bool {
    let (Some(first), Some(last)) = (path.first(), path.last()) else {
        return false;
    };
    first.row != last.row && first.col != last.col
}

fn minimum_diagonal_word_count(_shape: BoardShape, placement_target: usize) -> usize {
    ((placement_target * 2 + 2) / 3)
        .clamp(4, 14)
        .min(placement_target)
}

fn required_diagonal_answer_count(minimum_diagonal_words: usize, total_answers: usize) -> usize {
    if total_answers == 0 {
        return 0;
    }
    minimum_diagonal_words
        .max((total_answers + 4) / 5)
        .min(total_answers)
}

fn dedup_preserving_order(words: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    words.retain(|word| seen.insert(word.clone()));
}

fn remove_strict_word_subsets(words: &mut Vec<String>) {
    let original = words.clone();
    words.retain(|word| {
        !original
            .iter()
            .any(|other| other.len() > word.len() && other.contains(word.as_str()))
    });
}

fn is_long_word(word: &str) -> bool {
    word.len() >= LONG_WORD_LEN
}

fn seed_length_allocation(target: usize, max_len: usize, seed_words: &[&str]) -> Vec<usize> {
    if target == 0 || seed_words.is_empty() {
        return Vec::new();
    }

    let mut available_lengths = seed_length_weights(max_len)
        .into_iter()
        .filter(|(len, weight)| *weight > 0 && seed_words.iter().any(|word| word.len() == *len))
        .collect::<Vec<_>>();
    if available_lengths.is_empty() && seed_words.iter().any(|word| word.len() == MIN_WORD_LEN) {
        available_lengths.push((MIN_WORD_LEN, 1));
    }
    if available_lengths.is_empty() {
        return Vec::new();
    }

    let total_weight = available_lengths
        .iter()
        .map(|(_, weight)| *weight)
        .sum::<usize>();
    let mut allocations = available_lengths
        .iter()
        .map(|(len, weight)| {
            let scaled = target * *weight;
            (*len, scaled / total_weight, scaled % total_weight)
        })
        .collect::<Vec<_>>();

    let mut used = allocations
        .iter()
        .map(|(_, count, _)| *count)
        .sum::<usize>();
    let mut remainder_order = allocations
        .iter()
        .enumerate()
        .map(|(index, (len, _, remainder))| (index, *len, *remainder))
        .collect::<Vec<_>>();
    remainder_order.sort_by(|(_, len_a, rem_a), (_, len_b, rem_b)| {
        rem_b.cmp(rem_a).then_with(|| len_b.cmp(len_a))
    });

    let mut cursor = 0;
    while used < target {
        let index = remainder_order[cursor % remainder_order.len()].0;
        allocations[index].1 += 1;
        used += 1;
        cursor += 1;
    }

    allocations
        .into_iter()
        .flat_map(|(len, count, _)| std::iter::repeat(len).take(count))
        .collect()
}

fn seed_length_weights(max_len: usize) -> Vec<(usize, usize)> {
    (MIN_WORD_LEN..=max_len)
        .map(|len| {
            let weight = SHORT_SEED_LENGTH_WEIGHTS
                .iter()
                .find_map(|(candidate, weight)| (*candidate == len).then_some(*weight))
                .unwrap_or(1);
            (len, weight)
        })
        .collect()
}

fn seed_candidates_for_lengths(
    seed_words: &[&str],
    length_plan: &[usize],
    rng: &mut Lcg,
) -> Vec<String> {
    if length_plan.is_empty() {
        return Vec::new();
    }

    let max_len = length_plan.iter().copied().max().unwrap_or(0);
    let mut buckets = vec![Vec::<&str>::new(); max_len + 1];
    for word in seed_words {
        if word.len() <= max_len {
            buckets[word.len()].push(*word);
        }
    }
    let mut candidates = Vec::with_capacity(length_plan.len() * 4);
    let mut lengths = length_plan.to_vec();
    shuffle_lengths(&mut lengths, rng);
    for len in &lengths {
        if let Some(word) = choose_seed_word_of_len(&buckets, *len, rng) {
            candidates.push(word);
        }
    }
    for _ in 0..3 {
        let mut alternates = length_plan.to_vec();
        shuffle_lengths(&mut alternates, rng);
        for len in alternates {
            if let Some(word) = choose_seed_word_of_len(&buckets, len, rng) {
                candidates.push(word);
            }
        }
    }
    candidates
}

fn shuffle_lengths(lengths: &mut [usize], rng: &mut Lcg) {
    for index in (1..lengths.len()).rev() {
        let other = rng.range(index + 1);
        lengths.swap(index, other);
    }
}

fn remove_strict_subset_paths(words: &mut Vec<BoardWord>) {
    let longer_paths = words
        .iter()
        .flat_map(|entry| {
            entry
                .paths
                .iter()
                .map(|path| (entry.word.len(), path.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for entry in words.iter_mut() {
        let word_len = entry.word.len();
        entry.paths.retain(|path| {
            !longer_paths.iter().any(|(other_len, other_path)| {
                *other_len > word_len && path_is_contiguous_subset(path, other_path)
            })
        });
    }
    words.retain(|entry| !entry.paths.is_empty());
}

fn path_is_contiguous_subset(path: &[Coord], other: &[Coord]) -> bool {
    path.len() < other.len() && other.windows(path.len()).any(|window| window == path)
}

fn choose_seed_word_of_len(buckets: &[Vec<&str>], len: usize, rng: &mut Lcg) -> Option<String> {
    let bucket = buckets.get(len)?;
    if bucket.is_empty() {
        return None;
    }
    bucket
        .get(rng.range(bucket.len()))
        .map(|word| (*word).to_string())
}

fn seeded_fallback_puzzle(data: &WordHuntData, shape: BoardShape, seed: u64) -> Puzzle {
    let mut rng = Lcg::new(seed ^ 0xa11c_e5eed);
    let mut board = vec![vec![None::<char>; shape.cols]; shape.rows];
    let seed_words = data.seed_words_for_size(max_seed_word_len(shape));
    let target =
        minimum_seeded_word_count(shape, target_word_count(shape), seed_words.len()).max(1);
    let length_plan = seed_length_allocation(target, max_seed_word_len(shape), &seed_words);
    let mut candidates = seed_candidates_for_lengths(&seed_words, &length_plan, &mut rng);
    dedup_preserving_order(&mut candidates);
    remove_strict_word_subsets(&mut candidates);

    for word in candidates.into_iter().take(target) {
        if place_word(&mut board, &word, &mut rng, true).is_none() {
            let row = rng.range(shape.rows);
            for (col, ch) in word
                .chars()
                .take(shape.cols)
                .map(|ch| ch.to_ascii_uppercase())
                .enumerate()
            {
                board[row][col] = Some(ch);
            }
        }
    }

    fill_board(&mut board, data.letter_weights(), &mut rng);
    let rows = board
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|ch| ch.unwrap_or('E'))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let puzzle = solve_rows(data, rows);
    if puzzle.words.is_empty() {
        solve_rows(data, fallback_rows(shape))
    } else {
        puzzle
    }
}

fn place_word(
    board: &mut [Vec<Option<char>>],
    word: &str,
    rng: &mut Lcg,
    prefer_diagonal: bool,
) -> Option<bool> {
    let rows = board.len();
    let cols = board.first().map(Vec::len).unwrap_or(0);
    let letters = word
        .chars()
        .map(|ch| ch.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let diagonal_possible = letters.len() <= rows.min(cols);

    for attempt in 0..WORD_PLACEMENT_TRIES {
        let use_diagonal = diagonal_possible
            && ((prefer_diagonal && attempt < WORD_PLACEMENT_TRIES * 9 / 10)
                || (!prefer_diagonal && rng.range(100) < 58));
        let (dr, dc) = if use_diagonal {
            DIAGONAL_DIRECTIONS[rng.range(DIAGONAL_DIRECTIONS.len())]
        } else {
            DIRECTIONS[rng.range(DIRECTIONS.len())]
        };
        let row = rng.range(rows);
        let col = rng.range(cols);
        if can_place(board, &letters, row, col, dr, dc) {
            for (index, ch) in letters.iter().enumerate() {
                let next_row = (row as isize + dr * index as isize) as usize;
                let next_col = (col as isize + dc * index as isize) as usize;
                board[next_row][next_col] = Some(*ch);
            }
            return Some(dr != 0 && dc != 0);
        }
    }

    None
}

fn can_place(
    board: &[Vec<Option<char>>],
    letters: &[char],
    row: usize,
    col: usize,
    dr: isize,
    dc: isize,
) -> bool {
    let rows = board.len() as isize;
    let cols = board.first().map(Vec::len).unwrap_or(0) as isize;
    for (index, ch) in letters.iter().enumerate() {
        let next_row = row as isize + dr * index as isize;
        let next_col = col as isize + dc * index as isize;
        if next_row < 0 || next_col < 0 || next_row >= rows || next_col >= cols {
            return false;
        }
        if board[next_row as usize][next_col as usize].is_some_and(|existing| existing != *ch) {
            return false;
        }
    }
    true
}

fn fill_board(board: &mut [Vec<Option<char>>], weights: &[u32; 26], rng: &mut Lcg) {
    for row in board {
        for cell in row {
            if cell.is_none() {
                *cell = Some(weighted_letter(weights, rng));
            }
        }
    }
}

fn weighted_letter(weights: &[u32; 26], rng: &mut Lcg) -> char {
    let total = weights.iter().sum::<u32>().max(1);
    let mut target = rng.range(total as usize) as u32;
    for (index, weight) in weights.iter().enumerate() {
        if target < *weight {
            return (b'A' + index as u8) as char;
        }
        target -= *weight;
    }
    'E'
}

fn fallback_rows(shape: BoardShape) -> Vec<String> {
    let pattern = b"ETAOINSHRDLUCMFYWGPBVKXQJZ";
    (0..shape.rows)
        .map(|row| {
            (0..shape.cols)
                .map(|col| pattern[(row * shape.cols + col) % pattern.len()] as char)
                .collect()
        })
        .collect()
}

#[derive(Clone, Debug)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next() as usize) % upper
    }
}
