use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{de::Error, Deserialize, Deserializer, Serialize};

pub const ROWS: usize = 15;
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
const HARD_SHOTS_PER_DROP: u32 = 4;
const LIVE_SAVE_VERSION: u8 = 1;
const CURRENT_SAVE_VERSION: u8 = 2;
const BOMB_BLAST_RADIUS: usize = 2;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum BubbleColor {
    Rose,
    Gold,
    Sky,
    Mint,
    Violet,
    Coral,
    Wild,
    Prize,
    Bomb,
    Lightning,
    Drill,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CellCoord {
    pub row: usize,
    pub col: usize,
}

impl CellCoord {
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Board {
    cells: [[Option<BubbleColor>; COLS]; ROWS],
    /// Parity offset for the hex stagger. Toggled whenever a ceiling row is
    /// pushed so existing bubbles keep their horizontal position as they move
    /// down instead of jittering left/right each drop.
    parity: usize,
}

#[derive(Deserialize)]
struct SavedBoardCells {
    cells: Vec<Vec<Option<BubbleColor>>>,
    #[serde(default)]
    parity: usize,
}

impl<'de> Deserialize<'de> for Board {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let saved = SavedBoardCells::deserialize(deserializer)?;
        if saved.cells.len() > ROWS {
            return Err(D::Error::custom("saved Bubbles board has too many rows"));
        }

        let mut board = Self::empty();
        for (row, saved_row) in saved.cells.into_iter().enumerate() {
            if saved_row.len() > COLS {
                return Err(D::Error::custom("saved Bubbles board has too many columns"));
            }
            for (col, color) in saved_row.into_iter().enumerate() {
                board.cells[row][col] = color;
            }
        }
        board.parity = saved.parity;
        Ok(board)
    }
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

        if color == BubbleColor::Bomb || color == BubbleColor::Drill {
            return BTreeSet::from([start]);
        }
        if color == BubbleColor::Wild || color == BubbleColor::Prize {
            return COLORS
                .iter()
                .map(|color| self.matching_cluster_for(start, *color))
                .max_by_key(BTreeSet::len)
                .unwrap_or_default();
        }

        self.matching_cluster_for(start, color)
    }

    fn matching_cluster_for(&self, start: CellCoord, color: BubbleColor) -> BTreeSet<CellCoord> {
        let mut cluster = BTreeSet::new();
        let mut pending = VecDeque::from([start]);
        cluster.insert(start);

        while let Some(coord) = pending.pop_front() {
            for neighbor in self.neighbors(coord) {
                let Some(neighbor_color) = self.get(neighbor) else {
                    continue;
                };
                if cluster.contains(&neighbor) || !color_matches(neighbor_color, color) {
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
        self.resolve_after_attach_with_bomb_radius(attached, BOMB_BLAST_RADIUS)
    }

    pub fn resolve_after_attach_with_bomb_radius(
        &mut self,
        attached: CellCoord,
        bomb_radius: usize,
    ) -> Resolution {
        self.resolve_after_attach_with_options(attached, bomb_radius, false, 0, 5)
    }

    pub fn resolve_after_attach_with_options(
        &mut self,
        attached: CellCoord,
        bomb_radius: usize,
        multi_wild: bool,
        lightning_power: u8,
        drill_depth: usize,
    ) -> Resolution {
        let mut resolution = Resolution::default();
        if self.get(attached) == Some(BubbleColor::Lightning) {
            resolution.popped =
                self.clear_cells(self.lightning_chain_from(attached, lightning_power));

            let anchored = self.top_connected();
            let disconnected = self
                .occupied_cells()
                .filter(|coord| !anchored.contains(coord))
                .collect::<Vec<_>>();
            resolution.dropped = self.clear_cells(disconnected);
            return resolution;
        }
        if self.get(attached) == Some(BubbleColor::Drill) {
            let tunnel = self.drill_column_from(attached, drill_depth);
            resolution.popped = self.clear_cells(tunnel);

            let anchored = self.top_connected();
            let disconnected = self
                .occupied_cells()
                .filter(|coord| !anchored.contains(coord))
                .collect::<Vec<_>>();
            resolution.dropped = self.clear_cells(disconnected);
            return resolution;
        }
        if self.get(attached) == Some(BubbleColor::Bomb) {
            let blast = self.bomb_blast_from(BTreeSet::from([attached]), bomb_radius);
            resolution.popped = self.clear_cells(blast);

            let anchored = self.top_connected();
            let disconnected = self
                .occupied_cells()
                .filter(|coord| !anchored.contains(coord))
                .collect::<Vec<_>>();
            resolution.dropped = self.clear_cells(disconnected);
            return resolution;
        }

        let cluster = if multi_wild && self.get(attached) == Some(BubbleColor::Wild) {
            self.multi_wild_cluster(attached)
        } else {
            self.matching_cluster(attached)
        };

        if cluster.len() >= 3 {
            let popped_cells = self.expand_adjacent_bombs(cluster, bomb_radius);
            resolution.popped = self.clear_cells(popped_cells);

            let anchored = self.top_connected();
            let disconnected = self
                .occupied_cells()
                .filter(|coord| !anchored.contains(coord))
                .collect::<Vec<_>>();
            resolution.dropped = self.clear_cells(disconnected);
        }

        resolution
    }

    pub fn resolve_drill_cells(
        &mut self,
        cells: impl IntoIterator<Item = CellCoord>,
    ) -> Resolution {
        let mut resolution = Resolution::default();
        let cells = cells
            .into_iter()
            .filter(|coord| self.in_bounds(*coord) && self.get(*coord).is_some())
            .collect::<BTreeSet<_>>();
        resolution.popped = self.clear_cells(cells);

        let anchored = self.top_connected();
        let disconnected = self
            .occupied_cells()
            .filter(|coord| !anchored.contains(coord))
            .collect::<Vec<_>>();
        resolution.dropped = self.clear_cells(disconnected);
        resolution
    }

    fn multi_wild_cluster(&self, attached: CellCoord) -> BTreeSet<CellCoord> {
        let mut cells = BTreeSet::from([attached]);
        for color in COLORS {
            let cluster = self.matching_cluster_for(attached, color);
            if cluster.len() >= 3 {
                cells.extend(cluster);
            }
        }
        cells
    }

    fn expand_adjacent_bombs(
        &self,
        mut cells: BTreeSet<CellCoord>,
        bomb_radius: usize,
    ) -> BTreeSet<CellCoord> {
        let adjacent_bombs = cells
            .iter()
            .flat_map(|coord| self.neighbors(*coord))
            .filter(|coord| self.get(*coord) == Some(BubbleColor::Bomb))
            .collect::<BTreeSet<_>>();
        cells.extend(self.bomb_blast_from(adjacent_bombs, bomb_radius));
        cells
    }

    fn bomb_blast_from(&self, seeds: BTreeSet<CellCoord>, radius: usize) -> BTreeSet<CellCoord> {
        let mut blast = BTreeSet::new();
        let mut pending = seeds.into_iter().collect::<VecDeque<_>>();
        let mut processed_bombs = BTreeSet::new();

        while let Some(coord) = pending.pop_front() {
            if !processed_bombs.insert(coord) {
                continue;
            }

            for blast_coord in self.cells_within_radius(coord, radius) {
                let Some(color) = self.get(blast_coord) else {
                    continue;
                };
                blast.insert(blast_coord);
                if color == BubbleColor::Bomb && !processed_bombs.contains(&blast_coord) {
                    pending.push_back(blast_coord);
                }
            }
        }

        blast
    }

    fn lightning_chain_from(&self, start: CellCoord, lightning_power: u8) -> BTreeSet<CellCoord> {
        if self.get(start).is_none() {
            return BTreeSet::new();
        }

        let parents = self.connected_parents_from(start);
        let mut best_coord = start;
        let mut best_top = start.row;
        let mut best_path_len = usize::MAX;
        let mut best_column_len = 0usize;

        for coord in parents.keys().copied() {
            let upward = self.lightning_up_column_from(coord);
            let top = upward
                .iter()
                .map(|coord| coord.row)
                .min()
                .unwrap_or(coord.row);
            let path_len = lightning_path_len(coord, &parents);
            let column_len = upward.len();

            let improves_reached_top = top < best_top;
            let ties_reached_top = top == best_top;
            let reached_above_start = best_top < start.row;
            let better_same_top = if reached_above_start {
                column_len > best_column_len
                    || (column_len == best_column_len && path_len < best_path_len)
            } else {
                path_len < best_path_len
            };

            if improves_reached_top || (ties_reached_top && better_same_top) {
                best_coord = coord;
                best_top = top;
                best_path_len = path_len;
                best_column_len = column_len;
            }
        }

        let mut cells = lightning_path_to(best_coord, &parents);
        cells.extend(self.lightning_up_column_from(best_coord));
        if lightning_power >= 1 {
            let extra = cells
                .iter()
                .copied()
                .flat_map(|coord| self.neighbors(coord))
                .filter(|coord| self.get(*coord).is_some())
                .collect::<Vec<_>>();
            cells.extend(extra);
        }
        if lightning_power >= 2 {
            let beam_radius = if lightning_power >= 3 { 3 } else { 2 };
            let start_col = best_coord.col.saturating_sub(beam_radius);
            let end_col = (best_coord.col + beam_radius).min(COLS - 1);
            for row in best_top..=best_coord.row {
                for col in start_col..=end_col {
                    let coord = CellCoord::new(row, col);
                    if parents.contains_key(&coord) {
                        cells.insert(coord);
                    }
                }
            }
        }
        cells
    }

    fn connected_parents_from(&self, start: CellCoord) -> BTreeMap<CellCoord, Option<CellCoord>> {
        let mut parents = BTreeMap::from([(start, None)]);
        let mut pending = VecDeque::from([start]);

        while let Some(coord) = pending.pop_front() {
            for neighbor in self.neighbors(coord) {
                if self.get(neighbor).is_none() || parents.contains_key(&neighbor) {
                    continue;
                }
                parents.insert(neighbor, Some(coord));
                pending.push_back(neighbor);
            }
        }

        parents
    }

    fn lightning_up_column_from(&self, start: CellCoord) -> BTreeSet<CellCoord> {
        let mut cells = BTreeSet::from([start]);

        for row in (0..start.row).rev() {
            let coord = CellCoord::new(row, start.col);
            if self.get(coord).is_none() {
                break;
            }
            cells.insert(coord);
        }

        cells
    }

    fn drill_column_from(&self, start: CellCoord, depth: usize) -> BTreeSet<CellCoord> {
        let mut tunnel = BTreeSet::new();
        let last_row = ROWS.min(start.row + depth.max(1));

        for row in start.row..last_row {
            let coord = CellCoord::new(row, start.col);
            if self.get(coord).is_some() {
                tunnel.insert(coord);
            }
        }

        tunnel
    }

    fn cells_within_radius(&self, start: CellCoord, radius: usize) -> BTreeSet<CellCoord> {
        if !self.in_bounds(start) {
            return BTreeSet::new();
        }

        let mut cells = BTreeSet::from([start]);
        let mut seen = BTreeSet::from([start]);
        let mut pending = VecDeque::from([(start, 0usize)]);

        while let Some((coord, distance)) = pending.pop_front() {
            if distance == radius {
                continue;
            }

            for neighbor in self.neighbors(coord) {
                if !seen.insert(neighbor) {
                    continue;
                }
                cells.insert(neighbor);
                pending.push_back((neighbor, distance + 1));
            }
        }

        cells
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

    pub fn color_count(&self, color: BubbleColor) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|cell| **cell == Some(color))
            .count()
    }

    pub fn clear_bottom_rows(&mut self, rows: usize) {
        for row in ROWS.saturating_sub(rows)..ROWS {
            for col in 0..COLS {
                self.cells[row][col] = None;
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    pub popped: Vec<(CellCoord, BubbleColor)>,
    pub dropped: Vec<(CellCoord, BubbleColor)>,
    pub prizes: Vec<&'static str>,
}

/// Pops and drops both reward larger clusters, but stay on round-number curves
/// that are readable at a glance instead of the old exponential payout.
fn score_resolution(resolution: &Resolution) -> u64 {
    let breakdown = score_breakdown(resolution);
    breakdown.total()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScoreBreakdown {
    pub pop: u64,
    pub drop: u64,
}

impl ScoreBreakdown {
    pub const fn total(self) -> u64 {
        self.pop + self.drop
    }
}

pub fn score_breakdown(resolution: &Resolution) -> ScoreBreakdown {
    let popped = resolution.popped.len() as u64;
    let dropped = resolution.dropped.len() as u64;
    ScoreBreakdown {
        pop: round_to_nearest_10(popped * 10 + (popped / 5) * 20),
        drop: round_to_nearest_10(dropped * 40 + (dropped / 10) * 40),
    }
}

const fn round_to_nearest_10(n: u64) -> u64 {
    (n + 5) / 10 * 10
}

fn color_matches(candidate: BubbleColor, target: BubbleColor) -> bool {
    candidate == target || candidate == BubbleColor::Wild || candidate == BubbleColor::Prize
}

fn lightning_path_len(
    mut coord: CellCoord,
    parents: &BTreeMap<CellCoord, Option<CellCoord>>,
) -> usize {
    let mut len = 1;
    while let Some(Some(parent)) = parents.get(&coord) {
        len += 1;
        coord = *parent;
    }
    len
}

fn lightning_path_to(
    mut coord: CellCoord,
    parents: &BTreeMap<CellCoord, Option<CellCoord>>,
) -> BTreeSet<CellCoord> {
    let mut path = BTreeSet::new();
    loop {
        path.insert(coord);
        let Some(Some(parent)) = parents.get(&coord) else {
            break;
        };
        coord = *parent;
    }
    path
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BubbleRun {
    #[serde(default)]
    pub round: u32,
    #[serde(default)]
    pub shop_awarded_round: u32,
    #[serde(default)]
    pub shop_offer_shift: u32,
    #[serde(default)]
    pub banked_score: u64,
    #[serde(default)]
    pub wild_orb_level: u8,
    #[serde(default)]
    pub bomb_drop_level: u8,
    #[serde(default)]
    pub drop_delay_level: u8,
    #[serde(default)]
    pub queue_sight_level: u8,
    #[serde(default)]
    pub trace_sight_level: u8,
    #[serde(default)]
    pub prize_bubble_level: u8,
    #[serde(default)]
    pub color_sense_level: u8,
    #[serde(default)]
    pub clean_start_level: u8,
    #[serde(default)]
    pub bomb_cache_level: u8,
    #[serde(default)]
    pub wild_cache_level: u8,
    #[serde(default)]
    pub drop_reserve_level: u8,
    #[serde(default)]
    pub revive_charm_level: u8,
    #[serde(default)]
    pub bomb_radius_level: u8,
    #[serde(default)]
    pub storm_focus_level: u8,
    #[serde(default)]
    pub prize_quality_level: u8,
    #[serde(default)]
    pub extra_rows_level: u8,
    #[serde(default)]
    pub drop_haste_level: u8,
    #[serde(default)]
    pub pop_drop_level: u8,
    #[serde(default)]
    pub lightning_power_level: u8,
    #[serde(default)]
    pub drill_power_level: u8,
    #[serde(default)]
    pub reticle_level: u8,
    #[serde(default)]
    pub landing_dot_level: u8,
    #[serde(default)]
    pub wild_orb_active_level: u8,
    #[serde(default)]
    pub bomb_drop_active_level: u8,
    #[serde(default)]
    pub drop_delay_active_level: u8,
    #[serde(default)]
    pub queue_sight_active_level: u8,
    #[serde(default)]
    pub trace_sight_active_level: u8,
    #[serde(default)]
    pub prize_bubble_active_level: u8,
    #[serde(default)]
    pub clean_start_active_level: u8,
    #[serde(default)]
    pub wild_cache_active_level: u8,
    #[serde(default)]
    pub bomb_radius_active_level: u8,
    #[serde(default)]
    pub prize_quality_active_level: u8,
    #[serde(default)]
    pub bank_sight_active_level: u8,
    #[serde(default)]
    pub extra_rows_active_level: u8,
    #[serde(default)]
    pub drop_haste_active_level: u8,
    #[serde(default)]
    pub pop_drop_active_level: u8,
    #[serde(default)]
    pub lightning_power_active_level: u8,
    #[serde(default)]
    pub drill_power_active_level: u8,
    #[serde(default)]
    pub reticle_active_level: u8,
    #[serde(default)]
    pub landing_dot_active_level: u8,
    #[serde(default)]
    pub color_call_charges: u32,
    #[serde(default)]
    pub revive_charges: u32,
    #[serde(default)]
    pub bomb_shot_charges: u32,
    #[serde(default)]
    pub wild_shot_charges: u32,
    #[serde(default)]
    pub lightning_charges: u32,
    #[serde(default)]
    pub drill_charges: u32,
    #[serde(default)]
    pub drop_reset_charges: u32,
    #[serde(default)]
    pub bank_sight_level: u8,
    #[serde(default)]
    pub wild_orb_enabled: bool,
    #[serde(default)]
    pub bomb_drop_enabled: bool,
    #[serde(default)]
    pub drop_delay_enabled: bool,
    #[serde(default)]
    pub queue_sight_enabled: bool,
    #[serde(default)]
    pub trace_sight_enabled: bool,
    #[serde(default)]
    pub prize_bubble_enabled: bool,
    #[serde(default)]
    pub color_sense_enabled: bool,
    #[serde(default)]
    pub clean_start_enabled: bool,
    #[serde(default)]
    pub bomb_cache_enabled: bool,
    #[serde(default)]
    pub wild_cache_enabled: bool,
    #[serde(default)]
    pub drop_reserve_enabled: bool,
    #[serde(default)]
    pub revive_charm_enabled: bool,
    #[serde(default)]
    pub bomb_radius_enabled: bool,
    #[serde(default)]
    pub storm_focus_enabled: bool,
    #[serde(default)]
    pub prize_quality_enabled: bool,
    #[serde(default)]
    pub color_call_enabled: bool,
    #[serde(default)]
    pub revive_enabled: bool,
    #[serde(default)]
    pub bank_sight_enabled: bool,
    #[serde(default)]
    pub extra_rows_enabled: bool,
    #[serde(default)]
    pub drop_haste_enabled: bool,
    #[serde(default)]
    pub pop_drop_enabled: bool,
    #[serde(default)]
    pub lightning_power_enabled: bool,
    #[serde(default)]
    pub drill_power_enabled: bool,
    #[serde(default)]
    pub reticle_enabled: bool,
    #[serde(default)]
    pub landing_dot_enabled: bool,
}

impl Default for BubbleRun {
    fn default() -> Self {
        Self {
            round: 1,
            shop_awarded_round: 0,
            shop_offer_shift: 0,
            banked_score: 0,
            wild_orb_level: 0,
            bomb_drop_level: 0,
            drop_delay_level: 0,
            queue_sight_level: 0,
            trace_sight_level: 0,
            prize_bubble_level: 0,
            color_sense_level: 0,
            clean_start_level: 0,
            bomb_cache_level: 0,
            wild_cache_level: 0,
            drop_reserve_level: 0,
            revive_charm_level: 0,
            bomb_radius_level: 0,
            storm_focus_level: 0,
            prize_quality_level: 0,
            extra_rows_level: 0,
            drop_haste_level: 0,
            pop_drop_level: 0,
            lightning_power_level: 0,
            drill_power_level: 0,
            reticle_level: 0,
            landing_dot_level: 0,
            wild_orb_active_level: 0,
            bomb_drop_active_level: 0,
            drop_delay_active_level: 0,
            queue_sight_active_level: 0,
            trace_sight_active_level: 0,
            prize_bubble_active_level: 0,
            clean_start_active_level: 0,
            wild_cache_active_level: 0,
            bomb_radius_active_level: 0,
            prize_quality_active_level: 0,
            bank_sight_active_level: 0,
            extra_rows_active_level: 0,
            drop_haste_active_level: 0,
            pop_drop_active_level: 0,
            lightning_power_active_level: 0,
            drill_power_active_level: 0,
            reticle_active_level: 0,
            landing_dot_active_level: 0,
            color_call_charges: 0,
            revive_charges: 0,
            bomb_shot_charges: 0,
            wild_shot_charges: 0,
            lightning_charges: 0,
            drill_charges: 0,
            drop_reset_charges: 0,
            bank_sight_level: 0,
            wild_orb_enabled: false,
            bomb_drop_enabled: false,
            drop_delay_enabled: false,
            queue_sight_enabled: false,
            trace_sight_enabled: false,
            prize_bubble_enabled: false,
            color_sense_enabled: false,
            clean_start_enabled: false,
            bomb_cache_enabled: false,
            wild_cache_enabled: false,
            drop_reserve_enabled: false,
            revive_charm_enabled: false,
            bomb_radius_enabled: false,
            storm_focus_enabled: false,
            prize_quality_enabled: false,
            color_call_enabled: false,
            revive_enabled: false,
            bank_sight_enabled: false,
            extra_rows_enabled: false,
            drop_haste_enabled: false,
            pop_drop_enabled: false,
            lightning_power_enabled: false,
            drill_power_enabled: false,
            reticle_enabled: false,
            landing_dot_enabled: false,
        }
    }
}

impl BubbleRun {
    pub fn normalized(mut self) -> Self {
        self.round = self.round.max(1);
        self.shop_awarded_round = self.shop_awarded_round.min(self.round);
        self.wild_orb_level = self.wild_orb_level.min(3);
        self.bomb_drop_level = self.bomb_drop_level.min(3);
        self.drop_delay_level = self.drop_delay_level.min(3);
        self.queue_sight_level = self.queue_sight_level.min(3);
        self.trace_sight_level = self.trace_sight_level.min(1);
        self.prize_bubble_level = self.prize_bubble_level.min(3);
        self.color_sense_level = self.color_sense_level.min(3);
        self.clean_start_level = self.clean_start_level.min(2);
        self.bomb_cache_level = self.bomb_cache_level.min(2);
        self.wild_cache_level = self.wild_cache_level.min(2);
        self.drop_reserve_level = self.drop_reserve_level.min(3);
        self.revive_charm_level = self.revive_charm_level.min(2);
        self.bomb_radius_level = self.bomb_radius_level.min(1);
        self.storm_focus_level = self.storm_focus_level.min(2);
        self.prize_quality_level = self.prize_quality_level.min(3);
        self.bank_sight_level = self.bank_sight_level.min(3);
        self.extra_rows_level = self.extra_rows_level.min(2);
        self.drop_haste_level = self.drop_haste_level.min(3);
        self.pop_drop_level = self.pop_drop_level.min(1);
        self.lightning_power_level = self.lightning_power_level.min(3);
        self.drill_power_level = self.drill_power_level.min(3);
        self.reticle_level = self.reticle_level.min(2);
        self.landing_dot_level = self.landing_dot_level.min(1);
        if self.wild_orb_level == 0 {
            self.wild_orb_enabled = false;
        }
        if self.bomb_drop_level == 0 {
            self.bomb_drop_enabled = false;
        }
        if self.drop_delay_level == 0 {
            self.drop_delay_enabled = false;
        }
        if self.queue_sight_level == 0 {
            self.queue_sight_enabled = false;
        }
        if self.trace_sight_level == 0 {
            self.trace_sight_enabled = false;
        }
        if self.prize_bubble_level == 0 {
            self.prize_bubble_enabled = false;
        }
        if self.color_sense_level == 0 {
            self.color_sense_enabled = false;
        }
        if self.clean_start_level == 0 {
            self.clean_start_enabled = false;
        }
        if self.bomb_cache_level == 0 {
            self.bomb_cache_enabled = false;
        }
        if self.wild_cache_level == 0 {
            self.wild_cache_enabled = false;
        }
        if self.drop_reserve_level == 0 {
            self.drop_reserve_enabled = false;
        }
        if self.revive_charm_level == 0 {
            self.revive_charm_enabled = false;
        }
        if self.bomb_radius_level == 0 {
            self.bomb_radius_enabled = false;
        }
        if self.storm_focus_level == 0 {
            self.storm_focus_enabled = false;
        }
        if self.prize_quality_level == 0 {
            self.prize_quality_enabled = false;
        }
        if self.color_call_charges == 0 {
            self.color_call_enabled = false;
        }
        if self.revive_charges == 0 {
            self.revive_enabled = false;
        }
        if self.bank_sight_level == 0 {
            self.bank_sight_enabled = false;
        }
        if self.extra_rows_level == 0 {
            self.extra_rows_enabled = false;
        }
        if self.drop_haste_level == 0 {
            self.drop_haste_enabled = false;
        }
        if self.pop_drop_level == 0 {
            self.pop_drop_enabled = false;
        }
        if self.lightning_power_level == 0 {
            self.lightning_power_enabled = false;
        }
        if self.drill_power_level == 0 {
            self.drill_power_enabled = false;
        }
        if self.reticle_level == 0 {
            self.reticle_enabled = false;
        }
        if self.landing_dot_level == 0 {
            self.landing_dot_enabled = false;
        }
        if self.clean_start_enabled && self.extra_rows_enabled {
            self.extra_rows_enabled = false;
        }
        normalize_active_level(
            &mut self.wild_orb_enabled,
            self.wild_orb_level,
            &mut self.wild_orb_active_level,
        );
        normalize_active_level(
            &mut self.bomb_drop_enabled,
            self.bomb_drop_level,
            &mut self.bomb_drop_active_level,
        );
        normalize_active_level(
            &mut self.drop_delay_enabled,
            self.drop_delay_level,
            &mut self.drop_delay_active_level,
        );
        normalize_active_level(
            &mut self.queue_sight_enabled,
            self.queue_sight_level,
            &mut self.queue_sight_active_level,
        );
        normalize_active_level(
            &mut self.trace_sight_enabled,
            self.trace_sight_level,
            &mut self.trace_sight_active_level,
        );
        normalize_active_level(
            &mut self.prize_bubble_enabled,
            self.prize_bubble_level,
            &mut self.prize_bubble_active_level,
        );
        normalize_active_level(
            &mut self.clean_start_enabled,
            self.clean_start_level,
            &mut self.clean_start_active_level,
        );
        normalize_active_level(
            &mut self.wild_cache_enabled,
            self.wild_cache_level,
            &mut self.wild_cache_active_level,
        );
        normalize_active_level(
            &mut self.bomb_radius_enabled,
            self.bomb_radius_level,
            &mut self.bomb_radius_active_level,
        );
        normalize_active_level(
            &mut self.prize_quality_enabled,
            self.prize_quality_level,
            &mut self.prize_quality_active_level,
        );
        normalize_active_level(
            &mut self.bank_sight_enabled,
            self.bank_sight_level,
            &mut self.bank_sight_active_level,
        );
        normalize_active_level(
            &mut self.extra_rows_enabled,
            self.extra_rows_level,
            &mut self.extra_rows_active_level,
        );
        normalize_active_level(
            &mut self.drop_haste_enabled,
            self.drop_haste_level,
            &mut self.drop_haste_active_level,
        );
        normalize_active_level(
            &mut self.pop_drop_enabled,
            self.pop_drop_level,
            &mut self.pop_drop_active_level,
        );
        normalize_active_level(
            &mut self.lightning_power_enabled,
            self.lightning_power_level,
            &mut self.lightning_power_active_level,
        );
        normalize_active_level(
            &mut self.drill_power_enabled,
            self.drill_power_level,
            &mut self.drill_power_active_level,
        );
        normalize_active_level(
            &mut self.reticle_enabled,
            self.reticle_level,
            &mut self.reticle_active_level,
        );
        normalize_active_level(
            &mut self.landing_dot_enabled,
            self.landing_dot_level,
            &mut self.landing_dot_active_level,
        );
        self
    }

    pub(crate) fn active_wild_orb_level(&self) -> u8 {
        active_level(
            self.wild_orb_enabled,
            self.wild_orb_level,
            self.wild_orb_active_level,
        )
    }

    pub(crate) fn active_bomb_drop_level(&self) -> u8 {
        active_level(
            self.bomb_drop_enabled,
            self.bomb_drop_level,
            self.bomb_drop_active_level,
        )
    }

    pub(crate) fn active_drop_delay_level(&self) -> u8 {
        active_level(
            self.drop_delay_enabled,
            self.drop_delay_level,
            self.drop_delay_active_level,
        )
    }

    pub(crate) fn active_queue_sight_level(&self) -> u8 {
        active_level(
            self.queue_sight_enabled,
            self.queue_sight_level,
            self.queue_sight_active_level,
        )
    }

    pub(crate) fn active_trace_sight_level(&self) -> u8 {
        active_level(
            self.trace_sight_enabled,
            self.trace_sight_level,
            self.trace_sight_active_level,
        )
    }

    pub(crate) fn active_prize_bubble_level(&self) -> u8 {
        active_level(
            self.prize_bubble_enabled,
            self.prize_bubble_level,
            self.prize_bubble_active_level,
        )
    }

    pub(crate) fn active_clean_start_level(&self) -> u8 {
        active_level(
            self.clean_start_enabled,
            self.clean_start_level,
            self.clean_start_active_level,
        )
    }

    pub(crate) fn active_wild_cache_level(&self) -> u8 {
        active_level(
            self.wild_cache_enabled,
            self.wild_cache_level,
            self.wild_cache_active_level,
        )
    }

    pub(crate) fn active_bomb_radius_level(&self) -> u8 {
        active_level(
            self.bomb_radius_enabled,
            self.bomb_radius_level,
            self.bomb_radius_active_level,
        )
    }

    pub(crate) fn active_prize_quality_level(&self) -> u8 {
        active_level(
            self.prize_quality_enabled,
            self.prize_quality_level,
            self.prize_quality_active_level,
        )
    }

    pub(crate) fn active_bank_sight_level(&self) -> u8 {
        active_level(
            self.bank_sight_enabled,
            self.bank_sight_level,
            self.bank_sight_active_level,
        )
    }

    pub(crate) fn active_extra_rows_level(&self) -> u8 {
        active_level(
            self.extra_rows_enabled,
            self.extra_rows_level,
            self.extra_rows_active_level,
        )
    }

    pub(crate) fn active_drop_haste_level(&self) -> u8 {
        active_level(
            self.drop_haste_enabled,
            self.drop_haste_level,
            self.drop_haste_active_level,
        )
    }

    pub(crate) fn active_pop_drop_level(&self) -> u8 {
        active_level(
            self.pop_drop_enabled,
            self.pop_drop_level,
            self.pop_drop_active_level,
        )
    }

    pub(crate) fn active_lightning_power_level(&self) -> u8 {
        active_level(
            self.lightning_power_enabled,
            self.lightning_power_level,
            self.lightning_power_active_level,
        )
    }

    pub(crate) fn active_drill_power_level(&self) -> u8 {
        active_level(
            self.drill_power_enabled,
            self.drill_power_level,
            self.drill_power_active_level,
        )
    }

    pub(crate) fn active_reticle_level(&self) -> u8 {
        active_level(
            self.reticle_enabled,
            self.reticle_level,
            self.reticle_active_level,
        )
    }

    pub(crate) fn active_landing_dot_level(&self) -> u8 {
        active_level(
            self.landing_dot_enabled,
            self.landing_dot_level,
            self.landing_dot_active_level,
        )
    }

    fn wild_chance_per_mille(&self) -> u32 {
        match self.active_wild_orb_level().min(3) {
            0 => 0,
            1 => 50,
            2 => 100,
            _ => 200,
        }
    }

    fn prize_chance_per_mille(&self) -> u32 {
        match self.active_prize_bubble_level().min(3) {
            0 => 0,
            1 => 100,
            2 => 200,
            _ => 300,
        }
    }

    fn bomb_drop_chance_per_mille(&self) -> u32 {
        let chance = match self.active_bomb_drop_level().min(3) {
            0 => 0,
            1 => 90,
            2 => 135,
            _ => 180,
        };
        (chance + if self.breach_kit_complete() { 45 } else { 0 }).min(500)
    }

    fn queue_size(&self) -> usize {
        4 + self.active_queue_sight_level().min(3) as usize + usize::from(self.scout_kit_complete())
    }

    fn bomb_blast_radius(&self) -> usize {
        BOMB_BLAST_RADIUS + usize::from(self.active_bomb_radius_level() > 0)
    }

    pub(crate) fn drill_depth(&self) -> usize {
        5 + self.active_drill_power_level().min(3) as usize * 2
    }

    fn wild_shot_multi_match_enabled(&self) -> bool {
        self.active_wild_cache_level() >= 2
    }

    fn add_round_start_charges(&mut self) {}

    fn starting_rows(&self, base_rows: usize) -> usize {
        let reduction = self.active_clean_start_level().min(2) as usize;
        let addition = if reduction == 0 {
            self.active_extra_rows_level().min(2) as usize
        } else {
            0
        };
        (base_rows + addition)
            .saturating_sub(reduction)
            .clamp(3, ROWS.saturating_sub(2))
    }

    fn scout_kit_complete(&self) -> bool {
        self.active_queue_sight_level() > 0
            && self.active_bank_sight_level() > 0
            && self.active_trace_sight_level() > 0
            && self.active_reticle_level() > 0
            && self.active_landing_dot_level() > 0
    }

    fn risk_kit_complete(&self) -> bool {
        self.active_drop_delay_level() > 0
            && (self.active_clean_start_level() > 0 || self.active_extra_rows_level() > 0)
            && (self.active_drop_haste_level() > 0 || self.active_pop_drop_level() > 0)
    }

    fn breach_kit_complete(&self) -> bool {
        self.active_bomb_drop_level() > 0
            && self.active_bomb_radius_level() > 0
            && self.active_lightning_power_level() > 0
            && self.active_drill_power_level() > 0
    }

    fn luck_kit_complete(&self) -> bool {
        self.active_wild_orb_level() > 0
            && self.active_prize_bubble_level() > 0
            && self.active_prize_quality_level() > 0
            && self.active_wild_cache_level() > 0
    }
}

fn active_level(enabled: bool, purchased: u8, active: u8) -> u8 {
    if !enabled || purchased == 0 {
        0
    } else if active == 0 {
        purchased
    } else {
        active.min(purchased)
    }
}

fn normalize_active_level(enabled: &mut bool, purchased: u8, active: &mut u8) {
    if purchased == 0 {
        *enabled = false;
        *active = 0;
    } else if *enabled {
        *active = if *active == 0 {
            purchased
        } else {
            (*active).min(purchased)
        };
    } else {
        *active = 0;
    }
}

#[derive(Clone, Debug)]
pub struct BubbleGame {
    pub board: Board,
    pub score: u64,
    pub high_score: u64,
    pub shots_until_drop: u32,
    pub game_over: bool,
    pub held: Option<BubbleColor>,
    pub run: BubbleRun,
    rng: Lcg,
    queue: VecDeque<BubbleColor>,
    rules: GameRules,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SavedBubbleGame {
    version: u8,
    hard: bool,
    board: Board,
    score: u64,
    shots_until_drop: u32,
    game_over: bool,
    held: Option<BubbleColor>,
    #[serde(default)]
    run: BubbleRun,
    rng_state: u64,
    queue: Vec<BubbleColor>,
}

impl BubbleGame {
    pub fn new(seed: u64, high_score: u64) -> Self {
        Self::with_rules(seed, high_score, GameRules::normal(), BubbleRun::default())
    }

    pub fn hard(seed: u64, high_score: u64) -> Self {
        Self::with_rules(seed, high_score, GameRules::hard(), BubbleRun::default())
    }

    pub fn with_run(seed: u64, high_score: u64, hard: bool, run: BubbleRun) -> Self {
        Self::with_rules(
            seed,
            high_score,
            GameRules::for_mode(hard),
            run.normalized(),
        )
    }

    fn with_rules(seed: u64, high_score: u64, rules: GameRules, run: BubbleRun) -> Self {
        let run = run.normalized();
        let board = Board::seeded(seed ^ 0xa53a_91c7, run.starting_rows(rules.starting_rows()));
        let mut rng = Lcg::new(seed);
        let active_colors = rules.colors_for_board(&board);
        let mut queue = VecDeque::new();
        refill_queue_from(&mut queue, &mut rng, &active_colors, &run);

        Self {
            board,
            score: 0,
            high_score,
            shots_until_drop: rules.shots_per_drop(&run),
            game_over: false,
            held: None,
            run,
            rng,
            queue,
            rules,
        }
    }

    pub fn current_color(&self) -> BubbleColor {
        self.queue[0]
    }

    pub fn next_colors(&self) -> Vec<BubbleColor> {
        self.queue.iter().copied().skip(1).collect()
    }

    pub fn save_state(&self) -> SavedBubbleGame {
        SavedBubbleGame {
            version: CURRENT_SAVE_VERSION,
            hard: self.rules.is_hard(),
            board: self.board.clone(),
            score: self.score,
            shots_until_drop: self.shots_until_drop,
            game_over: self.game_over,
            held: self.held,
            run: self.run.clone(),
            rng_state: self.rng.state,
            queue: self.queue.iter().copied().collect(),
        }
    }

    pub fn from_save(save: SavedBubbleGame, high_score: u64, hard: bool) -> Option<Self> {
        if !(LIVE_SAVE_VERSION..=CURRENT_SAVE_VERSION).contains(&save.version) || save.hard != hard
        {
            return None;
        }
        if !save.is_resumable()
            || save.queue.len() < 4
            || save.queue.len() > 8
            || save.board.parity > 1
        {
            return None;
        }

        let rules = GameRules::for_mode(hard);
        let mut run = save.run.normalized();
        if save.shots_until_drop == 0 || save.shots_until_drop > rules.shots_per_drop(&run) {
            return None;
        }

        let score = save.score;
        if save.version < CURRENT_SAVE_VERSION
            && save.game_over
            && save.board.is_cleared()
            && run.shop_awarded_round == run.round
            && run.banked_score == 0
        {
            run.banked_score = score;
        }
        let mut queue = save.queue;
        while queue.len() > run.queue_size() {
            queue.pop();
        }

        let mut game = Self {
            board: save.board,
            score,
            high_score: high_score.max(score),
            shots_until_drop: save.shots_until_drop,
            game_over: save.game_over,
            held: save.held,
            run,
            rng: Lcg {
                state: save.rng_state.max(1),
            },
            queue: queue.into(),
            rules,
        };
        game.refill_queue();
        Some(game)
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
    /// `next_colors` advance the instant a shot leaves the cannon. The final
    /// queue slot is refilled after the shot resolves so cleared colors are not
    /// requeued from the pre-shot board state.
    pub fn pop_next(&mut self) -> BubbleColor {
        self.queue.pop_front().expect("queue is never empty")
    }

    pub fn attach_current(&mut self, coord: CellCoord) -> Resolution {
        let color = self.current_color();
        self.queue.pop_front();
        self.attach_color(coord, color)
    }

    /// Place a bubble of the given color and run resolution. The queue is left
    /// alone — the shoot path already advanced it via `pop_next`.
    pub fn attach_color(&mut self, coord: CellCoord, color: BubbleColor) -> Resolution {
        if self.game_over || !self.board.in_bounds(coord) || !self.board.is_empty(coord) {
            return Resolution::default();
        }

        self.board.set(coord, Some(color));

        let mut resolution = self.board.resolve_after_attach_with_options(
            coord,
            self.run.bomb_blast_radius(),
            self.run.wild_shot_multi_match_enabled(),
            self.run.active_lightning_power_level(),
            self.run.drill_depth(),
        );
        self.score = self.score.saturating_add(score_resolution(&resolution));
        self.high_score = self.high_score.max(self.score);
        let prize_count = resolution
            .popped
            .iter()
            .filter(|(_, color)| *color == BubbleColor::Prize)
            .count();
        for _ in 0..prize_count {
            resolution.prizes.extend(self.grant_prize_reward());
        }

        if self.board.is_cleared() {
            self.game_over = true;
            return resolution;
        }

        if resolution.popped.is_empty() || self.run.active_pop_drop_level() > 0 {
            self.advance_ceiling();
        }
        if !self.game_over {
            self.sync_future_bubbles();
        }

        resolution
    }

    pub fn drill_from(&mut self, coord: CellCoord) -> Resolution {
        if self.game_over || !self.board.in_bounds(coord) || self.board.get(coord).is_none() {
            return Resolution::default();
        }

        let anchor_color = self.board.get(coord);
        self.board.set(coord, Some(BubbleColor::Drill));
        let mut resolution = self.board.resolve_after_attach_with_options(
            coord,
            self.run.bomb_blast_radius(),
            self.run.wild_shot_multi_match_enabled(),
            self.run.active_lightning_power_level(),
            self.run.drill_depth(),
        );
        self.score = self.score.saturating_add(score_resolution(&resolution));
        self.high_score = self.high_score.max(self.score);
        let prize_count = resolution
            .popped
            .iter()
            .filter(|(_, color)| *color == BubbleColor::Prize)
            .count()
            + usize::from(anchor_color == Some(BubbleColor::Prize));
        for _ in 0..prize_count {
            resolution.prizes.extend(self.grant_prize_reward());
        }

        if self.board.is_cleared() {
            self.game_over = true;
            return resolution;
        }
        if resolution.popped.is_empty() || self.run.active_pop_drop_level() > 0 {
            self.advance_ceiling();
        }
        if !self.game_over {
            self.sync_future_bubbles();
        }
        resolution
    }

    pub fn drill_through(&mut self, cells: impl IntoIterator<Item = CellCoord>) -> Resolution {
        if self.game_over {
            return Resolution::default();
        }

        let mut resolution = self.board.resolve_drill_cells(cells);
        self.score = self.score.saturating_add(score_resolution(&resolution));
        self.high_score = self.high_score.max(self.score);
        let prize_count = resolution
            .popped
            .iter()
            .filter(|(_, color)| *color == BubbleColor::Prize)
            .count();
        for _ in 0..prize_count {
            resolution.prizes.extend(self.grant_prize_reward());
        }

        if self.board.is_cleared() {
            self.game_over = true;
            return resolution;
        }
        if resolution.popped.is_empty() || self.run.active_pop_drop_level() > 0 {
            self.advance_ceiling();
        }
        if !self.game_over {
            self.sync_future_bubbles();
        }
        resolution
    }

    pub fn finish_discarded_shot(&mut self) {
        if !self.game_over {
            self.sync_future_bubbles();
        }
    }

    pub fn restart(&mut self) {
        self.restart_with_run(BubbleRun::default());
    }

    pub fn restart_with_run(&mut self, run: BubbleRun) {
        let high_score = self.high_score;
        *self = Self::with_rules(
            self.rng.next_u32() as u64 ^ 0x710d_d15c,
            high_score,
            self.rules,
            run,
        );
    }

    pub fn start_next_round(&mut self) {
        let high_score = self.high_score.max(self.score);
        let mut run = self.run.clone();
        run.round = run.round.saturating_add(1).max(2);
        run.add_round_start_charges();
        *self = Self::with_rules(
            self.rng.next_u32() as u64 ^ 0x35aa_51d0,
            high_score,
            self.rules,
            run,
        );
        self.high_score = high_score;
    }

    pub fn shop_score(&self) -> u64 {
        self.run.banked_score
    }

    pub fn bank_current_score(&mut self) -> u64 {
        let score = self.score;
        self.run.banked_score = self.run.banked_score.saturating_add(score);
        score
    }

    pub fn spend_shop_score(&mut self, amount: u64) -> bool {
        if self.run.banked_score < amount {
            return false;
        }
        self.run.banked_score -= amount;
        true
    }

    pub fn add_shop_score(&mut self, amount: u64) {
        self.run.banked_score = self.run.banked_score.saturating_add(amount);
    }

    pub fn call_color(&mut self) -> Option<BubbleColor> {
        if self.game_over || !self.run.color_call_enabled || self.run.color_call_charges == 0 {
            return None;
        }
        let color = self
            .board
            .active_colors()
            .into_iter()
            .max_by_key(|color| self.board.color_count(*color))?;
        self.queue[0] = color;
        self.run.color_call_charges -= 1;
        Some(color)
    }

    pub fn load_bomb_shot(&mut self) -> bool {
        if self.game_over || self.run.bomb_shot_charges == 0 {
            return false;
        }
        self.queue[0] = BubbleColor::Bomb;
        self.run.bomb_shot_charges -= 1;
        true
    }

    pub fn load_wild_shot(&mut self) -> bool {
        if self.game_over || self.run.wild_shot_charges == 0 {
            return false;
        }
        self.queue[0] = BubbleColor::Wild;
        self.run.wild_shot_charges -= 1;
        true
    }

    pub fn load_lightning_shot(&mut self) -> bool {
        if self.game_over || self.run.lightning_charges == 0 {
            return false;
        }
        self.queue[0] = BubbleColor::Lightning;
        self.run.lightning_charges -= 1;
        true
    }

    pub fn load_drill_shot(&mut self) -> bool {
        if self.game_over || self.run.drill_charges == 0 {
            return false;
        }
        self.queue[0] = BubbleColor::Drill;
        self.run.drill_charges -= 1;
        true
    }

    pub fn reset_drop_counter(&mut self) -> bool {
        if self.game_over || self.run.drop_reset_charges == 0 {
            return false;
        }
        self.run.drop_reset_charges -= 1;
        self.shots_until_drop = self.rules.shots_per_drop(&self.run);
        true
    }

    fn advance_ceiling(&mut self) {
        if self.shots_until_drop > 1 {
            self.shots_until_drop -= 1;
            return;
        }

        let active_colors = self.rules.colors_for_board(&self.board);
        let mut colors = [BubbleColor::Rose; COLS];
        for color in &mut colors {
            *color = self.rng.next_color_from(&active_colors);
        }
        let mut special_slots = [false; COLS];
        let prize_bubble_level = self.run.active_prize_bubble_level();
        if prize_bubble_level > 0 && self.rng.chance_per_mille(self.run.prize_chance_per_mille()) {
            let prizes = if prize_bubble_level >= 3 && self.rng.chance_per_mille(180) {
                2
            } else {
                1
            };
            for _ in 0..prizes {
                if let Some(index) = choose_open_slot(&mut self.rng, &mut special_slots) {
                    colors[index] = BubbleColor::Prize;
                }
            }
        }
        let bomb_drop_level = self.run.active_bomb_drop_level();
        if bomb_drop_level > 0
            && self
                .rng
                .chance_per_mille(self.run.bomb_drop_chance_per_mille())
        {
            let bombs = if bomb_drop_level >= 3 && self.rng.chance_per_mille(260) {
                2
            } else {
                1
            };
            for _ in 0..bombs {
                if let Some(index) = choose_open_slot(&mut self.rng, &mut special_slots) {
                    colors[index] = BubbleColor::Bomb;
                }
            }
        }

        let overflow = self.board.push_ceiling_row(&colors);
        self.shots_until_drop = self.rules.shots_per_drop(&self.run);
        if overflow {
            self.handle_overflow();
        }
    }

    fn sync_future_bubbles(&mut self) {
        let active_colors = self.rules.colors_for_board(&self.board);
        self.refill_queue_from(&active_colors);
    }

    fn refill_queue(&mut self) {
        let active_colors = self.rules.colors_for_board(&self.board);
        self.refill_queue_from(&active_colors);
    }

    fn refill_queue_from(&mut self, active_colors: &[BubbleColor]) {
        refill_queue_from(&mut self.queue, &mut self.rng, active_colors, &self.run);
    }

    fn handle_overflow(&mut self) {
        if !self.run.revive_enabled || self.run.revive_charges == 0 {
            self.game_over = true;
            return;
        }
        self.run.revive_charges -= 1;
        self.board.clear_bottom_rows(2);
        self.shots_until_drop = self.rules.shots_per_drop(&self.run);
    }

    fn grant_prize_reward(&mut self) -> Vec<&'static str> {
        let mut rewards = 1;
        let prize_quality_level = self.run.active_prize_quality_level();
        if prize_quality_level > 0 {
            let extra_chance = match prize_quality_level.min(3) {
                0 => 0,
                1 => 120,
                2 => 240,
                _ => 420,
            } + if self.run.luck_kit_complete() { 100 } else { 0 };
            if self.rng.chance_per_mille(extra_chance) {
                rewards += 1;
            }
        }

        (0..rewards)
            .map(|_| self.grant_one_prize_reward())
            .collect()
    }

    fn grant_one_prize_reward(&mut self) -> &'static str {
        let quality = self.run.active_prize_quality_level().min(3);
        let reward_count = 5 + quality as u32;
        match self.rng.next_u32() % reward_count {
            0 => {
                self.run.color_call_charges = self.run.color_call_charges.saturating_add(1);
                self.run.color_call_enabled = true;
                "Color Call"
            }
            1 => {
                self.run.bomb_shot_charges = self.run.bomb_shot_charges.saturating_add(1);
                "Bomb Shot"
            }
            2 => {
                self.run.lightning_charges = self.run.lightning_charges.saturating_add(1);
                "Lightning"
            }
            3 => {
                self.run.drill_charges = self.run.drill_charges.saturating_add(1);
                "Drill Shot"
            }
            4 => {
                self.run.drop_reset_charges = self.run.drop_reset_charges.saturating_add(1);
                "Drop Reset"
            }
            5 => {
                self.run.wild_shot_charges = self.run.wild_shot_charges.saturating_add(1);
                "Wild Shot"
            }
            6 => {
                self.run.revive_charges = self.run.revive_charges.saturating_add(1);
                self.run.revive_enabled = true;
                "Second Breath"
            }
            _ => {
                self.run.bomb_shot_charges = self.run.bomb_shot_charges.saturating_add(1);
                self.run.lightning_charges = self.run.lightning_charges.saturating_add(1);
                "Bomb + Lightning"
            }
        }
    }
}

impl SavedBubbleGame {
    pub fn is_resumable(&self) -> bool {
        (!self.game_over && !self.board.is_cleared()) || (self.game_over && self.board.is_cleared())
    }
}

#[derive(Clone, Copy, Debug)]
struct GameRules {
    shots_per_drop: u32,
    use_active_palette: bool,
    hard: bool,
}

impl GameRules {
    const fn for_mode(hard: bool) -> Self {
        if hard {
            Self::hard()
        } else {
            Self::normal()
        }
    }

    const fn normal() -> Self {
        Self {
            shots_per_drop: SHOTS_PER_DROP,
            use_active_palette: true,
            hard: false,
        }
    }

    const fn hard() -> Self {
        Self {
            shots_per_drop: HARD_SHOTS_PER_DROP,
            use_active_palette: false,
            hard: true,
        }
    }

    fn colors_for_board(self, board: &Board) -> Vec<BubbleColor> {
        if self.use_active_palette {
            board.active_colors()
        } else {
            COLORS.to_vec()
        }
    }

    const fn is_hard(self) -> bool {
        self.hard
    }

    fn shots_per_drop(self, run: &BubbleRun) -> u32 {
        (self.shots_per_drop
            + run.active_drop_delay_level().min(3) as u32
            + if run.risk_kit_complete() { 1 } else { 0 })
        .saturating_sub(run.active_drop_haste_level().min(3) as u32)
        .max(2)
    }

    const fn starting_rows(self) -> usize {
        if self.hard {
            6
        } else {
            5
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

    fn chance_per_mille(&mut self, per_mille: u32) -> bool {
        per_mille > 0 && self.next_u32() % 1000 < per_mille
    }
}

fn refill_queue_from(
    queue: &mut VecDeque<BubbleColor>,
    rng: &mut Lcg,
    active_colors: &[BubbleColor],
    run: &BubbleRun,
) {
    while queue.len() > run.queue_size() {
        queue.pop_back();
    }
    while queue.len() < run.queue_size() {
        let mut color = rng.next_color_from(active_colors);
        if rng.chance_per_mille(run.wild_chance_per_mille()) {
            color = BubbleColor::Wild;
        }
        queue.push_back(color);
    }
}

fn choose_open_slot(rng: &mut Lcg, occupied: &mut [bool; COLS]) -> Option<usize> {
    for _ in 0..COLS * 2 {
        let index = rng.next_u32() as usize % COLS;
        if !occupied[index] {
            occupied[index] = true;
            return Some(index);
        }
    }

    occupied.iter_mut().enumerate().find_map(|(index, slot)| {
        if *slot {
            None
        } else {
            *slot = true;
            Some(index)
        }
    })
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
    fn big_clusters_score_more_without_exploding() {
        // 5 popped, no drops: 5*10 + one 5-pop milestone bonus.
        let resolution = Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 5],
            dropped: Vec::new(),
            prizes: Vec::new(),
        };
        assert_eq!(score_resolution(&resolution), 70);
    }

    #[test]
    fn drops_are_strong_without_exponential_scaling() {
        let resolution = Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 3],
            dropped: vec![(CellCoord::new(0, 0), BubbleColor::Gold); 3],
            prizes: Vec::new(),
        };
        assert_eq!(score_resolution(&resolution), 150);
    }

    #[test]
    fn giant_drops_are_worth_more_than_basic_pop_grind() {
        let basic_pop_grind = score_resolution(&Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 3],
            dropped: Vec::new(),
            prizes: Vec::new(),
        }) * 11;
        let resolution = Resolution {
            popped: vec![(CellCoord::new(0, 0), BubbleColor::Rose); 3],
            dropped: vec![(CellCoord::new(0, 0), BubbleColor::Gold); 30],
            prizes: Vec::new(),
        };
        let score = score_resolution(&resolution);

        assert_eq!(score, 1350);
        assert!(score >= basic_pop_grind * 4);
    }

    #[test]
    fn pop_next_advances_queue() {
        let mut game = BubbleGame::new(7, 0);
        let first = game.current_color();
        let second = game.next_colors()[0];

        let fired = game.pop_next();

        assert_eq!(fired, first);
        assert_eq!(game.current_color(), second);
        assert_eq!(game.queue.len(), 3);
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
    fn lightning_clears_the_landed_column() {
        let mut board = Board::empty();
        board.set(CellCoord::new(0, 2), Some(BubbleColor::Gold));
        board.set(CellCoord::new(1, 2), Some(BubbleColor::Sky));
        board.set(CellCoord::new(2, 2), Some(BubbleColor::Lightning));
        board.set(CellCoord::new(0, 4), Some(BubbleColor::Mint));

        let resolution = board.resolve_after_attach(CellCoord::new(2, 2));

        assert_eq!(resolution.popped.len(), 3);
        assert!(board.is_empty(CellCoord::new(0, 2)));
        assert!(board.is_empty(CellCoord::new(1, 2)));
        assert!(board.is_empty(CellCoord::new(2, 2)));
        assert_eq!(board.get(CellCoord::new(0, 4)), Some(BubbleColor::Mint));
    }

    #[test]
    fn lightning_stops_at_column_gaps() {
        let mut board = Board::empty();
        board.set(CellCoord::new(0, 2), Some(BubbleColor::Gold));
        board.set(CellCoord::new(2, 2), Some(BubbleColor::Lightning));
        board.set(CellCoord::new(3, 2), Some(BubbleColor::Sky));

        let resolution = board.resolve_after_attach(CellCoord::new(2, 2));

        assert_eq!(resolution.popped.len(), 1);
        assert_eq!(resolution.dropped.len(), 1);
        assert_eq!(board.get(CellCoord::new(0, 2)), Some(BubbleColor::Gold));
        assert!(board.is_empty(CellCoord::new(2, 2)));
        assert!(resolution
            .dropped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(3, 2)));
    }

    #[test]
    fn lightning_travels_connected_chain_toward_the_top() {
        let mut board = Board::empty();
        let chain = [
            CellCoord::new(4, 3),
            CellCoord::new(3, 3),
            CellCoord::new(2, 4),
            CellCoord::new(1, 4),
            CellCoord::new(0, 5),
        ];
        board.set(chain[0], Some(BubbleColor::Lightning));
        for coord in chain.iter().copied().skip(1) {
            board.set(coord, Some(BubbleColor::Sky));
        }
        board.set(CellCoord::new(0, 6), Some(BubbleColor::Gold));

        let resolution = board.resolve_after_attach(chain[0]);

        assert_eq!(resolution.popped.len(), chain.len());
        for coord in chain {
            assert!(board.is_empty(coord));
        }
        assert_eq!(board.get(CellCoord::new(0, 6)), Some(BubbleColor::Gold));
    }

    #[test]
    fn max_lightning_does_not_pop_unconnected_islands() {
        let mut board = Board::empty();
        let chain = [
            CellCoord::new(3, 3),
            CellCoord::new(2, 3),
            CellCoord::new(1, 3),
            CellCoord::new(0, 3),
        ];
        board.set(chain[0], Some(BubbleColor::Lightning));
        for coord in chain.iter().copied().skip(1) {
            board.set(coord, Some(BubbleColor::Sky));
        }
        board.set(CellCoord::new(0, 8), Some(BubbleColor::Mint));
        board.set(CellCoord::new(1, 8), Some(BubbleColor::Mint));

        let resolution =
            board.resolve_after_attach_with_options(chain[0], BOMB_BLAST_RADIUS, false, 3, 5);

        assert!(!resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(0, 8)));
        assert!(!resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(1, 8)));
        assert_eq!(board.get(CellCoord::new(0, 8)), Some(BubbleColor::Mint));
        assert_eq!(board.get(CellCoord::new(1, 8)), Some(BubbleColor::Mint));
    }

    #[test]
    fn max_lightning_on_connected_board_only_pops_path_and_top_row() {
        let mut board = Board::empty();
        for row in 0..5 {
            for col in 0..COLS {
                board.set(CellCoord::new(row, col), Some(BubbleColor::Rose));
            }
        }
        let start = CellCoord::new(4, 5);
        board.set(start, Some(BubbleColor::Lightning));
        let occupied_before = board.occupied_cells().count();

        let resolution =
            board.resolve_after_attach_with_options(start, BOMB_BLAST_RADIUS, false, 3, 5);

        assert!(resolution.popped.len() < occupied_before);
        assert!(!board.is_cleared());
    }

    #[test]
    fn lightning_power_levels_scale_on_connected_board() {
        let mut popped_by_power = Vec::new();
        for power in 0..=3 {
            let mut board = Board::empty();
            for row in 0..5 {
                for col in 0..COLS {
                    board.set(CellCoord::new(row, col), Some(BubbleColor::Rose));
                }
            }
            let start = CellCoord::new(4, 5);
            board.set(start, Some(BubbleColor::Lightning));

            let resolution =
                board.resolve_after_attach_with_options(start, BOMB_BLAST_RADIUS, false, power, 5);
            popped_by_power.push(resolution.popped.len());
        }

        assert!(popped_by_power[1] > popped_by_power[0]);
        assert!(popped_by_power[2] > popped_by_power[1]);
        assert!(popped_by_power[3] > popped_by_power[2]);
    }

    #[test]
    fn bomb_blast_reaches_radius_beyond_touching_neighbors() {
        let mut board = Board::empty();
        board.set(CellCoord::new(3, 3), Some(BubbleColor::Bomb));
        board.set(CellCoord::new(3, 4), Some(BubbleColor::Rose));
        board.set(CellCoord::new(3, 5), Some(BubbleColor::Sky));
        board.set(CellCoord::new(0, 6), Some(BubbleColor::Mint));

        let resolution = board.resolve_after_attach(CellCoord::new(3, 3));

        assert_eq!(resolution.popped.len(), 3);
        assert!(resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(3, 5)));
        assert_eq!(board.get(CellCoord::new(0, 6)), Some(BubbleColor::Mint));
    }

    #[test]
    fn bomb_blast_chains_to_bombs_inside_radius() {
        let mut board = Board::empty();
        board.set(CellCoord::new(3, 3), Some(BubbleColor::Bomb));
        board.set(CellCoord::new(3, 5), Some(BubbleColor::Bomb));
        board.set(CellCoord::new(3, 7), Some(BubbleColor::Sky));

        let resolution = board.resolve_after_attach(CellCoord::new(3, 3));

        assert!(resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(3, 7)));
    }

    #[test]
    fn drill_clears_a_short_tunnel() {
        let mut board = Board::empty();
        for row in 2..8 {
            board.set(CellCoord::new(row, 3), Some(BubbleColor::Gold));
        }
        board.set(CellCoord::new(2, 3), Some(BubbleColor::Drill));
        board.set(CellCoord::new(0, 5), Some(BubbleColor::Mint));

        let resolution = board.resolve_after_attach(CellCoord::new(2, 3));

        assert_eq!(resolution.popped.len(), 5);
        for row in 2..7 {
            assert!(board.is_empty(CellCoord::new(row, 3)));
        }
        assert_eq!(board.get(CellCoord::new(0, 5)), Some(BubbleColor::Mint));
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
    fn hard_mode_uses_four_shots_per_drop() {
        let mut game = BubbleGame::hard(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([
            BubbleColor::Mint,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Rose,
        ]);

        assert_eq!(game.shots_until_drop, HARD_SHOTS_PER_DROP);

        let resolution = game.attach_current(CellCoord::new(5, 5));

        assert!(resolution.popped.is_empty());
        assert_eq!(game.shots_until_drop, HARD_SHOTS_PER_DROP - 1);

        game.queue.clear();
        game.queue.extend([
            BubbleColor::Rose,
            BubbleColor::Gold,
            BubbleColor::Sky,
            BubbleColor::Mint,
        ]);
        game.shots_until_drop = 1;
        let resolution = game.attach_current(CellCoord::new(8, 8));

        assert!(resolution.popped.is_empty());
        assert_eq!(game.shots_until_drop, HARD_SHOTS_PER_DROP);
    }

    #[test]
    fn normal_mode_keeps_eliminated_colors_in_queue_and_hold() {
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
        assert_eq!(game.current_color(), BubbleColor::Coral);
        assert_eq!(game.held, Some(BubbleColor::Rose));
    }

    #[test]
    fn normal_mode_refills_from_board_colors_without_pruning_queue() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([BubbleColor::Coral]);
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Gold));

        game.refill_queue();

        assert_eq!(game.current_color(), BubbleColor::Coral);
        assert_eq!(game.next_colors(), vec![BubbleColor::Gold; 3]);
    }

    #[test]
    fn normal_mode_does_not_refill_shot_slot_with_color_about_to_clear() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.queue.clear();
        game.queue.extend([BubbleColor::Rose]);
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 3), Some(BubbleColor::Gold));

        let resolution = game.attach_current(CellCoord::new(1, 0));

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(game.queue, VecDeque::from(vec![BubbleColor::Gold; 4]));
    }

    #[test]
    fn normal_mode_spawns_a_cleared_color_after_queue_puts_it_back() {
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

        let resolution = game.attach_current(CellCoord::new(5, 5));

        assert!(resolution.popped.is_empty());
        assert_eq!(game.shots_until_drop, SHOTS_PER_DROP);
        for col in 0..COLS {
            assert_eq!(
                game.board.get(CellCoord::new(0, col)),
                Some(BubbleColor::Rose)
            );
        }
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
    fn consumable_shots_load_current_slot_and_spend_a_charge() {
        let mut game = BubbleGame::new(7, 0);
        game.run.bomb_shot_charges = 1;
        game.run.wild_shot_charges = 1;
        game.run.lightning_charges = 1;
        game.run.drill_charges = 1;
        game.run.drop_reset_charges = 1;
        game.shots_until_drop = 1;

        assert!(game.load_bomb_shot());
        assert_eq!(game.current_color(), BubbleColor::Bomb);
        assert_eq!(game.run.bomb_shot_charges, 0);

        assert!(game.load_wild_shot());
        assert_eq!(game.current_color(), BubbleColor::Wild);
        assert_eq!(game.run.wild_shot_charges, 0);

        assert!(game.load_lightning_shot());
        assert_eq!(game.current_color(), BubbleColor::Lightning);
        assert_eq!(game.run.lightning_charges, 0);

        assert!(game.load_drill_shot());
        assert_eq!(game.current_color(), BubbleColor::Drill);
        assert_eq!(game.run.drill_charges, 0);

        assert!(game.reset_drop_counter());
        assert_eq!(game.shots_until_drop, SHOTS_PER_DROP);
        assert_eq!(game.run.drop_reset_charges, 0);
    }

    #[test]
    fn popping_prize_grants_one_expendable() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.board
            .set(CellCoord::new(0, 0), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(0, 1), Some(BubbleColor::Rose));

        let resolution = game.attach_color(CellCoord::new(1, 0), BubbleColor::Prize);
        let gained = game.run.color_call_charges
            + game.run.bomb_shot_charges
            + game.run.lightning_charges
            + game.run.drill_charges
            + game.run.drop_reset_charges;

        assert_eq!(resolution.popped.len(), 3);
        assert_eq!(gained, 1);
    }

    #[test]
    fn restart_starts_a_plain_run_without_upgrades() {
        let run = BubbleRun {
            queue_sight_level: 2,
            queue_sight_enabled: true,
            trace_sight_level: 1,
            trace_sight_enabled: true,
            color_sense_level: 2,
            color_sense_enabled: true,
            clean_start_level: 1,
            clean_start_enabled: true,
            bomb_shot_charges: 3,
            bomb_radius_level: 1,
            bomb_radius_enabled: true,
            bomb_cache_level: 1,
            bomb_cache_enabled: true,
            wild_cache_level: 1,
            wild_cache_enabled: true,
            prize_bubble_level: 1,
            prize_bubble_enabled: true,
            extra_rows_level: 1,
            extra_rows_enabled: true,
            drop_haste_level: 1,
            drop_haste_enabled: true,
            pop_drop_level: 1,
            pop_drop_enabled: true,
            lightning_power_level: 1,
            lightning_power_enabled: true,
            drill_power_level: 1,
            drill_power_enabled: true,
            reticle_level: 1,
            reticle_enabled: true,
            landing_dot_level: 1,
            landing_dot_enabled: true,
            ..BubbleRun::default()
        };
        let mut game = BubbleGame::with_run(7, 0, false, run);

        game.restart();

        assert_eq!(game.run.queue_sight_level, 0);
        assert!(!game.run.queue_sight_enabled);
        assert_eq!(game.run.trace_sight_level, 0);
        assert!(!game.run.trace_sight_enabled);
        assert_eq!(game.run.color_sense_level, 0);
        assert!(!game.run.color_sense_enabled);
        assert_eq!(game.run.clean_start_level, 0);
        assert!(!game.run.clean_start_enabled);
        assert_eq!(game.run.bomb_shot_charges, 0);
        assert_eq!(game.run.bomb_radius_level, 0);
        assert!(!game.run.bomb_radius_enabled);
        assert_eq!(game.run.bomb_cache_level, 0);
        assert!(!game.run.bomb_cache_enabled);
        assert_eq!(game.run.wild_cache_level, 0);
        assert!(!game.run.wild_cache_enabled);
        assert_eq!(game.run.prize_bubble_level, 0);
        assert!(!game.run.prize_bubble_enabled);
        assert_eq!(game.run.extra_rows_level, 0);
        assert!(!game.run.extra_rows_enabled);
        assert_eq!(game.run.drop_haste_level, 0);
        assert!(!game.run.drop_haste_enabled);
        assert_eq!(game.run.pop_drop_level, 0);
        assert!(!game.run.pop_drop_enabled);
        assert_eq!(game.run.lightning_power_level, 0);
        assert!(!game.run.lightning_power_enabled);
        assert_eq!(game.run.drill_power_level, 0);
        assert!(!game.run.drill_power_enabled);
        assert_eq!(game.run.reticle_level, 0);
        assert!(!game.run.reticle_enabled);
        assert_eq!(game.run.landing_dot_level, 0);
        assert!(!game.run.landing_dot_enabled);
    }

    #[test]
    fn next_round_keeps_the_current_upgraded_run() {
        let run = BubbleRun {
            queue_sight_level: 2,
            queue_sight_enabled: true,
            trace_sight_level: 1,
            trace_sight_enabled: true,
            color_sense_level: 2,
            color_sense_enabled: true,
            drop_reserve_level: 1,
            drop_reserve_enabled: true,
            revive_charm_level: 1,
            revive_charm_enabled: true,
            storm_focus_level: 1,
            storm_focus_enabled: true,
            bomb_cache_level: 1,
            bomb_cache_enabled: true,
            wild_cache_level: 1,
            wild_cache_enabled: true,
            bomb_shot_charges: 3,
            prize_bubble_level: 1,
            prize_bubble_enabled: true,
            ..BubbleRun::default()
        };
        let mut game = BubbleGame::with_run(7, 0, false, run);

        game.start_next_round();

        assert_eq!(game.run.round, 2);
        assert_eq!(game.run.queue_sight_level, 2);
        assert!(game.run.queue_sight_enabled);
        assert_eq!(game.run.color_call_charges, 0);
        assert_eq!(game.run.drop_reset_charges, 0);
        assert_eq!(game.run.revive_charges, 0);
        assert_eq!(game.run.lightning_charges, 0);
        assert_eq!(game.run.bomb_shot_charges, 3);
        assert_eq!(game.run.wild_shot_charges, 0);
        assert_eq!(game.run.prize_bubble_level, 1);
        assert!(game.run.prize_bubble_enabled);
    }

    #[test]
    fn bomb_radius_upgrade_extends_blast_reach() {
        let run = BubbleRun {
            bomb_radius_level: 1,
            bomb_radius_enabled: true,
            ..BubbleRun::default()
        };
        let mut game = BubbleGame::with_run(7, 0, false, run);
        game.board = Board::empty();
        game.board
            .set(CellCoord::new(3, 4), Some(BubbleColor::Rose));
        game.board.set(CellCoord::new(3, 6), Some(BubbleColor::Sky));

        let resolution = game.attach_color(CellCoord::new(3, 3), BubbleColor::Bomb);

        assert!(resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(3, 6)));
    }

    #[test]
    fn drill_from_contacted_bubble_clears_a_tunnel() {
        let run = BubbleRun {
            drill_power_level: 1,
            drill_power_enabled: true,
            ..BubbleRun::default()
        };
        let mut game = BubbleGame::with_run(7, 0, false, run);
        game.board = Board::empty();
        for row in 2..8 {
            game.board
                .set(CellCoord::new(row, 4), Some(BubbleColor::Rose));
        }

        let resolution = game.drill_from(CellCoord::new(2, 4));

        assert_eq!(resolution.popped.len(), 6);
        for row in 2..8 {
            assert!(game.board.is_empty(CellCoord::new(row, 4)));
        }
    }

    #[test]
    fn drill_through_clears_only_the_punch_path() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        let path = [
            CellCoord::new(2, 4),
            CellCoord::new(3, 4),
            CellCoord::new(4, 5),
            CellCoord::new(5, 5),
        ];
        for coord in path {
            game.board.set(coord, Some(BubbleColor::Rose));
        }
        game.board
            .set(CellCoord::new(0, 5), Some(BubbleColor::Gold));
        game.board
            .set(CellCoord::new(1, 5), Some(BubbleColor::Gold));
        game.board
            .set(CellCoord::new(2, 5), Some(BubbleColor::Gold));

        let resolution = game.drill_through(path);

        assert_eq!(resolution.popped.len(), 4);
        for coord in path {
            assert!(game.board.is_empty(coord));
        }
        assert_eq!(
            game.board.get(CellCoord::new(2, 5)),
            Some(BubbleColor::Gold)
        );
    }

    #[test]
    fn scout_kit_completion_adds_an_extra_queue_slot() {
        let incomplete_run = BubbleRun {
            queue_sight_level: 1,
            queue_sight_enabled: true,
            ..BubbleRun::default()
        };
        let complete_run = BubbleRun {
            queue_sight_level: 1,
            queue_sight_enabled: true,
            bank_sight_level: 1,
            bank_sight_enabled: true,
            trace_sight_level: 1,
            trace_sight_enabled: true,
            reticle_level: 1,
            reticle_enabled: true,
            landing_dot_level: 1,
            landing_dot_enabled: true,
            ..BubbleRun::default()
        };

        let incomplete = BubbleGame::with_run(7, 0, false, incomplete_run);
        let complete = BubbleGame::with_run(7, 0, false, complete_run);

        assert_eq!(incomplete.queue.len(), 5);
        assert_eq!(complete.queue.len(), 6);
        assert_eq!(complete.next_colors().len(), 5);
    }

    #[test]
    fn active_levels_can_be_lower_than_purchased_levels() {
        let run = BubbleRun {
            queue_sight_level: 3,
            queue_sight_active_level: 1,
            queue_sight_enabled: true,
            bank_sight_level: 1,
            bank_sight_active_level: 1,
            bank_sight_enabled: true,
            trace_sight_level: 1,
            trace_sight_active_level: 1,
            trace_sight_enabled: true,
            reticle_level: 1,
            reticle_active_level: 1,
            reticle_enabled: true,
            landing_dot_level: 1,
            landing_dot_active_level: 1,
            landing_dot_enabled: true,
            drop_delay_level: 3,
            drop_delay_active_level: 1,
            drop_delay_enabled: true,
            clean_start_level: 1,
            clean_start_active_level: 1,
            clean_start_enabled: true,
            ..BubbleRun::default()
        };
        let game = BubbleGame::with_run(7, 0, false, run);

        assert_eq!(game.queue.len(), 6);
        assert_eq!(game.shots_until_drop, SHOTS_PER_DROP + 1);
    }

    #[test]
    fn enabled_saved_upgrades_normalize_to_full_active_level() {
        let run = BubbleRun {
            queue_sight_level: 3,
            queue_sight_enabled: true,
            drop_delay_level: 2,
            drop_delay_enabled: true,
            ..BubbleRun::default()
        }
        .normalized();

        assert_eq!(run.queue_sight_active_level, 3);
        assert_eq!(run.active_queue_sight_level(), 3);
        assert_eq!(run.drop_delay_active_level, 2);
        assert_eq!(run.active_drop_delay_level(), 2);
    }

    #[test]
    fn risk_kit_completion_adds_one_more_shot_before_drops() {
        let run = BubbleRun {
            drop_delay_level: 1,
            drop_delay_enabled: true,
            clean_start_level: 1,
            clean_start_enabled: true,
            pop_drop_level: 1,
            pop_drop_enabled: true,
            ..BubbleRun::default()
        };
        let game = BubbleGame::with_run(7, 0, false, run);

        assert_eq!(game.shots_until_drop, SHOTS_PER_DROP + 2);
    }

    #[test]
    fn clean_start_reduces_later_round_opening_rows() {
        let run = BubbleRun {
            clean_start_level: 2,
            clean_start_enabled: true,
            ..BubbleRun::default()
        };
        let game = BubbleGame::with_run(7, 0, false, run);

        assert_eq!(game.board.occupied_cells().count(), (5 - 2) * COLS);
    }

    #[test]
    fn breach_kit_completion_improves_fuse_row_odds() {
        let incomplete_run = BubbleRun {
            bomb_drop_level: 1,
            bomb_drop_enabled: true,
            ..BubbleRun::default()
        };
        let complete_run = BubbleRun {
            bomb_drop_level: 1,
            bomb_drop_enabled: true,
            bomb_radius_level: 1,
            bomb_radius_enabled: true,
            storm_focus_level: 1,
            storm_focus_enabled: true,
            bomb_cache_level: 1,
            bomb_cache_enabled: true,
            lightning_power_level: 1,
            lightning_power_enabled: true,
            drill_power_level: 1,
            drill_power_enabled: true,
            ..BubbleRun::default()
        };

        assert_eq!(incomplete_run.bomb_drop_chance_per_mille(), 90);
        assert_eq!(complete_run.bomb_drop_chance_per_mille(), 135);
    }

    #[test]
    fn prize_row_odds_follow_active_level() {
        let incomplete_run = BubbleRun {
            prize_bubble_level: 1,
            prize_bubble_enabled: true,
            ..BubbleRun::default()
        };
        let complete_run = BubbleRun {
            wild_orb_level: 1,
            wild_orb_enabled: true,
            prize_bubble_level: 1,
            prize_bubble_enabled: true,
            prize_quality_level: 1,
            prize_quality_enabled: true,
            wild_cache_level: 1,
            wild_cache_enabled: true,
            ..BubbleRun::default()
        };

        assert_eq!(incomplete_run.prize_chance_per_mille(), 100);
        assert_eq!(complete_run.prize_chance_per_mille(), 100);
    }

    #[test]
    fn leveled_wild_shot_pops_every_adjacent_match() {
        let run = BubbleRun {
            wild_cache_level: 2,
            wild_cache_enabled: true,
            ..BubbleRun::default()
        };
        let mut game = BubbleGame::with_run(7, 0, false, run);
        game.board = Board::empty();
        game.board
            .set(CellCoord::new(2, 3), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(2, 2), Some(BubbleColor::Rose));
        game.board
            .set(CellCoord::new(4, 4), Some(BubbleColor::Gold));
        game.board
            .set(CellCoord::new(5, 4), Some(BubbleColor::Gold));

        let resolution = game.attach_color(CellCoord::new(3, 3), BubbleColor::Wild);

        assert_eq!(resolution.popped.len(), 5);
        assert!(resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(2, 2)));
        assert!(resolution
            .popped
            .iter()
            .any(|(coord, _)| *coord == CellCoord::new(5, 4)));
    }

    #[test]
    fn prize_bubbles_do_not_spawn_in_the_shot_queue() {
        let mut run = BubbleRun {
            prize_bubble_level: 3,
            prize_bubble_enabled: true,
            ..BubbleRun::default()
        };
        run.queue_sight_level = 3;
        run.queue_sight_enabled = true;
        let game = BubbleGame::with_run(7, 0, false, run);

        assert_ne!(game.current_color(), BubbleColor::Prize);
        assert!(!game.next_colors().contains(&BubbleColor::Prize));
    }

    #[test]
    fn prize_bubbles_can_spawn_in_dropped_rows() {
        let mut found_prize = false;

        for seed in 1..500 {
            let run = BubbleRun {
                prize_bubble_level: 3,
                prize_bubble_enabled: true,
                ..BubbleRun::default()
            };
            let mut game = BubbleGame::with_run(seed, 0, false, run);
            game.shots_until_drop = 1;
            game.advance_ceiling();

            if (0..COLS)
                .any(|col| game.board.get(CellCoord::new(0, col)) == Some(BubbleColor::Prize))
            {
                found_prize = true;
                break;
            }
        }

        assert!(found_prize);
    }

    #[test]
    fn saved_active_game_round_trips_without_restoring_saved_high_score() {
        let mut game = BubbleGame::new(7, 900);
        game.score = 120;
        game.hold();

        let json = serde_json::to_string(&game.save_state()).unwrap();
        let save = serde_json::from_str::<SavedBubbleGame>(&json).unwrap();
        let restored = BubbleGame::from_save(save, 30, false).unwrap();

        assert_eq!(restored.score, 120);
        assert_eq!(restored.high_score, 120);
        assert_eq!(restored.held, game.held);
        assert_eq!(restored.current_color(), game.current_color());
    }

    #[test]
    fn saved_terminal_games_cannot_resume() {
        let mut game = BubbleGame::new(7, 0);
        game.game_over = true;

        assert!(BubbleGame::from_save(game.save_state(), 0, false).is_none());

        let mut cleared = BubbleGame::new(7, 0);
        cleared.board = Board::empty();

        assert!(BubbleGame::from_save(cleared.save_state(), 0, false).is_none());
    }

    #[test]
    fn saved_won_boards_resume_to_shop() {
        let mut game = BubbleGame::new(7, 0);
        game.board = Board::empty();
        game.game_over = true;
        game.score = 400;
        game.run.shop_awarded_round = 1;

        let restored = BubbleGame::from_save(game.save_state(), 0, false).unwrap();

        assert!(restored.game_over);
        assert!(restored.board.is_cleared());
        assert_eq!(restored.score, 400);
    }

    #[test]
    fn saved_games_resume_only_in_matching_mode() {
        let hard = BubbleGame::hard(7, 0).save_state();

        assert!(BubbleGame::from_save(hard.clone(), 0, true).is_some());
        assert!(BubbleGame::from_save(hard, 0, false).is_none());
    }

    #[test]
    fn saved_games_migrate_live_shorter_boards_to_current_height() {
        let mut game = BubbleGame::new(7, 0);
        game.board
            .set(CellCoord::new(12, 3), Some(BubbleColor::Rose));
        game.score = 420;
        game.run.queue_sight_level = 1;
        game.run.queue_sight_enabled = true;

        let mut raw = serde_json::to_value(game.save_state()).unwrap();
        raw["version"] = serde_json::json!(LIVE_SAVE_VERSION);
        raw["board"]["cells"].as_array_mut().unwrap().truncate(13);

        let save = serde_json::from_value::<SavedBubbleGame>(raw).unwrap();
        let restored = BubbleGame::from_save(save, 0, false).unwrap();

        assert_eq!(
            restored.board.get(CellCoord::new(12, 3)),
            Some(BubbleColor::Rose)
        );
        assert!(restored.board.is_empty(CellCoord::new(ROWS - 1, 3)));
        assert_eq!(restored.score, 420);
        assert_eq!(restored.run.queue_sight_level, 1);
    }

    #[test]
    fn saved_games_require_between_moves_queue() {
        let mut save = BubbleGame::new(7, 0).save_state();
        save.queue.truncate(1);

        assert!(BubbleGame::from_save(save, 0, false).is_none());
    }

    #[test]
    fn saved_games_allow_full_scout_queue() {
        let run = BubbleRun {
            queue_sight_level: 3,
            queue_sight_enabled: true,
            bank_sight_level: 1,
            bank_sight_enabled: true,
            trace_sight_level: 1,
            trace_sight_enabled: true,
            reticle_level: 1,
            reticle_enabled: true,
            landing_dot_level: 1,
            landing_dot_enabled: true,
            ..BubbleRun::default()
        };
        let game = BubbleGame::with_run(7, 0, false, run);
        let save = game.save_state();

        assert_eq!(save.queue.len(), 8);
        assert!(BubbleGame::from_save(save, 0, false).is_some());
    }

    #[test]
    fn discarded_shot_restores_saveable_queue() {
        let mut game = BubbleGame::new(7, 0);
        let expected_current = game.next_colors()[0];
        game.pop_next();
        assert_eq!(game.current_color(), expected_current);
        assert_eq!(game.queue.len(), 3);

        game.finish_discarded_shot();

        assert_eq!(game.queue.len(), 4);
        assert!(BubbleGame::from_save(game.save_state(), 0, false).is_some());
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
