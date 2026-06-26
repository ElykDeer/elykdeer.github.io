use std::collections::{BTreeSet, VecDeque};

pub const ROWS: usize = 13;
pub const COLS: usize = 12;
pub const COLORS: [BubbleColor; 6] = [
    BubbleColor::Rose,
    BubbleColor::Gold,
    BubbleColor::Sky,
    BubbleColor::Mint,
    BubbleColor::Violet,
    BubbleColor::Coral,
];
const SHOTS_PER_DROP: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BubbleColor {
    Rose,
    Gold,
    Sky,
    Mint,
    Violet,
    Coral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellCoord {
    pub row: usize,
    pub col: usize,
}

impl CellCoord {
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    cells: [[Option<BubbleColor>; COLS]; ROWS],
    /// Parity offset for the hex stagger. Toggled whenever a ceiling row is
    /// pushed so existing bubbles keep their horizontal position as they move
    /// down instead of jittering left/right each drop.
    parity: usize,
}

impl Board {
    pub fn empty() -> Self {
        Self {
            cells: [[None; COLS]; ROWS],
            parity: 0,
        }
    }

    /// Base stagger parity, toggled on each ceiling push.
    pub fn parity(&self) -> usize {
        self.parity
    }

    /// Effective stagger parity for a row, accounting for ceiling pushes.
    pub fn row_parity(&self, row: usize) -> usize {
        (row + self.parity) % 2
    }

    pub fn seeded(seed: u64, filled_rows: usize) -> Self {
        let mut board = Self::empty();
        let mut rng = Lcg::new(seed);

        for row in 0..filled_rows.min(ROWS) {
            for col in 0..COLS {
                board.cells[row][col] = Some(rng.next_color());
            }
        }

        board
    }

    pub fn get(&self, coord: CellCoord) -> Option<BubbleColor> {
        self.in_bounds(coord).then_some(())?;
        self.cells[coord.row][coord.col]
    }

    pub fn set(&mut self, coord: CellCoord, color: Option<BubbleColor>) -> bool {
        if !self.in_bounds(coord) {
            return false;
        }

        self.cells[coord.row][coord.col] = color;
        true
    }

    pub fn in_bounds(&self, coord: CellCoord) -> bool {
        coord.row < ROWS && coord.col < COLS
    }

    pub fn is_empty(&self, coord: CellCoord) -> bool {
        self.get(coord).is_none()
    }

    pub fn occupied_cells(&self) -> impl Iterator<Item = CellCoord> + '_ {
        (0..ROWS).flat_map(move |row| {
            (0..COLS).filter_map(move |col| {
                self.cells[row][col]
                    .is_some()
                    .then_some(CellCoord { row, col })
            })
        })
    }

    pub fn neighbors(&self, coord: CellCoord) -> Vec<CellCoord> {
        if !self.in_bounds(coord) {
            return Vec::new();
        }

        let offsets_even: &[(isize, isize)] =
            &[(-1, -1), (-1, 0), (0, -1), (0, 1), (1, -1), (1, 0)];
        let offsets_odd: &[(isize, isize)] = &[(-1, 0), (-1, 1), (0, -1), (0, 1), (1, 0), (1, 1)];
        let offsets = if self.row_parity(coord.row) == 0 {
            offsets_even
        } else {
            offsets_odd
        };

        offsets
            .iter()
            .filter_map(|(dr, dc)| {
                let row = coord.row.checked_add_signed(*dr)?;
                let col = coord.col.checked_add_signed(*dc)?;
                let next = CellCoord { row, col };
                self.in_bounds(next).then_some(next)
            })
            .collect()
    }

    pub fn empty_attachment_neighbors(&self, anchor: CellCoord) -> Vec<CellCoord> {
        if self.get(anchor).is_none() {
            return Vec::new();
        }

        self.neighbors(anchor)
            .into_iter()
            .filter(|coord| self.is_empty(*coord))
            .collect()
    }

    pub fn matching_cluster(&self, start: CellCoord) -> BTreeSet<CellCoord> {
        let Some(color) = self.get(start) else {
            return BTreeSet::new();
        };

        let mut cluster = BTreeSet::new();
        let mut pending = VecDeque::from([start]);
        cluster.insert(start);

        while let Some(coord) = pending.pop_front() {
            for neighbor in self.neighbors(coord) {
                if cluster.contains(&neighbor) || self.get(neighbor) != Some(color) {
                    continue;
                }
                cluster.insert(neighbor);
                pending.push_back(neighbor);
            }
        }

        cluster
    }

    pub fn top_connected(&self) -> BTreeSet<CellCoord> {
        let mut connected = BTreeSet::new();
        let mut pending = VecDeque::new();

        for col in 0..COLS {
            let coord = CellCoord { row: 0, col };
            if self.get(coord).is_some() {
                connected.insert(coord);
                pending.push_back(coord);
            }
        }

        while let Some(coord) = pending.pop_front() {
            for neighbor in self.neighbors(coord) {
                if connected.contains(&neighbor) || self.get(neighbor).is_none() {
                    continue;
                }
                connected.insert(neighbor);
                pending.push_back(neighbor);
            }
        }

        connected
    }

    pub fn resolve_after_attach(&mut self, attached: CellCoord) -> Resolution {
        let mut resolution = Resolution::default();
        let cluster = self.matching_cluster(attached);

        if cluster.len() >= 3 {
            resolution.popped = self.clear_cells(cluster);

            let anchored = self.top_connected();
            let disconnected = self
                .occupied_cells()
                .filter(|coord| !anchored.contains(coord))
                .collect::<Vec<_>>();
            resolution.dropped = self.clear_cells(disconnected);
        }

        resolution
    }

    /// Clear the given cells, returning each with the color it held — the
    /// renderer uses the colors to animate the pop and the falling debris.
    fn clear_cells(
        &mut self,
        cells: impl IntoIterator<Item = CellCoord>,
    ) -> Vec<(CellCoord, BubbleColor)> {
        cells
            .into_iter()
            .filter_map(|coord| {
                let color = self.get(coord)?;
                self.set(coord, None);
                Some((coord, color))
            })
            .collect()
    }

    pub fn push_ceiling_row(&mut self, colors: &[BubbleColor; COLS]) -> bool {
        let overflow = self.cells[ROWS - 1].iter().any(Option::is_some);

        for row in (1..ROWS).rev() {
            self.cells[row] = self.cells[row - 1];
        }
        self.cells[0] = colors.map(Some);
        self.parity ^= 1;

        overflow
    }

    pub fn is_cleared(&self) -> bool {
        self.occupied_cells().next().is_none()
    }

    pub fn active_colors(&self) -> Vec<BubbleColor> {
        COLORS
            .iter()
            .copied()
            .filter(|color| {
                self.cells
                    .iter()
                    .flatten()
                    .any(|cell| cell.as_ref() == Some(color))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    pub popped: Vec<(CellCoord, BubbleColor)>,
    pub dropped: Vec<(CellCoord, BubbleColor)>,
}

/// Classic Bust-a-Move scoring: pops are 10 each plus a square big-cluster
/// bonus, and each dropped bubble doubles the last (20, 40, 80, ...) so big
/// drops pay off enormously.
fn score_resolution(resolution: &Resolution) -> u64 {
    let n = resolution.popped.len() as u64;
    let cluster_bonus = (n.saturating_sub(3)).pow(2) * 10;
    let mut score = n * 10 + cluster_bonus;

    for k in 1..=resolution.dropped.len() as u32 {
        score = score.saturating_add(10u64.saturating_mul(1u64 << k.min(63)));
    }

    score
}

#[derive(Clone, Debug)]
pub struct BubbleGame {
    pub board: Board,
    pub score: u64,
    pub high_score: u64,
    pub shots_until_drop: u32,
    pub game_over: bool,
    pub held: Option<BubbleColor>,
    rng: Lcg,
    queue: VecDeque<BubbleColor>,
}

impl BubbleGame {
    pub fn new(seed: u64, high_score: u64) -> Self {
        let board = Board::seeded(seed ^ 0xa53a_91c7, 5);
        let mut rng = Lcg::new(seed);
        let mut queue = VecDeque::new();
        let active_colors = board.active_colors();
        for _ in 0..4 {
            queue.push_back(rng.next_color_from(&active_colors));
        }

        Self {
            board,
            score: 0,
            high_score,
            shots_until_drop: SHOTS_PER_DROP,
            game_over: false,
            held: None,
            rng,
            queue,
        }
    }

    pub fn current_color(&self) -> BubbleColor {
        self.queue[0]
    }

    pub fn next_colors(&self) -> [BubbleColor; 3] {
        [self.queue[1], self.queue[2], self.queue[3]]
    }

    /// Stash the current bubble for later, or swap it back with a held one.
    pub fn hold(&mut self) {
        if self.game_over {
            return;
        }
        match self.held {
            Some(held) => {
                self.held = Some(self.queue[0]);
                self.queue[0] = held;
            }
            None => {
                self.held = self.queue.pop_front();
                self.refill_queue();
            }
        }
    }

    /// Pop the fired bubble off the queue and refill so `current_color` and
    /// `next_colors` advance the instant a shot leaves the cannon.
    pub fn pop_next(&mut self) -> BubbleColor {
        let color = self.queue.pop_front().expect("queue is never empty");
        self.refill_queue();
        color
    }

    pub fn attach_current(&mut self, coord: CellCoord) -> Resolution {
        let color = self.current_color();
        self.queue.pop_front();
        self.refill_queue();
        self.attach_color(coord, color)
    }

    /// Place a bubble of the given color and run resolution. The queue is left
    /// alone — the shoot path already advanced it via `pop_next`.
    pub fn attach_color(&mut self, coord: CellCoord, color: BubbleColor) -> Resolution {
        if self.game_over || !self.board.in_bounds(coord) || !self.board.is_empty(coord) {
            return Resolution::default();
        }

        self.board.set(coord, Some(color));

        let resolution = self.board.resolve_after_attach(coord);
        self.score = self.score.saturating_add(score_resolution(&resolution));
        self.high_score = self.high_score.max(self.score);

        if self.board.is_cleared() {
            self.game_over = true;
            return resolution;
        }

        // A shot that pops something doesn't count toward the next row drop.
        if resolution.popped.is_empty() {
            self.advance_ceiling();
        }
        if !self.game_over {
            self.sync_future_bubbles();
        }

        resolution
    }

    pub fn restart(&mut self) {
        let high_score = self.high_score;
        *self = Self::new(self.rng.next_u32() as u64 ^ 0x710d_d15c, high_score);
    }

    fn advance_ceiling(&mut self) {
        if self.shots_until_drop > 1 {
            self.shots_until_drop -= 1;
            return;
        }

        let active_colors = self.board.active_colors();
        let mut colors = [BubbleColor::Rose; COLS];
        for color in &mut colors {
            *color = self.rng.next_color_from(&active_colors);
        }

        let overflow = self.board.push_ceiling_row(&colors);
        self.shots_until_drop = SHOTS_PER_DROP;
        if overflow {
            self.game_over = true;
        }
    }

    fn sync_future_bubbles(&mut self) {
        let active_colors = self.board.active_colors();
        self.queue = self
            .queue
            .iter()
            .copied()
            .filter(|color| active_colors.contains(color))
            .collect();
        self.refill_queue_from(&active_colors);

        if self.held.is_some_and(|held| !active_colors.contains(&held)) {
            self.held = None;
        }
    }

    fn refill_queue(&mut self) {
        let active_colors = self.board.active_colors();
        self.refill_queue_from(&active_colors);
    }

    fn refill_queue_from(&mut self, active_colors: &[BubbleColor]) {
        while self.queue.len() < 4 {
            self.queue
                .push_back(self.rng.next_color_from(active_colors));
        }
    }
}

#[derive(Clone, Debug)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn next_color(&mut self) -> BubbleColor {
        self.next_color_from(&COLORS)
    }

    fn next_color_from(&mut self, active_colors: &[BubbleColor]) -> BubbleColor {
        let palette = if active_colors.is_empty() {
            &COLORS[..]
        } else {
            active_colors
        };
        palette[self.next_u32() as usize % palette.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_three_pops_and_scores() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(game.score, 30);
        assert!(game.board.is_empty(CellCoord::new(0, 0)));
        assert!(game.board.is_empty(CellCoord::new(0, 1)));
        assert!(game.board.is_empty(CellCoord::new(1, 0)));
    }

    #[test]
    fn big_cluster_bonus_is_square() {
        // 5 popped, no drops: 5*10 + (5-3)^2*10 = 50 + 40 = 90.
        let resolution = Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 5],
            dropped: Vec::new(),
        };
        assert_eq!(score_resolution(&resolution), 90);
    }

    #[test]
    fn drops_double_each_bubble() {
        // 3 popped (no cluster bonus) + 3 drops: 30 + 20 + 40 + 80 = 170.
        let resolution = Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 3],
            dropped: vec![(CellCoord::new(0, 0), BubbleColor::Gold); 3],
        };
        assert_eq!(score_resolution(&resolution), 170);
    }

    #[test]
    fn pop_next_advances_queue() {
        let mut game = BubbleGame::new(7, 0);
        let first = game.current_color();
        let second = game.next_colors()[0];

        let fired = game.pop_next();

        assert_eq!(fired, first);
        assert_eq!(game.current_color(), second);
    }

    #[test]
    fn clearing_shot_keeps_queue_indexable() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert!(game.board.is_cleared());
        // Rendering reads these every frame; they must not panic after a clear.
        let _ = game.current_color();
        let _ = game.next_colors();
    }

    #[test]
    fn disconnected_cluster_drops_after_anchor_pop() {
        let mut board = Board::empty();
        board.set(CellCoord::new(0, 0), Some(BubbleColor::Sky));
        board.set(CellCoord::new(0, 1), Some(BubbleColor::Sky));
        board.set(CellCoord::new(1, 0), Some(BubbleColor::Sky));
        board.set(CellCoord::new(2, 0), Some(BubbleColor::Gold));
        board.set(CellCoord::new(2, 1), Some(BubbleColor::Gold));

        let resolution = board.resolve_after_attach(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(resolution.dropped.len(), 2);
        assert!(board.is_empty(CellCoord::new(2, 0)));
        assert!(board.is_empty(CellCoord::new(2, 1)));
    }

    #[test]
    fn non_matching_attachment_remains() {
        let mut board = Board::empty();
        board.set(CellCoord::new(0, 0), Some(BubbleColor::Mint));
        board.set(CellCoord::new(1, 0), Some(BubbleColor::Rose));

        let resolution = board.resolve_after_attach(CellCoord::new(1, 0));

        assert!(resolution.popped.is_empty());
        assert!(resolution.dropped.is_empty());
        assert_eq!(board.get(CellCoord::new(1, 0)), Some(BubbleColor::Rose));
        assert!(board.top_connected().contains(&CellCoord::new(1, 0)));
    }

    #[test]
    fn attachment_neighbors_require_occupied_anchor() {
        let board = Board::empty();

        assert!(board
            .empty_attachment_neighbors(CellCoord::new(1, 1))
            .is_empty());
    }

    #[test]
    fn attachment_neighbors_are_adjacent_only() {
        let mut board = Board::empty();
        let anchor = CellCoord::new(1, 1);
        let occupied_neighbor = CellCoord::new(0, 1);
        board.set(anchor, Some(BubbleColor::Mint));
        board.set(occupied_neighbor, Some(BubbleColor::Gold));

        let candidates = board.empty_attachment_neighbors(anchor);

        assert!(!candidates.contains(&anchor));
        assert!(!candidates.contains(&occupied_neighbor));
        assert!(!candidates.contains(&CellCoord::new(3, 1)));
        assert!(candidates.contains(&CellCoord::new(0, 2)));
        assert!(candidates.contains(&CellCoord::new(1, 0)));
        assert!(candidates.contains(&CellCoord::new(1, 2)));
        assert!(candidates.contains(&CellCoord::new(2, 1)));
        assert!(candidates.contains(&CellCoord::new(2, 2)));
    }

    #[test]
    fn ceiling_push_reports_floor_overflow() {
        let mut board = Board::empty();
        board.set(CellCoord::new(ROWS - 1, 3), Some(BubbleColor::Coral));
        let row = [BubbleColor::Gold; COLS];

        assert!(board.push_ceiling_row(&row));
        assert_eq!(board.get(CellCoord::new(0, 0)), Some(BubbleColor::Gold));
    }

    #[test]
    fn popping_keeps_the_drop_counter() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.shots_until_drop = 5;
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(game.shots_until_drop, 5);
    }

    #[test]
    fn non_pop_advances_the_drop_counter() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Mint,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Rose,
        ]);
        game.shots_until_drop = 5;
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert!(resolution.popped.is_empty());
        assert_eq!(game.shots_until_drop, 4);
    }

    #[test]
    fn drop_pacing_resets_to_five_after_each_ceiling_push() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Gold,
            BubbleColor::Gold,
            BubbleColor::Gold,
            BubbleColor::Gold,
        ]);
        game.shots_until_drop = 1;
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Gold));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert!(resolution.popped.is_empty());
        assert_eq!(game.shots_until_drop, SHOTS_PER_DROP);
        for col in 0..COLS {
            assert_eq!(
                game.board.get(CellCoord::new(0, col)),
                Some(BubbleColor::Gold)
            );
        }
    }

    #[test]
    fn eliminated_colors_are_removed_from_queue_and_hold() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Coral,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.held = Some(BubbleColor::Rose);
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 3), Some(BubbleColor::Gold));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(game.current_color(), BubbleColor::Gold);
        assert_eq!(game.next_colors(), [BubbleColor::Gold; 3]);
        assert_eq!(game.held, None);
    }

    #[test]
    fn hold_stashes_then_swaps_back() {
        let mut game = BubbleGame::new(7, 0);
        let first = game.current_color();

        game.hold();
        assert_eq!(game.held, Some(first));
        let second = game.current_color();

        game.hold();
        assert_eq!(game.current_color(), first);
        assert_eq!(game.held, Some(second));
    }

    #[test]
    fn clearing_board_on_drop_shot_does_not_insert_new_row() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.shots_until_drop = 1;
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert!(game.board.is_cleared());
        assert!(game.game_over);
    }
}
