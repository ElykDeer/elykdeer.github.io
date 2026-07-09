#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

pub(super) const DEFAULT_COLS: usize = 40;
pub(super) const DEFAULT_ROWS: usize = 28;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Cell {
    pub(super) col: usize,
    pub(super) row: usize,
}

impl Cell {
    pub(super) const fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }

    fn wrapped(self, cols: usize, rows: usize) -> Self {
        Self {
            col: self.col % cols.max(1),
            row: self.row % rows.max(1),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SavedSnekGame {
    direction: Direction,
    positions: Vec<Cell>,
    len: usize,
    apple: Cell,
}

#[derive(Clone, Debug)]
pub(super) struct SnekGame {
    cols: usize,
    rows: usize,
    direction: Direction,
    positions: Vec<Cell>,
    len: usize,
    apple: Cell,
    wiggle_t: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Lcg {
    state: u64,
}

impl Lcg {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            state: seed.max(0x9e37_79b9_7f4a_7c15),
        }
    }

    fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.state >> 32) as usize) % max
    }
}

impl SnekGame {
    pub(super) fn new(cols: usize, rows: usize, seed: u64) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let center = Cell::new(cols / 2, rows / 2);
        let mut rng = Lcg::new(seed);
        Self {
            cols,
            rows,
            direction: Direction::Up,
            positions: vec![center; 3],
            len: 3,
            apple: random_cell(cols, rows, &mut rng),
            wiggle_t: 0.0,
        }
    }

    pub(super) fn from_save(save: SavedSnekGame, cols: usize, rows: usize) -> Option<Self> {
        if save.positions.is_empty() || save.len < 3 {
            return None;
        }

        let cols = cols.max(1);
        let rows = rows.max(1);
        let len = save.len.clamp(3, 5_000);
        let mut positions: Vec<_> = save
            .positions
            .into_iter()
            .map(|cell| cell.wrapped(cols, rows))
            .collect();

        while positions.len() < len {
            let tail = positions
                .first()
                .copied()
                .unwrap_or(Cell::new(cols / 2, rows / 2));
            positions.insert(0, tail);
        }
        while positions.len() > len {
            positions.remove(0);
        }

        Some(Self {
            cols,
            rows,
            direction: save.direction,
            positions,
            len,
            apple: save.apple.wrapped(cols, rows),
            wiggle_t: 0.0,
        })
    }

    pub(super) fn save(&self) -> SavedSnekGame {
        SavedSnekGame {
            direction: self.direction,
            positions: self.positions.clone(),
            len: self.len,
            apple: self.apple,
        }
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        for cell in &mut self.positions {
            *cell = cell.wrapped(cols, rows);
        }
        self.apple = self.apple.wrapped(cols, rows);
    }

    pub(super) fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }

    pub(super) fn step(&mut self, rng: &mut Lcg) {
        let head = self.head();
        let next = match self.direction {
            Direction::Up => Cell::new(
                head.col,
                if head.row == 0 {
                    self.rows - 1
                } else {
                    head.row - 1
                },
            ),
            Direction::Down => Cell::new(head.col, (head.row + 1) % self.rows),
            Direction::Left => Cell::new(
                if head.col == 0 {
                    self.cols - 1
                } else {
                    head.col - 1
                },
                head.row,
            ),
            Direction::Right => Cell::new((head.col + 1) % self.cols, head.row),
        };
        self.positions.push(next);

        if next == self.apple {
            self.len += 1;
            self.apple = self.next_apple(rng);
        }

        while self.positions.len() > self.len {
            self.positions.remove(0);
        }

        self.tick_wiggle();
    }

    pub(super) fn score(&self) -> usize {
        self.len.saturating_sub(3)
    }

    pub(super) fn positions(&self) -> &[Cell] {
        &self.positions
    }

    pub(super) fn apple(&self) -> Cell {
        self.apple
    }

    pub(super) fn wiggle_t(&self) -> f64 {
        self.wiggle_t
    }

    fn head(&self) -> Cell {
        self.positions
            .last()
            .copied()
            .unwrap_or(Cell::new(self.cols / 2, self.rows / 2))
    }

    fn next_apple(&self, rng: &mut Lcg) -> Cell {
        for _ in 0..128 {
            let cell = random_cell(self.cols, self.rows, rng);
            if !self.positions.contains(&cell) {
                return cell;
            }
        }
        random_cell(self.cols, self.rows, rng)
    }

    fn tick_wiggle(&mut self) {
        self.wiggle_t += TAU / 9.0;
        self.wiggle_t %= TAU;
    }
}

fn random_cell(cols: usize, rows: usize, rng: &mut Lcg) -> Cell {
    Cell::new(rng.next_usize(cols.max(1)), rng.next_usize(rows.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_edges() {
        let mut game = SnekGame::new(5, 5, 1);
        game.positions = vec![Cell::new(2, 0); 3];
        game.set_direction(Direction::Up);
        game.step(&mut Lcg::new(2));

        assert_eq!(game.positions().last(), Some(&Cell::new(2, 4)));

        game.set_direction(Direction::Right);
        game.positions = vec![Cell::new(4, 3); 3];
        game.step(&mut Lcg::new(3));

        assert_eq!(game.positions().last(), Some(&Cell::new(0, 3)));
    }

    #[test]
    fn apple_grows_score() {
        let mut game = SnekGame::new(8, 8, 1);
        game.positions = vec![Cell::new(4, 4); 3];
        game.apple = Cell::new(4, 3);
        game.set_direction(Direction::Up);
        game.step(&mut Lcg::new(2));

        assert_eq!(game.score(), 1);
        assert_eq!(game.positions().len(), 4);
    }

    #[test]
    fn saved_game_resizes_into_current_grid() {
        let save = SavedSnekGame {
            direction: Direction::Left,
            positions: vec![Cell::new(10, 10), Cell::new(11, 10), Cell::new(12, 10)],
            len: 3,
            apple: Cell::new(12, 12),
        };
        let game = SnekGame::from_save(save, 5, 5).unwrap();

        assert_eq!(
            game.positions(),
            &[Cell::new(0, 0), Cell::new(1, 0), Cell::new(2, 0)]
        );
        assert_eq!(game.apple(), Cell::new(2, 2));
    }
}
