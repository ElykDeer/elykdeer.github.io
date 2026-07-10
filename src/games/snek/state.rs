#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{HashSet, VecDeque};
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use super::navigation::{
    direction_out_of_station, idle_direction, toroidal_distance, unique_directions,
    NavigationCache, PathMode,
};

pub(super) const DEFAULT_COLS: usize = 40;
pub(super) const DEFAULT_ROWS: usize = 28;
pub(super) const MIN_SNEK_LEN: usize = 3;
pub(super) const AUTO_SNEK_COST: usize = 10;
pub(super) const SPLITTER_STATION_UNLOCK_COST: usize = 200;
pub(super) const SPLITTER_STATION_COST: usize = 20;
pub(super) const APPLE_COST: usize = 500;
pub(super) const SPLITTER_STATION_COLS: usize = 8;
pub(super) const SPLITTER_STATION_ROWS: usize = 5;
pub(super) const BUILDING_BUFFER: usize = 5;
const SNEK_SAVE_VERSION: u8 = 3;
const SPLITTER_FEED_STEPS: u8 = MIN_SNEK_LEN as u8;
const SPLITTER_CUT_STEPS: u8 = 10;
const MAX_WORLD_DIMENSION: usize = 1_024;
const MAX_WORLD_CELLS: usize = MAX_WORLD_DIMENSION * MAX_WORLD_DIMENSION;
const MAX_SNEKS: usize = 1_024;
const MAX_SNEK_LEN: usize = 5_000;
const MAX_NAVIGATION_FIELD_CELLS: usize = 4 * 1_024 * 1_024;

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SavedSnekGame {
    version: u8,
    #[serde(default)]
    cols: usize,
    #[serde(default)]
    rows: usize,
    #[serde(default)]
    sneks: Vec<SavedSnek>,
    #[serde(default)]
    apples: Vec<Cell>,
    #[serde(default)]
    auto_snek_unlocked: bool,
    #[serde(default)]
    splitter_unlocked: bool,
    #[serde(default)]
    splitter_stations: Vec<SplitterStation>,
    #[serde(default)]
    splitter_jobs: Vec<SplitterJob>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SavedSnek {
    id: usize,
    direction: Direction,
    positions: Vec<Cell>,
    len: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SnekGame {
    cols: usize,
    rows: usize,
    sneks: Vec<Snek>,
    apples: Vec<Cell>,
    auto_snek_unlocked: bool,
    splitter_unlocked: bool,
    splitter_stations: Vec<SplitterStation>,
    splitter_jobs: Vec<SplitterJob>,
    manual_snek: Option<usize>,
    wiggle_t: f64,
}

#[derive(Clone, Debug)]
pub(super) struct Snek {
    id: usize,
    direction: Direction,
    positions: VecDeque<Cell>,
    len: usize,
    exit_override: Option<Direction>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SplitterActivity {
    station: SplitterStation,
    phase: SplitterActivityPhase,
    progress: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitterActivityPhase {
    Feeding,
    Cutting,
    Releasing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SplitterJob {
    source_snek_id: usize,
    phase: SplitterJobPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum SplitterJobPhase {
    Queued,
    Routing {
        snek_id: usize,
        station: SplitterStation,
    },
    Feeding {
        snek_id: usize,
        station: SplitterStation,
        advanced: u8,
    },
    Cutting {
        snek_id: usize,
        station: SplitterStation,
        elapsed: u8,
    },
    Releasing {
        old_snek_id: usize,
        new_snek_id: usize,
        station: SplitterStation,
    },
}

#[derive(Clone, Debug)]
struct SplitterDispatch {
    station_by_snek: Vec<Option<SplitterStation>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SplitterStation {
    #[serde(default)]
    id: usize,
    origin: Cell,
    rotation: u8,
    design: StationDesign,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum StationDesign {
    Gate,
    Saw,
    Press,
    Butterfly,
    Mantis,
}

impl StationDesign {
    pub(super) const ALL: [Self; 5] = [
        Self::Gate,
        Self::Saw,
        Self::Press,
        Self::Butterfly,
        Self::Mantis,
    ];
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Lcg {
    state: u64,
}

impl SavedSnekGame {
    fn is_valid(&self, cols: usize, rows: usize) -> bool {
        let Some(world_cells) = cols.checked_mul(rows) else {
            return false;
        };
        if self.apples.len() > MAX_SNEKS
            || world_cells
                .checked_mul(self.sneks.len())
                .is_none_or(|cells| cells > MAX_NAVIGATION_FIELD_CELLS)
        {
            return false;
        }

        let mut snek_ids = HashSet::new();
        let mut total_segments = 0usize;
        if !self.sneks.iter().all(|snek| {
            snek.id > 0
                && snek_ids.insert(snek.id)
                && (MIN_SNEK_LEN..=MAX_SNEK_LEN).contains(&snek.len)
                && snek.positions.len() == snek.len
                && snek
                    .positions
                    .iter()
                    .all(|cell| cell.col < cols && cell.row < rows)
                && total_segments.checked_add(snek.len).is_some_and(|total| {
                    total_segments = total;
                    total <= MAX_SNEKS * MAX_SNEK_LEN
                })
        }) || !snek_ids.contains(&1)
        {
            return false;
        }

        if !self
            .apples
            .iter()
            .all(|cell| cell.col < cols && cell.row < rows)
        {
            return false;
        }

        let mut station_ids = HashSet::new();
        if self.splitter_stations.len() > MAX_SNEKS
            || !self.splitter_stations.iter().all(|station| {
                station.id > 0
                    && station_ids.insert(station.id)
                    && station.valid_geometry(cols, rows)
            })
            || self
                .splitter_stations
                .iter()
                .enumerate()
                .any(|(index, left)| {
                    self.splitter_stations[index + 1..]
                        .iter()
                        .any(|right| stations_overlap(*left, *right))
                })
        {
            return false;
        }

        let mut active_sneks = HashSet::new();
        let mut active_stations = HashSet::new();
        self.splitter_jobs.len() <= MAX_SNEKS
            && self.splitter_jobs.iter().all(|job| {
                job.is_valid_ids(&snek_ids, &self.splitter_stations)
                    && job
                        .active_snek_ids()
                        .into_iter()
                        .flatten()
                        .all(|id| active_sneks.insert(id))
                    && job
                        .station()
                        .is_none_or(|station| active_stations.insert(station.id))
            })
    }
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
        let mut game = Self {
            cols,
            rows,
            sneks: vec![Snek::new(1, center)],
            apples: Vec::new(),
            auto_snek_unlocked: false,
            splitter_unlocked: false,
            splitter_stations: Vec::new(),
            splitter_jobs: Vec::new(),
            manual_snek: Some(1),
            wiggle_t: 0.0,
        };
        game.ensure_apples(&mut Lcg::new(seed));
        game
    }

    pub(super) fn from_save(save: SavedSnekGame, cols: usize, rows: usize) -> Option<Self> {
        if save.version != SNEK_SAVE_VERSION
            || save.cols == 0
            || save.rows == 0
            || save.cols > MAX_WORLD_DIMENSION
            || save.rows > MAX_WORLD_DIMENSION
            || cols > MAX_WORLD_DIMENSION
            || rows > MAX_WORLD_DIMENSION
            || save.cols.checked_mul(save.rows)? > MAX_WORLD_CELLS
            || save.sneks.is_empty()
            || save.sneks.len() > MAX_SNEKS
        {
            return None;
        }

        let saved_cols = save.cols;
        let saved_rows = save.rows;
        if !save.is_valid(saved_cols, saved_rows) {
            return None;
        }

        let cols = cols.max(saved_cols).max(1);
        let rows = rows.max(saved_rows).max(1);
        let sneks: Vec<_> = save.sneks.into_iter().map(Snek::from_save).collect();
        let splitter_stations = save.splitter_stations;
        let splitter_jobs: Vec<_> = save
            .splitter_jobs
            .into_iter()
            .filter(|job| job.is_valid(&sneks, &splitter_stations))
            .collect();
        let splitter_unlocked =
            save.splitter_unlocked || !splitter_stations.is_empty() || !splitter_jobs.is_empty();
        let mut game = Self {
            cols,
            rows,
            sneks,
            apples: save.apples,
            auto_snek_unlocked: save.auto_snek_unlocked,
            splitter_unlocked,
            splitter_stations,
            splitter_jobs: if splitter_unlocked {
                splitter_jobs
            } else {
                Vec::new()
            },
            manual_snek: if save.auto_snek_unlocked {
                None
            } else {
                Some(1)
            },
            wiggle_t: 0.0,
        };
        let mut rng = Lcg::new(1);
        game.ensure_apples(&mut rng);
        Some(game)
    }

    pub(super) fn save(&self) -> SavedSnekGame {
        SavedSnekGame {
            version: SNEK_SAVE_VERSION,
            cols: self.cols,
            rows: self.rows,
            sneks: self.sneks.iter().map(Snek::save).collect(),
            apples: self.apples.clone(),
            auto_snek_unlocked: self.auto_snek_unlocked,
            splitter_unlocked: self.splitter_unlocked,
            splitter_stations: self.splitter_stations.clone(),
            splitter_jobs: self.splitter_jobs.clone(),
        }
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let (next_cols, next_rows) = self.required_world_size(cols, rows);
        let target_apple_count = self.apples.len().max(1);
        if self.cols == next_cols && self.rows == next_rows {
            self.remove_blocked_apples();
            self.ensure_apple_count_without_rng(target_apple_count);
            return;
        }
        self.cols = next_cols;
        self.rows = next_rows;
        self.remove_blocked_apples();
        self.ensure_apple_count_without_rng(target_apple_count);
    }

    pub(super) fn step(&mut self, rng: &mut Lcg) {
        self.advance_cutting_jobs();
        self.finish_releasing_jobs();
        self.assign_splitter_jobs();
        let splitter_dispatch = self.splitter_dispatch();
        if self.auto_snek_unlocked {
            self.update_autopilot_directions(&splitter_dispatch);
        }

        let initial_snek_count = self.sneks.len();
        for index in 0..initial_snek_count {
            let snek_id = self.sneks[index].id;
            if self.snek_is_cutting(snek_id) {
                continue;
            }
            let old_head = self.sneks[index].head();
            let next = self.next_snek_cell(index, &splitter_dispatch);
            let ate_apple = self
                .apples
                .iter()
                .position(|apple| *apple == next)
                .map(|apple_index| {
                    self.apples.remove(apple_index);
                })
                .is_some();
            self.sneks[index].step_to(next, ate_apple);
            self.advance_job_after_move(snek_id, old_head, next);
        }

        self.finish_releasing_jobs();
        self.ensure_apples(rng);
        self.tick_wiggle();
    }

    pub(super) fn set_direction(&mut self, direction: Direction) {
        let Some(id) = self.controlled_snek_id() else {
            return;
        };
        if let Some(snek) = self.sneks.iter_mut().find(|snek| snek.id == id) {
            snek.direction = direction;
        }
    }

    pub(super) fn tap_cell(&mut self, cell: Cell) {
        if !self.auto_snek_unlocked {
            return;
        }

        self.manual_snek = self
            .sneks
            .iter()
            .find(|snek| snek.contains_near(cell, self.cols, self.rows))
            .map(|snek| snek.id);
    }

    pub(super) fn buy_auto_snek(&mut self) -> bool {
        if !self.can_buy_auto_snek() || !self.spend_apls(AUTO_SNEK_COST) {
            return false;
        }
        self.auto_snek_unlocked = true;
        self.manual_snek = None;
        true
    }

    pub(super) fn buy_splitter_station(
        &mut self,
        origin: Cell,
        rotation: u8,
        design: StationDesign,
        rng: &mut Lcg,
    ) -> bool {
        let station = SplitterStation::new(origin, rotation, design, self.cols, self.rows)
            .with_id(self.next_station_id());
        if !self.can_place_splitter_station()
            || self
                .splitter_stations
                .iter()
                .any(|existing| stations_overlap(*existing, station))
            || !self.spend_apls(SPLITTER_STATION_COST)
        {
            return false;
        }
        let station = self.fit_station_with_buffer(station);
        self.splitter_stations.push(station);
        self.remove_blocked_apples();
        self.ensure_apples(rng);
        true
    }

    pub(super) fn move_splitter_station(
        &mut self,
        index: usize,
        origin: Cell,
        rotation: u8,
        rng: &mut Lcg,
    ) -> bool {
        let Some(station) = self.splitter_stations.get(index).copied() else {
            return false;
        };
        if self.station_busy(station) {
            return false;
        }
        let moved = SplitterStation::new(origin, rotation, station.design, self.cols, self.rows)
            .with_id(station.id);
        if self
            .splitter_stations
            .iter()
            .enumerate()
            .any(|(other_index, existing)| {
                other_index != index && stations_overlap(*existing, moved)
            })
        {
            return false;
        }
        let moved = self.fit_station_with_buffer(moved);
        self.splitter_stations[index] = moved;
        self.remove_blocked_apples();
        self.ensure_apples(rng);
        true
    }

    pub(super) fn delete_splitter_station(&mut self, index: usize) -> bool {
        let Some(station) = self.splitter_stations.get(index).copied() else {
            return false;
        };
        if self.station_busy(station) {
            return false;
        }
        self.splitter_stations.remove(index);
        if let Some(snek) = self.sneks.first_mut() {
            snek.add_segments(SPLITTER_STATION_COST);
        }
        true
    }

    pub(super) fn can_buy_auto_snek(&self) -> bool {
        !self.auto_snek_unlocked && self.apls() >= AUTO_SNEK_COST
    }

    pub(super) fn can_unlock_splitter_station(&self) -> bool {
        self.auto_snek_unlocked
            && !self.splitter_unlocked
            && self.spendable_apls() >= SPLITTER_STATION_UNLOCK_COST
    }

    pub(super) fn buy_splitter_unlock(&mut self) -> bool {
        if !self.can_unlock_splitter_station() || !self.spend_apls(SPLITTER_STATION_UNLOCK_COST) {
            return false;
        }
        self.splitter_unlocked = true;
        true
    }

    pub(super) fn splitter_unlocked(&self) -> bool {
        self.splitter_unlocked
    }

    pub(super) fn can_place_splitter_station(&self) -> bool {
        self.splitter_unlocked && self.spendable_apls() >= SPLITTER_STATION_COST
    }

    pub(super) fn apls(&self) -> usize {
        self.sneks.iter().map(Snek::excess_len).sum()
    }

    pub(super) fn sneks(&self) -> &[Snek] {
        &self.sneks
    }

    pub(super) fn apples(&self) -> &[Cell] {
        &self.apples
    }

    pub(super) fn cols(&self) -> usize {
        self.cols
    }

    pub(super) fn rows(&self) -> usize {
        self.rows
    }

    pub(super) fn can_buy_snek(&self) -> bool {
        self.auto_snek_unlocked
            && !self.splitter_stations.is_empty()
            && self.select_snek_for_order().is_some()
    }

    pub(super) fn buy_snek(&mut self) -> bool {
        let Some(source_snek_id) = self.select_snek_for_order() else {
            return false;
        };
        self.splitter_jobs.push(SplitterJob::queued(source_snek_id));
        true
    }

    pub(super) fn can_buy_apple(&self) -> bool {
        self.spendable_apls() >= APPLE_COST && self.has_open_apple_cell()
    }

    pub(super) fn buy_apple(&mut self, rng: &mut Lcg) -> bool {
        if !self.can_buy_apple() || !self.spend_apls(APPLE_COST) {
            return false;
        }
        self.add_apple(rng)
    }

    fn has_open_apple_cell(&self) -> bool {
        (0..self.rows)
            .any(|row| (0..self.cols).any(|col| self.apple_cell_open(Cell::new(col, row))))
    }

    fn add_apple(&mut self, rng: &mut Lcg) -> bool {
        for _ in 0..128 {
            let cell = random_cell(self.cols, self.rows, rng);
            if self.apple_cell_open(cell) {
                self.apples.push(cell);
                return true;
            }
        }
        self.add_first_open_apple()
    }

    fn apple_cell_open(&self, cell: Cell) -> bool {
        cell.col < self.cols
            && cell.row < self.rows
            && !self.cell_occupied(cell)
            && !self.apples.contains(&cell)
    }

    pub(super) fn auto_snek_unlocked(&self) -> bool {
        self.auto_snek_unlocked
    }

    pub(super) fn splitter_stations(&self) -> &[SplitterStation] {
        &self.splitter_stations
    }

    pub(super) fn station_index_at(&self, cell: Cell) -> Option<usize> {
        self.splitter_stations
            .iter()
            .position(|station| station.occupies(cell))
    }

    pub(super) fn manual_snek(&self) -> Option<usize> {
        self.manual_snek
    }

    pub(super) fn wiggle_t(&self) -> f64 {
        self.wiggle_t
    }

    pub(super) fn pending_snek_orders(&self) -> usize {
        self.splitter_jobs.len()
    }

    pub(super) fn splitter_activities(&self) -> Vec<SplitterActivity> {
        self.splitter_jobs
            .iter()
            .filter_map(|job| job.activity())
            .collect()
    }

    #[cfg(debug_assertions)]
    pub(super) fn debug_add_apls(&mut self, count: usize) {
        if let Some(snek) = self.sneks.first_mut() {
            snek.add_segments(count);
        }
    }

    fn controlled_snek_id(&self) -> Option<usize> {
        self.manual_snek.or_else(|| {
            if self.auto_snek_unlocked {
                None
            } else {
                self.sneks.first().map(|snek| snek.id)
            }
        })
    }

    fn update_autopilot_directions(&mut self, splitter_dispatch: &SplitterDispatch) {
        let assignments = self.apple_assignments();
        let mut navigation =
            NavigationCache::new(self.cols, self.rows, self.splitter_stations.clone());
        for (index, assignment) in assignments.into_iter().enumerate() {
            let snek = &self.sneks[index];
            if self.manual_snek == Some(snek.id) {
                continue;
            }
            let direction = if let Some(station) = splitter_dispatch.station_for_snek(index) {
                let pre_entry = station.pre_entry_cell(self.cols, self.rows);
                if snek.head() == pre_entry {
                    station.flow_direction()
                } else {
                    navigation.direction_toward(
                        snek.head(),
                        pre_entry,
                        snek.direction,
                        PathMode::AvoidStation,
                    )
                }
            } else if let Some(target) = assignment {
                navigation.direction_toward(
                    snek.head(),
                    target,
                    snek.direction,
                    PathMode::AvoidStation,
                )
            } else {
                idle_direction(
                    snek.id,
                    snek.head(),
                    self.cols,
                    self.rows,
                    snek.direction,
                    &self.splitter_stations,
                )
            };
            self.sneks[index].direction = direction;
        }
    }

    fn apple_assignments(&self) -> Vec<Option<Cell>> {
        let mut assignments = vec![None; self.sneks.len()];
        if self.apples.is_empty() {
            return assignments;
        }

        let mut pairs = Vec::new();
        for (snek_index, snek) in self.sneks.iter().enumerate() {
            if self.manual_snek == Some(snek.id) || self.snek_has_splitter_job(snek.id) {
                continue;
            }
            for (apple_index, apple) in self.apples.iter().copied().enumerate() {
                pairs.push((
                    toroidal_distance(snek.head(), apple, self.cols, self.rows),
                    snek_index,
                    apple_index,
                    apple,
                ));
            }
        }
        pairs.sort_by_key(|(distance, snek_index, apple_index, _)| {
            (*distance, *apple_index, *snek_index)
        });

        let mut used_sneks = vec![false; self.sneks.len()];
        let mut used_apples = vec![false; self.apples.len()];
        for (_, snek_index, apple_index, apple) in pairs {
            if used_sneks[snek_index] || used_apples[apple_index] {
                continue;
            }
            assignments[snek_index] = Some(apple);
            used_sneks[snek_index] = true;
            used_apples[apple_index] = true;
        }

        assignments
    }

    fn splitter_dispatch(&self) -> SplitterDispatch {
        SplitterDispatch::new(self)
    }

    fn reserved_segments_for_snek(&self, snek_id: usize) -> usize {
        self.splitter_jobs
            .iter()
            .filter(|job| job.source_snek_id == snek_id && job.reserves_segments())
            .count()
            * MIN_SNEK_LEN
    }

    fn station_busy(&self, station: SplitterStation) -> bool {
        self.splitter_jobs
            .iter()
            .any(|job| job.station().is_some_and(|active| active.id == station.id))
    }

    fn snek_has_splitter_job(&self, snek_id: usize) -> bool {
        self.splitter_jobs.iter().any(|job| job.uses_snek(snek_id))
    }

    fn snek_is_cutting(&self, snek_id: usize) -> bool {
        self.splitter_jobs.iter().any(|job| {
            matches!(
                job.phase,
                SplitterJobPhase::Cutting {
                    snek_id: id,
                    ..
                } if id == snek_id
            )
        })
    }

    fn assign_splitter_jobs(&mut self) {
        if self.splitter_stations.is_empty() {
            return;
        }
        let mut used_sneks = Vec::new();
        let mut used_stations = Vec::new();
        for job in &self.splitter_jobs {
            match job.phase {
                SplitterJobPhase::Queued => {}
                SplitterJobPhase::Routing { snek_id, station }
                | SplitterJobPhase::Feeding {
                    snek_id, station, ..
                }
                | SplitterJobPhase::Cutting {
                    snek_id, station, ..
                } => {
                    used_sneks.push(snek_id);
                    used_stations.push(station);
                }
                SplitterJobPhase::Releasing {
                    old_snek_id,
                    new_snek_id,
                    station,
                } => {
                    used_sneks.extend([old_snek_id, new_snek_id]);
                    used_stations.push(station);
                }
            }
        }

        let queued_jobs: Vec<_> = self
            .splitter_jobs
            .iter()
            .enumerate()
            .filter_map(|(index, job)| (job.phase == SplitterJobPhase::Queued).then_some(index))
            .collect();
        for job_index in queued_jobs {
            let source_snek_id = self.splitter_jobs[job_index].source_snek_id;
            let mut best = None;
            for snek in &self.sneks {
                if snek.id != source_snek_id
                    || self.manual_snek == Some(snek.id)
                    || snek.excess_len() < MIN_SNEK_LEN
                    || used_sneks.contains(&snek.id)
                {
                    continue;
                }
                for station in self.splitter_stations.iter().copied() {
                    if used_stations.contains(&station) {
                        continue;
                    }
                    let key = (
                        toroidal_distance(
                            snek.head(),
                            station.pre_entry_cell(self.cols, self.rows),
                            self.cols,
                            self.rows,
                        ),
                        station.origin.row,
                        station.origin.col,
                        station.id,
                        snek.id,
                    );
                    if best.is_none_or(|(current, _)| key < current) {
                        best = Some((key, station));
                    }
                }
            }
            let Some(((_, _, _, _, snek_id), station)) = best else {
                break;
            };
            self.splitter_jobs[job_index].phase = SplitterJobPhase::Routing { snek_id, station };
            used_sneks.push(snek_id);
            used_stations.push(station);
        }
    }

    fn advance_job_after_move(&mut self, snek_id: usize, old_head: Cell, next: Cell) {
        let Some(job) = self
            .splitter_jobs
            .iter_mut()
            .find(|job| job.uses_snek(snek_id))
        else {
            return;
        };
        job.phase = match job.phase {
            SplitterJobPhase::Routing {
                snek_id: id,
                station,
            } if next == station.cut_target() && old_head != station.cut_target() => {
                SplitterJobPhase::Feeding {
                    snek_id: id,
                    station,
                    advanced: 0,
                }
            }
            SplitterJobPhase::Feeding {
                snek_id: id,
                station,
                advanced,
            } => {
                let advanced = advanced + 1;
                if advanced >= SPLITTER_FEED_STEPS {
                    SplitterJobPhase::Cutting {
                        snek_id: id,
                        station,
                        elapsed: 0,
                    }
                } else {
                    SplitterJobPhase::Feeding {
                        snek_id: id,
                        station,
                        advanced,
                    }
                }
            }
            phase => phase,
        };
    }

    fn advance_cutting_jobs(&mut self) {
        let mut completed = Vec::new();
        for (index, job) in self.splitter_jobs.iter_mut().enumerate() {
            let SplitterJobPhase::Cutting {
                snek_id,
                station,
                elapsed,
            } = job.phase
            else {
                continue;
            };
            if elapsed + 1 >= SPLITTER_CUT_STEPS {
                completed.push((index, snek_id, station));
            } else {
                job.phase = SplitterJobPhase::Cutting {
                    snek_id,
                    station,
                    elapsed: elapsed + 1,
                };
            }
        }
        for (job_index, snek_id, station) in completed {
            self.complete_splitter_job(job_index, snek_id, station);
        }
    }

    fn complete_splitter_job(
        &mut self,
        job_index: usize,
        old_snek_id: usize,
        station: SplitterStation,
    ) {
        let Some(snek_index) = self.sneks.iter().position(|snek| snek.id == old_snek_id) else {
            self.splitter_jobs[job_index].phase = SplitterJobPhase::Queued;
            return;
        };
        let new_snek_id = self.next_snek_id();
        let Some(new_snek) =
            self.sneks[snek_index].split_head(new_snek_id, station.flow_direction())
        else {
            self.splitter_jobs[job_index].phase = SplitterJobPhase::Queued;
            return;
        };
        self.sneks[snek_index].direction = station.old_exit_direction();
        self.sneks[snek_index].exit_override = Some(station.old_exit_direction());
        self.sneks.push(new_snek);
        self.splitter_jobs[job_index].phase = SplitterJobPhase::Releasing {
            old_snek_id,
            new_snek_id,
            station,
        };
    }

    fn finish_releasing_jobs(&mut self) {
        let sneks = &self.sneks;
        self.splitter_jobs.retain(|job| {
            let SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                station,
            } = job.phase
            else {
                return true;
            };
            [old_snek_id, new_snek_id].into_iter().any(|id| {
                sneks
                    .iter()
                    .find(|snek| snek.id == id)
                    .is_some_and(|snek| snek.intersects_station(station))
            })
        });
    }

    fn select_snek_for_order(&self) -> Option<usize> {
        self.sneks
            .iter()
            .filter_map(|snek| {
                let available = snek
                    .excess_len()
                    .saturating_sub(self.reserved_segments_for_snek(snek.id));
                if available < MIN_SNEK_LEN {
                    return None;
                }
                let distance = self
                    .splitter_stations
                    .iter()
                    .map(|station| {
                        toroidal_distance(snek.head(), station.entry_cell(), self.cols, self.rows)
                    })
                    .min()?;
                Some((distance, usize::MAX - available, snek.id))
            })
            .min()
            .map(|(_, _, snek_id)| snek_id)
    }

    fn ensure_apples(&mut self, rng: &mut Lcg) {
        let target_count = self.apples.len().max(1);
        self.remove_blocked_apples();
        while self.apples.len() < target_count && self.add_apple(rng) {}
    }

    fn ensure_apple_count_without_rng(&mut self, target_count: usize) {
        self.remove_blocked_apples();
        while self.apples.len() < target_count && self.add_first_open_apple() {}
    }

    fn add_first_open_apple(&mut self) -> bool {
        let total = self.cols.saturating_mul(self.rows);
        if total == 0 {
            return false;
        }
        let start = (self.rows / 2) * self.cols + self.cols / 2;
        for offset in 0..total {
            let index = (start + offset) % total;
            let cell = Cell::new(index % self.cols, index / self.cols);
            if self.apple_cell_open(cell) {
                self.apples.push(cell);
                return true;
            }
        }
        false
    }

    fn spend_apls(&mut self, cost: usize) -> bool {
        if self.spendable_apls() < cost {
            return false;
        }

        let mut remaining = cost;
        while remaining > 0 {
            let Some(index) = self
                .sneks
                .iter()
                .enumerate()
                .filter(|(_, snek)| snek.excess_len() > self.reserved_segments_for_snek(snek.id))
                .max_by_key(|(_, snek)| {
                    snek.excess_len() - self.reserved_segments_for_snek(snek.id)
                })
                .map(|(index, _)| index)
            else {
                return false;
            };
            let spendable = self.sneks[index]
                .excess_len()
                .saturating_sub(self.reserved_segments_for_snek(self.sneks[index].id));
            let spend = spendable.min(remaining);
            self.sneks[index].remove_tail_segments(spend);
            remaining -= spend;
        }

        true
    }

    fn spendable_apls(&self) -> usize {
        self.sneks
            .iter()
            .map(|snek| {
                snek.excess_len()
                    .saturating_sub(self.reserved_segments_for_snek(snek.id))
            })
            .sum()
    }

    fn next_snek_cell(&mut self, index: usize, splitter_dispatch: &SplitterDispatch) -> Cell {
        let head = self.sneks[index].head();
        if let Some(station) = self.station_cutting(head) {
            if let Some(direction) = self.sneks[index].exit_override {
                if let Some((direction, next)) =
                    station_cut_step(head, station, direction, self.cols, self.rows)
                {
                    if !station.occupies(next) {
                        self.sneks[index].exit_override = None;
                    }
                    self.sneks[index].direction = direction;
                    return next;
                }
                self.sneks[index].exit_override = None;
            }
            let preferred = if station.main_lane_cell(head) {
                station.flow_direction()
            } else {
                station.old_exit_direction()
            };
            if let Some((direction, next)) =
                station_cut_step(head, station, preferred, self.cols, self.rows)
            {
                self.sneks[index].direction = direction;
                return next;
            }
            return head;
        }

        let next = self.sneks[index].next_cell(self.cols, self.rows);
        if self.cell_blocked(next) {
            if let Some(station) = self
                .splitter_stations
                .iter()
                .copied()
                .find(|station| station.occupies(head) && !station.cuts(head))
            {
                if let Some(direction) = direction_out_of_station(
                    head,
                    self.cols,
                    self.rows,
                    self.sneks[index].direction,
                    station,
                ) {
                    self.sneks[index].direction = direction;
                    return head.stepped(direction, self.cols, self.rows);
                }
            }
        }
        if self.can_enter_cutter(index, head, next, splitter_dispatch)
            || !self.cell_blocked(next)
            || self.station_escape_allowed(head, next)
        {
            return next;
        }

        let reverse = self.sneks[index].direction.reversed();
        self.sneks[index].direction = reverse;
        let bounced = self.sneks[index].next_cell(self.cols, self.rows);
        if self.cell_blocked(bounced)
            && !self.can_enter_cutter(index, head, bounced, splitter_dispatch)
            && !self.station_escape_allowed(head, bounced)
        {
            self.sneks[index].head()
        } else {
            bounced
        }
    }

    fn cell_occupied(&self, cell: Cell) -> bool {
        self.sneks.iter().any(|snek| snek.contains(cell))
            || self
                .splitter_stations
                .iter()
                .any(|station| station.occupies(cell))
    }

    fn cell_blocked(&self, cell: Cell) -> bool {
        self.splitter_stations
            .iter()
            .any(|station| station.occupies(cell))
    }

    fn station_escape_allowed(&self, from: Cell, to: Cell) -> bool {
        self.splitter_stations
            .iter()
            .copied()
            .any(|station| station.occupies(from) && !station.cuts(from) && !station.occupies(to))
    }

    fn station_cutting(&self, cell: Cell) -> Option<SplitterStation> {
        self.splitter_stations
            .iter()
            .copied()
            .find(|station| station.cuts(cell))
    }

    fn can_enter_cutter(
        &self,
        index: usize,
        from: Cell,
        cell: Cell,
        splitter_dispatch: &SplitterDispatch,
    ) -> bool {
        splitter_dispatch
            .station_for_snek(index)
            .is_some_and(|station| {
                station.cuts(cell)
                    && ((station.cuts(from) && station.cuts(cell))
                        || station.legal_entry_step(from, cell, self.cols, self.rows))
            })
            && self.sneks[index].excess_len() >= MIN_SNEK_LEN
    }

    fn next_snek_id(&self) -> usize {
        self.sneks.iter().map(|snek| snek.id).max().unwrap_or(0) + 1
    }

    fn next_station_id(&self) -> usize {
        self.splitter_stations
            .iter()
            .map(|station| station.id)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn required_world_size(&self, base_cols: usize, base_rows: usize) -> (usize, usize) {
        let mut cols = base_cols.max(1);
        let mut rows = base_rows.max(1);
        for snek in &self.sneks {
            for cell in snek.positions() {
                cols = cols.max(cell.col + 1);
                rows = rows.max(cell.row + 1);
            }
        }
        for station in &self.splitter_stations {
            cols = cols.max(station.origin.col + station.width() + BUILDING_BUFFER);
            rows = rows.max(station.origin.row + station.height() + BUILDING_BUFFER);
        }
        (cols, rows)
    }

    fn fit_station_with_buffer(&mut self, mut station: SplitterStation) -> SplitterStation {
        let shift_cols = BUILDING_BUFFER.saturating_sub(station.origin.col);
        let shift_rows = BUILDING_BUFFER.saturating_sub(station.origin.row);
        if shift_cols > 0 || shift_rows > 0 {
            self.shift_cells(shift_cols, shift_rows);
            station.origin.col += shift_cols;
            station.origin.row += shift_rows;
        }

        self.cols = self
            .cols
            .max(station.origin.col + station.width() + BUILDING_BUFFER);
        self.rows = self
            .rows
            .max(station.origin.row + station.height() + BUILDING_BUFFER);
        station
    }

    fn shift_cells(&mut self, cols: usize, rows: usize) {
        if cols == 0 && rows == 0 {
            return;
        }
        self.cols += cols;
        self.rows += rows;
        for snek in &mut self.sneks {
            snek.shift(cols, rows);
        }
        for apple in &mut self.apples {
            apple.col += cols;
            apple.row += rows;
        }
        for station in &mut self.splitter_stations {
            station.origin.col += cols;
            station.origin.row += rows;
        }
        for job in &mut self.splitter_jobs {
            job.shift_station(cols, rows);
        }
    }

    fn remove_blocked_apples(&mut self) {
        self.apples.retain(|apple| {
            apple.col < self.cols
                && apple.row < self.rows
                && !self
                    .splitter_stations
                    .iter()
                    .any(|station| station.occupies(*apple))
        });
    }

    fn tick_wiggle(&mut self) {
        self.wiggle_t += TAU / 9.0;
        self.wiggle_t %= TAU;
    }
}

impl SplitterStation {
    pub(super) fn new(
        origin: Cell,
        rotation: u8,
        design: StationDesign,
        _cols: usize,
        _rows: usize,
    ) -> Self {
        Self {
            id: 0,
            origin,
            rotation: rotation % 4,
            design,
        }
    }

    fn with_id(mut self, id: usize) -> Self {
        self.id = id;
        self
    }

    fn valid_geometry(self, cols: usize, rows: usize) -> bool {
        self.rotation < 4
            && self
                .origin
                .col
                .checked_add(self.width())
                .is_some_and(|end| end <= cols)
            && self
                .origin
                .row
                .checked_add(self.height())
                .is_some_and(|end| end <= rows)
    }

    pub(super) fn origin(self) -> Cell {
        self.origin
    }

    pub(super) fn rotation(self) -> u8 {
        self.rotation
    }

    pub(super) fn design(self) -> StationDesign {
        self.design
    }

    pub(super) fn width(self) -> usize {
        station_width(self.rotation)
    }

    pub(super) fn height(self) -> usize {
        station_height(self.rotation)
    }

    pub(super) fn contains(self, cell: Cell) -> bool {
        cell.col >= self.origin.col
            && cell.col < self.origin.col + self.width()
            && cell.row >= self.origin.row
            && cell.row < self.origin.row + self.height()
    }

    pub(super) fn cuts(self, cell: Cell) -> bool {
        let Some((row, col)) = self.local_cell(cell) else {
            return false;
        };
        station_cell(self.design, row, col, self.rotation) == Some('0')
    }

    pub(super) fn occupies(self, cell: Cell) -> bool {
        let Some((row, col)) = self.local_cell(cell) else {
            return false;
        };
        station_cell(self.design, row, col, self.rotation).is_some_and(|tile| tile != ' ')
    }

    pub(super) fn cut_target(self) -> Cell {
        let (row, col) = rotated_station_position(
            SPLITTER_STATION_ROWS / 2,
            SPLITTER_STATION_COLS / 2,
            self.rotation,
        );
        Cell::new(self.origin.col + col, self.origin.row + row)
    }

    pub(super) fn entry_cell(self) -> Cell {
        match self.flow_direction() {
            Direction::Right => {
                let row = self.origin.row + self.height() / 2;
                for col in self.origin.col..self.origin.col + self.width() {
                    let candidate = Cell::new(col, row);
                    if self.cuts(candidate) {
                        return candidate;
                    }
                }
            }
            Direction::Left => {
                let row = self.origin.row + self.height() / 2;
                for col in (self.origin.col..self.origin.col + self.width()).rev() {
                    let candidate = Cell::new(col, row);
                    if self.cuts(candidate) {
                        return candidate;
                    }
                }
            }
            Direction::Down => {
                let col = self.origin.col + self.width() / 2;
                for row in self.origin.row..self.origin.row + self.height() {
                    let candidate = Cell::new(col, row);
                    if self.cuts(candidate) {
                        return candidate;
                    }
                }
            }
            Direction::Up => {
                let col = self.origin.col + self.width() / 2;
                for row in (self.origin.row..self.origin.row + self.height()).rev() {
                    let candidate = Cell::new(col, row);
                    if self.cuts(candidate) {
                        return candidate;
                    }
                }
            }
        }
        self.cut_target()
    }

    fn pre_entry_cell(self, cols: usize, rows: usize) -> Cell {
        self.entry_cell()
            .stepped(self.flow_direction().reversed(), cols, rows)
    }

    pub(super) fn new_exit_cell(self) -> Cell {
        self.lane_edge(self.flow_direction())
    }

    pub(super) fn old_exit_cell(self) -> Cell {
        self.lane_edge(self.old_exit_direction())
    }

    pub(super) fn legal_entry_step(self, from: Cell, to: Cell, cols: usize, rows: usize) -> bool {
        to == self.entry_cell()
            && from
                == self
                    .entry_cell()
                    .stepped(self.flow_direction().reversed(), cols, rows)
    }

    pub(super) fn flow_direction(self) -> Direction {
        match self.rotation % 4 {
            1 => Direction::Down,
            2 => Direction::Left,
            3 => Direction::Up,
            _ => Direction::Right,
        }
    }

    pub(super) fn old_exit_direction(self) -> Direction {
        self.flow_direction().clockwise()
    }

    fn main_lane_cell(self, cell: Cell) -> bool {
        if !self.cuts(cell) {
            return false;
        }
        match self.flow_direction() {
            Direction::Left | Direction::Right => cell.row == self.origin.row + self.height() / 2,
            Direction::Up | Direction::Down => cell.col == self.origin.col + self.width() / 2,
        }
    }

    fn local_cell(self, cell: Cell) -> Option<(usize, usize)> {
        if !self.contains(cell) {
            return None;
        }
        Some((cell.row - self.origin.row, cell.col - self.origin.col))
    }

    fn lane_edge(self, direction: Direction) -> Cell {
        let mut edge = self.cut_target();
        for row in 0..self.height() {
            for col in 0..self.width() {
                let candidate = Cell::new(self.origin.col + col, self.origin.row + row);
                if !self.cuts(candidate) {
                    continue;
                }
                let farther = match direction {
                    Direction::Up => candidate.row < edge.row,
                    Direction::Down => candidate.row > edge.row,
                    Direction::Left => candidate.col < edge.col,
                    Direction::Right => candidate.col > edge.col,
                };
                if farther {
                    edge = candidate;
                }
            }
        }
        edge
    }
}

fn station_cut_step(
    from: Cell,
    station: SplitterStation,
    preferred: Direction,
    cols: usize,
    rows: usize,
) -> Option<(Direction, Cell)> {
    unique_directions([
        preferred,
        station.flow_direction(),
        station.old_exit_direction(),
        preferred.reversed(),
    ])
    .into_iter()
    .find_map(|direction| {
        let next = from.stepped(direction, cols, rows);
        (station.cuts(next) || !station.occupies(next)).then_some((direction, next))
    })
}

fn stations_overlap(left: SplitterStation, right: SplitterStation) -> bool {
    let start_col = left.origin.col.max(right.origin.col);
    let end_col = (left.origin.col + left.width()).min(right.origin.col + right.width());
    let start_row = left.origin.row.max(right.origin.row);
    let end_row = (left.origin.row + left.height()).min(right.origin.row + right.height());
    (start_row..end_row).any(|row| {
        (start_col..end_col).any(|col| {
            let cell = Cell::new(col, row);
            left.occupies(cell) && right.occupies(cell)
        })
    })
}

impl SplitterActivity {
    pub(super) fn station(self) -> SplitterStation {
        self.station
    }

    pub(super) fn phase(self) -> SplitterActivityPhase {
        self.phase
    }

    pub(super) fn progress(self) -> f64 {
        self.progress
    }
}

impl SplitterJob {
    const fn queued(source_snek_id: usize) -> Self {
        Self {
            source_snek_id,
            phase: SplitterJobPhase::Queued,
        }
    }

    fn reserves_segments(self) -> bool {
        !matches!(self.phase, SplitterJobPhase::Releasing { .. })
    }

    fn station(self) -> Option<SplitterStation> {
        match self.phase {
            SplitterJobPhase::Queued => None,
            SplitterJobPhase::Routing { station, .. }
            | SplitterJobPhase::Feeding { station, .. }
            | SplitterJobPhase::Cutting { station, .. }
            | SplitterJobPhase::Releasing { station, .. } => Some(station),
        }
    }

    fn uses_snek(self, snek_id: usize) -> bool {
        match self.phase {
            SplitterJobPhase::Queued => false,
            SplitterJobPhase::Routing { snek_id: id, .. }
            | SplitterJobPhase::Feeding { snek_id: id, .. }
            | SplitterJobPhase::Cutting { snek_id: id, .. } => id == snek_id,
            SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                ..
            } => old_snek_id == snek_id || new_snek_id == snek_id,
        }
    }

    fn active_snek_ids(self) -> [Option<usize>; 2] {
        match self.phase {
            SplitterJobPhase::Queued => [None, None],
            SplitterJobPhase::Routing { snek_id, .. }
            | SplitterJobPhase::Feeding { snek_id, .. }
            | SplitterJobPhase::Cutting { snek_id, .. } => [Some(snek_id), None],
            SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                ..
            } => [Some(old_snek_id), Some(new_snek_id)],
        }
    }

    fn is_valid_ids(self, snek_ids: &HashSet<usize>, stations: &[SplitterStation]) -> bool {
        if !snek_ids.contains(&self.source_snek_id) {
            return false;
        }
        if let Some(station) = self.station() {
            if !stations.contains(&station) {
                return false;
            }
        }
        match self.phase {
            SplitterJobPhase::Queued => true,
            SplitterJobPhase::Routing { snek_id, .. } => {
                snek_id == self.source_snek_id && snek_ids.contains(&snek_id)
            }
            SplitterJobPhase::Feeding {
                snek_id, advanced, ..
            } => {
                snek_id == self.source_snek_id
                    && snek_ids.contains(&snek_id)
                    && advanced < SPLITTER_FEED_STEPS
            }
            SplitterJobPhase::Cutting {
                snek_id, elapsed, ..
            } => {
                snek_id == self.source_snek_id
                    && snek_ids.contains(&snek_id)
                    && elapsed < SPLITTER_CUT_STEPS
            }
            SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                ..
            } => {
                old_snek_id == self.source_snek_id
                    && old_snek_id != new_snek_id
                    && snek_ids.contains(&old_snek_id)
                    && snek_ids.contains(&new_snek_id)
            }
        }
    }

    fn shift_station(&mut self, cols: usize, rows: usize) {
        let station = match &mut self.phase {
            SplitterJobPhase::Queued => return,
            SplitterJobPhase::Routing { station, .. }
            | SplitterJobPhase::Feeding { station, .. }
            | SplitterJobPhase::Cutting { station, .. }
            | SplitterJobPhase::Releasing { station, .. } => station,
        };
        station.origin.col += cols;
        station.origin.row += rows;
    }

    fn is_valid(self, sneks: &[Snek], stations: &[SplitterStation]) -> bool {
        if !sneks.iter().any(|snek| snek.id == self.source_snek_id) {
            return false;
        }
        if let Some(station) = self.station() {
            if !stations.contains(&station) {
                return false;
            }
        }
        match self.phase {
            SplitterJobPhase::Queued => true,
            SplitterJobPhase::Routing { snek_id, .. }
            | SplitterJobPhase::Feeding { snek_id, .. }
            | SplitterJobPhase::Cutting { snek_id, .. } => {
                sneks.iter().any(|snek| snek.id == snek_id)
            }
            SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                ..
            } => [old_snek_id, new_snek_id]
                .into_iter()
                .all(|id| sneks.iter().any(|snek| snek.id == id)),
        }
    }

    fn activity(self) -> Option<SplitterActivity> {
        let (station, phase, progress) = match self.phase {
            SplitterJobPhase::Queued | SplitterJobPhase::Routing { .. } => return None,
            SplitterJobPhase::Feeding {
                station, advanced, ..
            } => (
                station,
                SplitterActivityPhase::Feeding,
                advanced as f64 / SPLITTER_FEED_STEPS as f64,
            ),
            SplitterJobPhase::Cutting {
                station, elapsed, ..
            } => (
                station,
                SplitterActivityPhase::Cutting,
                elapsed as f64 / SPLITTER_CUT_STEPS as f64,
            ),
            SplitterJobPhase::Releasing { station, .. } => {
                (station, SplitterActivityPhase::Releasing, 1.0)
            }
        };
        Some(SplitterActivity {
            station,
            phase,
            progress,
        })
    }
}

impl SplitterDispatch {
    fn new(game: &SnekGame) -> Self {
        let mut station_by_snek = vec![None; game.sneks.len()];
        for job in &game.splitter_jobs {
            let SplitterJobPhase::Routing { snek_id, station } = job.phase else {
                continue;
            };
            if let Some(index) = game.sneks.iter().position(|snek| snek.id == snek_id) {
                station_by_snek[index] = Some(station);
            }
        }

        Self { station_by_snek }
    }

    fn station_for_snek(&self, index: usize) -> Option<SplitterStation> {
        self.station_by_snek.get(index).copied().flatten()
    }
}

impl Snek {
    fn new(id: usize, center: Cell) -> Self {
        Self {
            id,
            direction: Direction::Up,
            positions: vec![center; MIN_SNEK_LEN].into(),
            len: MIN_SNEK_LEN,
            exit_override: None,
        }
    }

    #[cfg(test)]
    fn new_with_direction(id: usize, center: Cell, direction: Direction, len: usize) -> Self {
        let len = len.max(MIN_SNEK_LEN);
        Self {
            id,
            direction,
            positions: vec![center; len].into(),
            len,
            exit_override: None,
        }
    }

    fn from_save(save: SavedSnek) -> Self {
        Self {
            id: save.id,
            direction: save.direction,
            positions: save.positions.into(),
            len: save.len,
            exit_override: None,
        }
    }

    fn save(&self) -> SavedSnek {
        SavedSnek {
            id: self.id,
            direction: self.direction,
            positions: self.positions.iter().copied().collect(),
            len: self.len,
        }
    }

    pub(super) fn id(&self) -> usize {
        self.id
    }

    pub(super) fn positions(&self) -> &VecDeque<Cell> {
        &self.positions
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn head(&self) -> Cell {
        self.positions.back().copied().unwrap_or(Cell::new(0, 0))
    }

    fn next_cell(&self, cols: usize, rows: usize) -> Cell {
        self.head().stepped(self.direction, cols, rows)
    }

    fn step_to(&mut self, next: Cell, ate_apple: bool) {
        self.positions.push_back(next);
        if ate_apple {
            self.len += 1;
        }
        while self.positions.len() > self.len {
            self.positions.pop_front();
        }
    }

    fn shift(&mut self, cols: usize, rows: usize) {
        for cell in &mut self.positions {
            cell.col += cols;
            cell.row += rows;
        }
    }

    fn contains(&self, cell: Cell) -> bool {
        self.positions.contains(&cell)
    }

    fn contains_near(&self, cell: Cell, cols: usize, rows: usize) -> bool {
        self.positions
            .iter()
            .any(|position| toroidal_distance(*position, cell, cols, rows) <= 1)
    }

    fn excess_len(&self) -> usize {
        self.len.saturating_sub(MIN_SNEK_LEN)
    }

    fn remove_tail_segments(&mut self, count: usize) {
        self.len = self.len.saturating_sub(count).max(MIN_SNEK_LEN);
        while self.positions.len() > self.len {
            self.positions.pop_front();
        }
    }

    fn split_head(&mut self, new_id: usize, direction: Direction) -> Option<Self> {
        if self.excess_len() < MIN_SNEK_LEN || self.positions.len() < MIN_SNEK_LEN * 2 {
            return None;
        }
        let new_positions = self
            .positions
            .split_off(self.positions.len() - MIN_SNEK_LEN);
        self.len -= MIN_SNEK_LEN;
        Some(Self {
            id: new_id,
            direction,
            positions: new_positions,
            len: MIN_SNEK_LEN,
            exit_override: Some(direction),
        })
    }

    fn intersects_station(&self, station: SplitterStation) -> bool {
        self.positions.iter().any(|cell| station.occupies(*cell))
    }

    fn add_segments(&mut self, count: usize) {
        self.len += count;
        let tail = self
            .positions
            .front()
            .copied()
            .unwrap_or_else(|| self.head());
        while self.positions.len() < self.len {
            self.positions.push_front(tail);
        }
    }
}

fn random_cell(cols: usize, rows: usize, rng: &mut Lcg) -> Cell {
    Cell::new(rng.next_usize(cols.max(1)), rng.next_usize(rows.max(1)))
}

pub(super) fn station_cell(
    design: StationDesign,
    row: usize,
    col: usize,
    rotation: u8,
) -> Option<char> {
    if row >= station_height(rotation) || col >= station_width(rotation) {
        return None;
    }
    let (source_row, source_col) = rotated_station_index(row, col, rotation);
    station_pattern(design)
        .get(source_row)
        .and_then(|line| line.as_bytes().get(source_col))
        .map(|byte| *byte as char)
}

fn rotated_station_index(row: usize, col: usize, rotation: u8) -> (usize, usize) {
    match rotation % 4 {
        1 => (SPLITTER_STATION_ROWS - 1 - col, row),
        2 => (
            SPLITTER_STATION_ROWS - 1 - row,
            SPLITTER_STATION_COLS - 1 - col,
        ),
        3 => (col, SPLITTER_STATION_COLS - 1 - row),
        _ => (row, col),
    }
}

fn rotated_station_position(source_row: usize, source_col: usize, rotation: u8) -> (usize, usize) {
    match rotation % 4 {
        1 => (source_col, SPLITTER_STATION_ROWS - 1 - source_row),
        2 => (
            SPLITTER_STATION_ROWS - 1 - source_row,
            SPLITTER_STATION_COLS - 1 - source_col,
        ),
        3 => (SPLITTER_STATION_COLS - 1 - source_col, source_row),
        _ => (source_row, source_col),
    }
}

fn station_pattern(design: StationDesign) -> [&'static str; SPLITTER_STATION_ROWS] {
    match design {
        StationDesign::Gate => ["  1221  ", " 123321 ", "00000000", "  11011 ", "   101  "],
        StationDesign::Saw => [" 233332 ", "12333221", "00000000", " 333033 ", "  22022 "],
        StationDesign::Press => [" 133331 ", " 133331 ", "00000000", "  22022 ", "   101  "],
        StationDesign::Butterfly => ["22 33 22", " 223322 ", "00000000", "22 10122", "  11011 "],
        StationDesign::Mantis => ["2 2332 2", " 223322 ", "00000000", "2  101 2", "   101  "],
    }
}

fn station_width(rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        SPLITTER_STATION_COLS
    } else {
        SPLITTER_STATION_ROWS
    }
}

fn station_height(rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        SPLITTER_STATION_ROWS
    } else {
        SPLITTER_STATION_COLS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_edges() {
        let mut game = SnekGame::new(5, 5, 1);
        game.sneks[0].positions = vec![Cell::new(2, 0); MIN_SNEK_LEN].into();
        game.set_direction(Direction::Up);
        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].positions().back(), Some(&Cell::new(2, 4)));

        game.set_direction(Direction::Right);
        game.sneks[0].positions = vec![Cell::new(4, 3); MIN_SNEK_LEN].into();
        game.step(&mut Lcg::new(3));

        assert_eq!(game.sneks[0].positions().back(), Some(&Cell::new(0, 3)));
    }

    #[test]
    fn apple_grows_score_and_replaces_itself() {
        let mut game = SnekGame::new(8, 8, 1);
        game.sneks[0].positions = vec![Cell::new(4, 4); MIN_SNEK_LEN].into();
        game.apples = vec![Cell::new(4, 3)];
        game.set_direction(Direction::Up);
        game.step(&mut Lcg::new(2));

        assert_eq!(game.apls(), 1);
        assert_eq!(game.sneks[0].positions().len(), 4);
        assert_eq!(game.apples.len(), 1);
    }

    #[test]
    fn saved_game_rejects_missing_world_dimensions() {
        let save = SavedSnekGame {
            version: SNEK_SAVE_VERSION,
            cols: 0,
            rows: 0,
            sneks: vec![SavedSnek {
                id: 1,
                direction: Direction::Left,
                positions: vec![Cell::new(10, 10), Cell::new(11, 10), Cell::new(12, 10)],
                len: MIN_SNEK_LEN,
            }],
            apples: vec![Cell::new(12, 12)],
            auto_snek_unlocked: false,
            splitter_unlocked: false,
            splitter_stations: Vec::new(),
            splitter_jobs: Vec::new(),
        };
        assert!(SnekGame::from_save(save, 5, 5).is_none());
    }

    #[test]
    fn saved_game_preserves_grown_world_dimensions() {
        let station =
            SplitterStation::new(Cell::new(45, 32), 0, StationDesign::Gate, 60, 45).with_id(1);
        let save = SavedSnekGame {
            version: SNEK_SAVE_VERSION,
            cols: 60,
            rows: 45,
            sneks: vec![SavedSnek {
                id: 1,
                direction: Direction::Left,
                positions: vec![Cell::new(50, 30), Cell::new(51, 30), Cell::new(52, 30)],
                len: MIN_SNEK_LEN,
            }],
            apples: vec![Cell::new(55, 40)],
            auto_snek_unlocked: true,
            splitter_unlocked: true,
            splitter_stations: vec![station],
            splitter_jobs: Vec::new(),
        };

        let game = SnekGame::from_save(save, 8, 8).unwrap();

        assert_eq!(game.cols(), 60);
        assert_eq!(game.rows(), 45);
        assert_eq!(
            game.sneks[0].positions(),
            &[Cell::new(50, 30), Cell::new(51, 30), Cell::new(52, 30)]
        );
        assert_eq!(game.apples(), &[Cell::new(55, 40)]);
        assert_eq!(game.splitter_stations(), &[station]);
    }

    #[test]
    fn saved_game_requires_current_version() {
        let mut save = SnekGame::new(8, 8, 1).save();
        save.version = SNEK_SAVE_VERSION.saturating_add(1);

        assert!(SnekGame::from_save(save, 8, 8).is_none());
    }

    #[test]
    fn saved_game_rejects_unsafe_world_and_identity_data() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Gate, 30, 30).with_id(1);
        game.splitter_unlocked = true;
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Routing {
                snek_id: 1,
                station,
            },
        }];
        let valid = game.save();

        let mut oversized = valid.clone();
        oversized.cols = MAX_WORLD_DIMENSION + 1;
        assert!(SnekGame::from_save(oversized, 30, 30).is_none());

        let mut duplicate_snek_id = valid.clone();
        duplicate_snek_id
            .sneks
            .push(duplicate_snek_id.sneks[0].clone());
        assert!(SnekGame::from_save(duplicate_snek_id, 30, 30).is_none());

        let mut too_many_apples = valid.clone();
        too_many_apples.apples = vec![Cell::new(1, 1); MAX_SNEKS + 1];
        assert!(SnekGame::from_save(too_many_apples, 30, 30).is_none());

        let mut excessive_navigation = valid.clone();
        excessive_navigation.cols = MAX_WORLD_DIMENSION;
        excessive_navigation.rows = MAX_WORLD_DIMENSION;
        excessive_navigation.splitter_jobs.clear();
        excessive_navigation.sneks = (1..=5)
            .map(|id| SavedSnek {
                id,
                direction: Direction::Right,
                positions: vec![Cell::new(id, id); MIN_SNEK_LEN],
                len: MIN_SNEK_LEN,
            })
            .collect();
        assert!(SnekGame::from_save(excessive_navigation, 30, 30).is_none());

        let mut invalid_station = valid.clone();
        invalid_station.splitter_stations[0].origin = Cell::new(29, 29);
        assert!(SnekGame::from_save(invalid_station, 30, 30).is_none());

        let mut stale_job_station = valid;
        let SplitterJobPhase::Routing { station, .. } =
            &mut stale_job_station.splitter_jobs[0].phase
        else {
            unreachable!();
        };
        station.origin.col += 1;
        assert!(SnekGame::from_save(stale_job_station, 30, 30).is_none());
    }

    #[test]
    fn offscreen_apples_do_not_force_world_to_stay_large_on_resize() {
        let mut game = SnekGame::new(40, 28, 1);
        game.sneks[0].positions = vec![Cell::new(2, 2); MIN_SNEK_LEN].into();
        game.apples = vec![Cell::new(39, 27)];

        game.resize(8, 8);

        assert_eq!(game.cols(), 8);
        assert_eq!(game.rows(), 8);
        assert_eq!(game.apples().len(), 1);
        assert!(game.apples()[0].col < 8);
        assert!(game.apples()[0].row < 8);
    }

    #[test]
    fn resize_replaces_each_removed_offscreen_apple() {
        let mut game = SnekGame::new(40, 28, 1);
        game.sneks[0].positions = vec![Cell::new(2, 2); MIN_SNEK_LEN].into();
        game.apples = vec![Cell::new(1, 1), Cell::new(39, 27)];

        game.resize(8, 8);

        assert_eq!(game.cols(), 8);
        assert_eq!(game.rows(), 8);
        assert_eq!(game.apples().len(), 2);
        assert!(game
            .apples()
            .iter()
            .all(|apple| apple.col < 8 && apple.row < 8));
    }

    #[test]
    fn auto_snek_purchase_spends_segments_without_shortening_below_min() {
        let mut game = SnekGame::new(8, 8, 1);
        game.sneks[0].len = MIN_SNEK_LEN + AUTO_SNEK_COST;
        game.sneks[0].positions = vec![Cell::new(4, 4); MIN_SNEK_LEN + AUTO_SNEK_COST].into();

        assert!(game.buy_auto_snek());
        assert!(game.auto_snek_unlocked());
        assert_eq!(game.apls(), 0);
        assert_eq!(game.sneks[0].positions().len(), MIN_SNEK_LEN);
    }

    #[test]
    fn autopilot_moves_toward_nearest_apple() {
        let mut game = SnekGame::new(10, 10, 1);
        game.sneks[0].len = MIN_SNEK_LEN + AUTO_SNEK_COST;
        game.sneks[0].positions = vec![Cell::new(2, 2); MIN_SNEK_LEN + AUTO_SNEK_COST].into();
        assert!(game.buy_auto_snek());

        game.apples = vec![Cell::new(5, 2)];
        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].head(), Cell::new(3, 2));
    }

    #[test]
    fn closest_auto_snek_claims_the_apple() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.sneks = vec![Snek::new(1, Cell::new(2, 2)), Snek::new(2, Cell::new(8, 2))];
        game.sneks[0].direction = Direction::Up;
        game.sneks[1].direction = Direction::Up;
        game.apples = vec![Cell::new(9, 2)];

        let splitter_dispatch = game.splitter_dispatch();
        game.update_autopilot_directions(&splitter_dispatch);

        assert_eq!(game.sneks[1].direction, Direction::Right);
        assert_ne!(game.sneks[0].direction, Direction::Right);
    }

    #[test]
    fn taps_near_any_body_segment_select_the_snek() {
        let mut game = SnekGame::new(10, 10, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.sneks[0].positions = vec![
            Cell::new(2, 2),
            Cell::new(3, 2),
            Cell::new(4, 2),
            Cell::new(5, 2),
        ]
        .into();

        game.tap_cell(Cell::new(4, 3));

        assert_eq!(game.manual_snek(), Some(1));
    }

    #[test]
    fn manual_snek_bounces_off_splitter_station() {
        let mut game = SnekGame::new(20, 20, 1);
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(5, 5),
            0,
            StationDesign::Gate,
            20,
            20,
        )];
        game.sneks[0].positions = vec![Cell::new(7, 4); MIN_SNEK_LEN].into();
        game.set_direction(Direction::Down);

        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].head(), Cell::new(7, 3));
    }

    #[test]
    fn splitter_station_purchase_places_design_and_spends_segments() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(4, 4); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(6, 7),
            2,
            StationDesign::Saw,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(station.origin(), Cell::new(6, 7));
        assert_eq!(station.rotation(), 2);
        assert_eq!(station.design(), StationDesign::Saw);
        assert_eq!(game.apls(), 0);
    }

    #[test]
    fn splitter_station_purchase_allows_snek_overlap_and_spends() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(6, 7); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(5, 5),
            0,
            StationDesign::Gate,
            &mut Lcg::new(2)
        ));

        assert!(!game.splitter_stations.is_empty());
        assert_eq!(game.apls(), 0);
    }

    #[test]
    fn splitter_station_purchase_replaces_covered_apples() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(15, 15); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();
        game.apples = vec![Cell::new(6, 7)];

        assert!(game.buy_splitter_station(
            Cell::new(5, 5),
            0,
            StationDesign::Gate,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(game.apples.len(), 1);
        assert!(!station.contains(game.apples[0]));
    }

    #[test]
    fn splitter_station_move_allows_snek_overlap_and_replaces_covered_apples() {
        let mut game = SnekGame::new(20, 20, 1);
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(1, 1),
            0,
            StationDesign::Gate,
            20,
            20,
        )];
        game.sneks[0].positions = vec![Cell::new(10, 10); MIN_SNEK_LEN].into();

        game.apples = vec![Cell::new(6, 6)];
        assert!(game.move_splitter_station(0, Cell::new(5, 5), 0, &mut Lcg::new(3)));
        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(game.apples.len(), 1);
        assert!(!station.contains(game.apples[0]));
    }

    #[test]
    fn snek_inside_splitter_wall_can_route_out() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(5, 5),
            0,
            StationDesign::Gate,
            20,
            20,
        )];
        game.sneks[0].positions = vec![Cell::new(6, 5); MIN_SNEK_LEN].into();
        game.apples = vec![Cell::new(4, 5)];

        game.step(&mut Lcg::new(2));

        assert_ne!(game.sneks[0].head(), Cell::new(6, 5));
    }

    #[test]
    fn splitter_placement_near_edge_grows_world_with_buffer() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(10, 10); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(18, 18),
            0,
            StationDesign::Gate,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert!(game.cols() >= station.origin().col + station.width() + BUILDING_BUFFER);
        assert!(game.rows() >= station.origin().row + station.height() + BUILDING_BUFFER);
    }

    #[test]
    fn buying_snek_queues_order_and_spends_when_cutter_is_entered() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(8, 8),
            0,
            StationDesign::Gate,
            20,
            20,
        )];
        game.sneks[0].len = MIN_SNEK_LEN + MIN_SNEK_LEN;
        game.sneks[0].positions = vec![Cell::new(7, 10); MIN_SNEK_LEN + MIN_SNEK_LEN].into();
        game.sneks[0].direction = Direction::Right;
        let apls_before = game.apls();

        assert!(game.buy_snek());
        assert_eq!(game.pending_snek_orders(), 1);
        assert_eq!(game.sneks().len(), 1);
        assert_eq!(game.apls(), apls_before);

        for _ in 0..60 {
            game.step(&mut Lcg::new(2));
            if game.sneks().len() == 2 && game.pending_snek_orders() == 0 {
                break;
            }
        }

        assert_eq!(game.sneks().len(), 2);
        assert_eq!(game.apls(), 0);
        assert_eq!(game.pending_snek_orders(), 0);
    }

    #[test]
    fn splitter_job_routes_to_the_directed_pre_entry_cell() {
        let mut game = SnekGame::new(30, 30, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station =
            SplitterStation::new(Cell::new(10, 10), 0, StationDesign::Gate, 30, 30).with_id(1);
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob::queued(1)];
        let side_of_entry =
            station
                .entry_cell()
                .stepped(station.flow_direction().clockwise(), 30, 30);
        game.sneks[0] =
            Snek::new_with_direction(1, side_of_entry, station.flow_direction(), MIN_SNEK_LEN * 2);

        let mut entered = false;
        for _ in 0..80 {
            let previous = game.sneks[0].head();
            game.step(&mut Lcg::new(2));
            let current = game.sneks[0].head();
            if current == station.entry_cell() {
                assert!(station.legal_entry_step(previous, current, 30, 30));
                entered = true;
            }
            if !matches!(
                game.splitter_jobs.first().map(|job| job.phase),
                Some(SplitterJobPhase::Queued | SplitterJobPhase::Routing { .. })
            ) {
                break;
            }
        }

        assert!(entered);
        assert!(!matches!(
            game.splitter_jobs.first().map(|job| job.phase),
            Some(SplitterJobPhase::Queued | SplitterJobPhase::Routing { .. })
        ));
    }

    #[test]
    fn splitter_dispatch_uses_closest_unassigned_snek_cutter_pairs() {
        let mut game = SnekGame::new(40, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![
            SplitterStation::new(Cell::new(4, 4), 0, StationDesign::Gate, 40, 20).with_id(1),
            SplitterStation::new(Cell::new(28, 4), 0, StationDesign::Gate, 40, 20).with_id(2),
        ];
        game.splitter_jobs = vec![SplitterJob::queued(1), SplitterJob::queued(2)];
        game.sneks = vec![
            Snek::new_with_direction(1, Cell::new(3, 6), Direction::Right, MIN_SNEK_LEN * 2),
            Snek::new_with_direction(2, Cell::new(27, 6), Direction::Right, MIN_SNEK_LEN * 2),
            Snek::new_with_direction(3, Cell::new(18, 18), Direction::Right, MIN_SNEK_LEN * 2),
        ];

        game.assign_splitter_jobs();
        let dispatch = game.splitter_dispatch();

        assert_eq!(
            dispatch.station_for_snek(0),
            Some(game.splitter_stations[0])
        );
        assert_eq!(
            dispatch.station_for_snek(1),
            Some(game.splitter_stations[1])
        );
        assert_eq!(dispatch.station_for_snek(2), None);
    }

    #[test]
    fn splitter_dispatch_keeps_in_lane_snek_assigned() {
        let mut game = SnekGame::new(40, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Gate, 40, 20).with_id(1);
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob::queued(2)];
        game.sneks = vec![
            Snek::new_with_direction(
                1,
                station.entry_cell().stepped(Direction::Left, 40, 20),
                Direction::Right,
                MIN_SNEK_LEN * 2,
            ),
            Snek::new_with_direction(2, station.entry_cell(), Direction::Right, MIN_SNEK_LEN * 2),
        ];

        game.assign_splitter_jobs();
        let dispatch = game.splitter_dispatch();

        assert_eq!(dispatch.station_for_snek(0), None);
        assert_eq!(dispatch.station_for_snek(1), Some(station));
    }

    #[test]
    fn splitter_designs_have_side_exit_from_cut_target() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let station = SplitterStation::new(Cell::new(8, 8), rotation, design, 40, 40);
                let first_side = station
                    .cut_target()
                    .stepped(station.old_exit_direction(), 40, 40);
                let second_side = first_side.stepped(station.old_exit_direction(), 40, 40);

                assert!(
                    station.cuts(first_side),
                    "{design:?} rotation {rotation} missing side exit"
                );
                assert!(
                    station.cuts(second_side) || !station.contains(second_side),
                    "{design:?} rotation {rotation} side exit does not leave"
                );
            }
        }
    }

    #[test]
    fn splitter_output_corridors_are_one_cell_wide() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let station = SplitterStation::new(Cell::new(8, 8), rotation, design, 40, 40);
                for direction in [station.flow_direction(), station.old_exit_direction()] {
                    let mut cell = station.cut_target().stepped(direction, 40, 40);
                    while station.contains(cell) {
                        assert!(
                            station.cuts(cell),
                            "{design:?} rotation {rotation} has a broken output corridor"
                        );
                        for side in [direction.clockwise(), direction.counter_clockwise()] {
                            let neighbor = cell.stepped(side, 40, 40);
                            assert!(
                                !station.cuts(neighbor),
                                "{design:?} rotation {rotation} has an output wider than one cell"
                            );
                        }
                        cell = cell.stepped(direction, 40, 40);
                    }
                }
            }
        }
    }

    #[test]
    fn splitter_sends_new_snek_forward_and_old_snek_out_side_exit() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let mut game = SnekGame::new(40, 40, 1);
                game.auto_snek_unlocked = true;
                game.manual_snek = None;
                let station =
                    SplitterStation::new(Cell::new(12, 12), rotation, design, 40, 40).with_id(1);
                game.splitter_stations = vec![station];
                game.splitter_jobs = vec![SplitterJob::queued(1)];
                game.sneks = vec![Snek::new_with_direction(
                    1,
                    station
                        .entry_cell()
                        .stepped(station.flow_direction().reversed(), 40, 40),
                    station.flow_direction(),
                    MIN_SNEK_LEN * 2,
                )];
                game.apples = vec![Cell::new(1, 1)];

                for _ in 0..60 {
                    game.step(&mut Lcg::new(2));
                    if game.sneks().len() == 2 {
                        break;
                    }
                }

                assert_eq!(
                    game.sneks().len(),
                    2,
                    "{design:?} rotation {rotation} did not split"
                );
                assert_eq!(
                    game.sneks[0].head(),
                    station
                        .cut_target()
                        .stepped(station.old_exit_direction(), 40, 40)
                );
                assert_eq!(game.sneks[0].direction, station.old_exit_direction());
                assert_eq!(game.sneks[1].direction, station.flow_direction());
                assert_eq!(
                    game.sneks[1].head(),
                    station
                        .new_exit_cell()
                        .stepped(station.flow_direction(), 40, 40)
                );
                assert!(matches!(
                    game.splitter_activities().as_slice(),
                    [SplitterActivity {
                        phase: SplitterActivityPhase::Releasing,
                        ..
                    }]
                ));

                game.step(&mut Lcg::new(4));
                assert_eq!(
                    game.sneks[0].head(),
                    station
                        .cut_target()
                        .stepped(station.old_exit_direction(), 40, 40)
                        .stepped(station.old_exit_direction(), 40, 40)
                );
                assert_eq!(
                    game.sneks[1].head(),
                    station
                        .new_exit_cell()
                        .stepped(station.flow_direction(), 40, 40)
                        .stepped(station.flow_direction(), 40, 40)
                );
            }
        }
    }

    #[test]
    fn snek_on_side_exit_cut_cells_routes_out_instead_of_into_station_wall() {
        let mut game = SnekGame::new(30, 30, 1);
        let station = SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Butterfly, 30, 30);
        let side_cell = station
            .cut_target()
            .stepped(station.old_exit_direction(), 30, 30);
        game.splitter_stations = vec![station];
        game.sneks[0].positions = vec![side_cell; MIN_SNEK_LEN].into();
        game.sneks[0].direction = station.flow_direction();
        game.apples = vec![Cell::new(1, 1)];

        game.step(&mut Lcg::new(2));

        assert_eq!(
            game.sneks[0].head(),
            side_cell.stepped(station.old_exit_direction(), 30, 30)
        );
        assert!(station.cuts(game.sneks[0].head()));

        game.step(&mut Lcg::new(3));

        assert!(!station.contains(game.sneks[0].head()));
    }

    #[test]
    fn snek_inside_splitter_footprint_prefers_to_leave() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 20, 20);
        game.splitter_stations = vec![station];
        game.sneks[0].positions = vec![Cell::new(6, 7); MIN_SNEK_LEN].into();
        game.sneks[0].direction = Direction::Right;
        game.apples = vec![Cell::new(2, 7)];

        for _ in 0..10 {
            game.step(&mut Lcg::new(2));
            if !station.contains(game.sneks[0].head()) {
                return;
            }
        }

        panic!("snek stayed inside splitter footprint");
    }

    #[test]
    fn snek_inside_splitter_wall_can_cross_interior_to_leave() {
        let mut game = SnekGame::new(20, 20, 1);
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 20, 20);
        let start = Cell::new(6, 6);
        assert!(station.contains(start));
        assert!(!station.cuts(start));
        game.splitter_stations = vec![station];
        game.sneks[0].positions = vec![start; MIN_SNEK_LEN].into();
        game.sneks[0].direction = Direction::Up;
        game.apples = vec![Cell::new(2, 2)];

        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].head(), Cell::new(6, 5));
        assert!(station.contains(game.sneks[0].head()));

        game.step(&mut Lcg::new(3));

        assert!(!station.contains(game.sneks[0].head()));
    }

    #[test]
    fn buying_apple_spends_segments_and_adds_an_apple() {
        let mut game = SnekGame::new(20, 20, 1);
        game.sneks[0].len = MIN_SNEK_LEN + APPLE_COST;
        game.sneks[0].positions = vec![Cell::new(10, 10); MIN_SNEK_LEN + APPLE_COST].into();
        let initial_apples = game.apples().len();

        assert!(game.buy_apple(&mut Lcg::new(2)));

        assert_eq!(game.apples().len(), initial_apples + 1);
        assert_eq!(game.apls(), 0);
    }

    #[test]
    fn splitter_station_rotation_swaps_footprint_dimensions() {
        let station = SplitterStation::new(Cell::new(18, 18), 1, StationDesign::Gate, 20, 20);

        assert_eq!(station.origin(), Cell::new(18, 18));
        assert_eq!(station.width(), SPLITTER_STATION_ROWS);
        assert_eq!(station.height(), SPLITTER_STATION_COLS);
        assert!(station.contains(Cell::new(19, 19)));
        assert!(!station.contains(Cell::new(14, 19)));
    }

    #[test]
    fn splitter_station_delete_refunds_segments() {
        let mut game = SnekGame::new(20, 20, 1);
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(5, 5),
            1,
            StationDesign::Press,
            20,
            20,
        )];

        assert!(game.delete_splitter_station(0));

        assert!(game.splitter_stations.is_empty());
        assert_eq!(game.apls(), SPLITTER_STATION_COST);
    }

    #[test]
    fn splitter_phase_round_trips_through_save() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Butterfly, 30, 30).with_id(7);
        game.splitter_unlocked = true;
        game.splitter_stations = vec![station];
        game.sneks[0] = Snek::new_with_direction(
            1,
            station.new_exit_cell(),
            station.flow_direction(),
            MIN_SNEK_LEN * 2,
        );
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Cutting {
                snek_id: 1,
                station,
                elapsed: 4,
            },
        }];

        let restored = SnekGame::from_save(game.save(), 30, 30).unwrap();

        assert_eq!(restored.splitter_jobs, game.splitter_jobs);
        assert_eq!(restored.splitter_stations[0].id, 7);
        assert_eq!(
            restored.splitter_activities()[0].phase(),
            SplitterActivityPhase::Cutting
        );
    }

    #[test]
    fn queued_order_reserves_segments_on_its_source_snek() {
        let mut game = SnekGame::new(30, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 30, 20).with_id(1);
        game.splitter_stations = vec![station];
        game.sneks = vec![
            Snek::new_with_direction(1, Cell::new(4, 7), Direction::Right, MIN_SNEK_LEN * 2),
            Snek::new_with_direction(2, Cell::new(20, 15), Direction::Left, MIN_SNEK_LEN * 2),
        ];

        assert!(game.buy_snek());
        assert_eq!(game.splitter_jobs[0].source_snek_id, 1);
        assert!(game.spend_apls(MIN_SNEK_LEN));

        assert_eq!(game.sneks[0].excess_len(), MIN_SNEK_LEN);
        assert_eq!(game.sneks[1].excess_len(), 0);
    }

    #[test]
    fn active_splitter_cannot_move_or_delete() {
        let mut game = SnekGame::new(30, 20, 1);
        let station =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 30, 20).with_id(1);
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Routing {
                snek_id: 1,
                station,
            },
        }];

        assert!(!game.move_splitter_station(0, Cell::new(10, 10), 0, &mut Lcg::new(2)));
        assert!(!game.delete_splitter_station(0));
    }

    #[test]
    fn shifting_the_world_keeps_active_job_station_in_sync() {
        let mut game = SnekGame::new(30, 20, 1);
        let station =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 30, 20).with_id(1);
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Routing {
                snek_id: 1,
                station,
            },
        }];

        game.shift_cells(4, 3);

        assert_eq!(
            game.splitter_jobs[0].station(),
            Some(game.splitter_stations[0])
        );
        assert_eq!(game.splitter_stations[0].origin(), Cell::new(9, 8));
    }
}
