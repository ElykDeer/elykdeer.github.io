#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use super::navigation::{
    direction_out_of_station, idle_direction, toroidal_distance, unique_directions,
    NavigationCache, PathMode,
};

pub(super) const DEFAULT_COLS: usize = 40;
pub(super) const DEFAULT_ROWS: usize = 28;
pub(super) const MIN_SNEK_LEN: usize = 3;
pub(super) const JUICER_UNLOCK_COST: usize = 10;
pub(super) const JUICER_PLACEMENT_UNLOCK_COST: usize = 50;
pub(super) const AUTO_SNEK_COST: usize = 1;
pub(super) const SPLITTER_STATION_UNLOCK_COST: usize = 10;
pub(super) const SPLITTER_STATION_COST: usize = 10;
pub(super) const JUICER_STATION_COST: usize = 20;
pub(super) const APPLE_COST: usize = 50;
pub(super) const BUILDING_BUFFER: usize = 5;
const SNEK_SAVE_VERSION: u8 = 14;
const SEGMENTS_PER_JUICE: usize = 10;
const SPLITTER_FEED_STEPS: u8 = MIN_SNEK_LEN as u8;
const SPLITTER_CUT_STEPS: u8 = 10;
const GROWTH_STALL_STEPS: u16 = 600;
const JUICER_EFFECT_STEPS: u8 = 18;
const BOOTSTRAP_CUT_STEPS: u8 = 10;
const BOOTSTRAP_WALK_STEPS: u8 = 28;
const BOOTSTRAP_FLASH_STEPS: u8 = 14;
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
    #[serde(default = "default_apple_capacity")]
    apple_capacity: usize,
    #[serde(default)]
    juice_segments: usize,
    #[serde(default)]
    juice_discovered: bool,
    #[serde(default)]
    lifetime_apls: usize,
    #[serde(default)]
    juicer_unlocked: bool,
    #[serde(default)]
    juicer_placement_unlocked: bool,
    #[serde(default)]
    bootstrap_phase: BootstrapPhase,
    #[serde(default)]
    bootstrap_juicer_id: Option<usize>,
    #[serde(default)]
    bootstrap_free_juicer_id: Option<usize>,
    #[serde(default)]
    bootstrap_cost_segments: VecDeque<Cell>,
    #[serde(default)]
    auto_snek_unlocked: bool,
    #[serde(default)]
    auto_snek_offered: bool,
    #[serde(default)]
    splitter_unlocked: bool,
    #[serde(default)]
    population_unlocked: bool,
    #[serde(default)]
    splitter_stations: Vec<SplitterStation>,
    #[serde(default)]
    construction_sites: Vec<ConstructionSite>,
    #[serde(default)]
    pending_purchases: VecDeque<JuicePurchase>,
    #[serde(default)]
    splitter_jobs: Vec<SplitterJob>,
    #[serde(default)]
    juicer_jobs: Vec<JuicerJob>,
    #[serde(default)]
    max_snek_len: Option<usize>,
    #[serde(default)]
    length_targets: Vec<LengthTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SavedSnek {
    id: usize,
    direction: Direction,
    positions: Vec<Cell>,
    len: usize,
    #[serde(default)]
    // One body segment can carry one complete juice package.
    carried_juice_segments: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SnekGame {
    cols: usize,
    rows: usize,
    sneks: Vec<Snek>,
    apples: Vec<Cell>,
    apple_capacity: usize,
    juice_segments: usize,
    juice_discovered: bool,
    lifetime_apls: usize,
    juicer_unlocked: bool,
    juicer_placement_unlocked: bool,
    bootstrap_phase: BootstrapPhase,
    bootstrap_juicer_id: Option<usize>,
    bootstrap_free_juicer_id: Option<usize>,
    bootstrap_cost_segments: VecDeque<Cell>,
    auto_snek_unlocked: bool,
    auto_snek_offered: bool,
    splitter_unlocked: bool,
    population_unlocked: bool,
    splitter_stations: Vec<SplitterStation>,
    construction_sites: Vec<ConstructionSite>,
    pending_purchases: VecDeque<JuicePurchase>,
    splitter_jobs: Vec<SplitterJob>,
    juicer_jobs: Vec<JuicerJob>,
    max_snek_len: Option<usize>,
    length_targets: Vec<LengthTarget>,
    manual_snek: Option<usize>,
    wiggle_t: f64,
    apls_produced: usize,
    sneks_produced: usize,
    juice_produced: usize,
    juicer_effects: Vec<JuicerEffect>,
}

#[derive(Clone, Debug)]
pub(super) struct Snek {
    id: usize,
    direction: Direction,
    positions: VecDeque<Cell>,
    len: usize,
    exit_override: Option<Direction>,
    growth_stall_steps: u16,
    carried_juice_segments: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LengthTarget {
    length: usize,
    count: usize,
}

const fn default_apple_capacity() -> usize {
    1
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum BootstrapPhase {
    #[default]
    Locked,
    Routing {
        snek_id: usize,
    },
    Cutting {
        snek_id: usize,
        elapsed: u8,
        center: Cell,
    },
    Walking {
        snek_id: usize,
        direction: Direction,
        elapsed: u8,
        center: Cell,
    },
    Flash {
        snek_id: usize,
        elapsed: u8,
        direction: Direction,
        center: Cell,
    },
    Complete,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
enum JuicePurchase {
    AutoSnek,
    JuicerPlacement,
    Splitter,
    Apple,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BootstrapActivityPhase {
    Routing,
    Cutting,
    Walking,
    Flash,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BootstrapActivity {
    pub(super) phase: BootstrapActivityPhase,
    pub(super) progress: f64,
    pub(super) center: Cell,
    pub(super) flash_origin: Cell,
    pub(super) direction: Direction,
}

#[derive(Clone, Copy, Debug)]
struct JuicerEffect {
    station: SplitterStation,
    remaining: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct JuicerJob {
    station: SplitterStation,
    source_snek_id: usize,
    retained_snek_id: Option<usize>,
    feed_segments: VecDeque<Cell>,
    approach_path: VecDeque<Cell>,
    gradient_len: usize,
    #[serde(default)]
    purpose: Option<JuicingPurpose>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum JuicingPurpose {
    Construction,
    Purchase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct JuicingAssignment {
    purpose: JuicingPurpose,
    segments: usize,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitterChannel {
    Entry,
    NewSnek,
    OldSnek,
}

#[derive(Clone, Debug)]
struct SplitterDispatch {
    station_by_snek: Vec<Option<SplitterStation>>,
    channel_by_snek: Vec<Option<(SplitterStation, SplitterChannel)>>,
    juicing_by_snek: Vec<Option<JuicingAssignment>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SplitterStation {
    #[serde(default)]
    id: usize,
    origin: Cell,
    rotation: u8,
    design: StationDesign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ConstructionSite {
    station_id: usize,
    // Construction is delivered in complete juice packages, not raw segments.
    remaining_segments: usize,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum StationDesign {
    Juicer,
    Splitter,
}

impl StationDesign {
    #[cfg(test)]
    pub(super) const ALL: [Self; 1] = [Self::Splitter];

    pub(super) const fn width(self) -> usize {
        9
    }

    pub(super) const fn height(self) -> usize {
        9
    }

    const fn cost(self) -> usize {
        match self {
            Self::Juicer => JUICER_STATION_COST,
            Self::Splitter => SPLITTER_STATION_COST,
        }
    }

    fn channel(self, channel: SplitterChannel) -> &'static [(usize, usize)] {
        match (self, channel) {
            (_, SplitterChannel::Entry) => SPLITTER_ENTRY,
            (Self::Splitter, SplitterChannel::NewSnek) => SPLITTER_NEW,
            (Self::Splitter, SplitterChannel::OldSnek) => SPLITTER_OLD,
            (Self::Juicer, SplitterChannel::NewSnek | SplitterChannel::OldSnek) => SPLITTER_OLD,
        }
    }

    const fn cut_cell(self) -> (usize, usize) {
        (3, 4)
    }
}

const SPLITTER_ENTRY: &[(usize, usize)] = &[(8, 4), (7, 4), (6, 4), (5, 4), (4, 4), (3, 4)];
const SPLITTER_NEW: &[(usize, usize)] = &[(3, 4), (3, 5), (2, 5), (1, 5), (0, 5)];
const SPLITTER_OLD: &[(usize, usize)] = &[(3, 4), (3, 3), (2, 3), (1, 3), (0, 3)];
const JUICER_FEED: &[(usize, usize)] = &[(3, 4), (3, 5), (2, 5), (2, 6)];

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
        let mut construction_ids = HashSet::new();
        if self.construction_sites.len() > self.splitter_stations.len()
            || self.construction_sites.iter().any(|site| {
                site.station_id == 0
                    || site.remaining_segments == 0
                    || self
                        .splitter_stations
                        .iter()
                        .find(|station| station.id == site.station_id)
                        .is_none_or(|station| site.remaining_segments > station.design.cost())
                    || !construction_ids.insert(site.station_id)
                    || !self
                        .splitter_stations
                        .iter()
                        .any(|station| station.id == site.station_id)
            })
        {
            return false;
        }

        let mut snek_ids = HashSet::new();
        let mut total_segments = 0usize;
        if !self.sneks.iter().all(|snek| {
            snek.id > 0
                && snek_ids.insert(snek.id)
                && (MIN_SNEK_LEN..=MAX_SNEK_LEN).contains(&snek.len)
                && snek.carried_juice_segments <= snek.len
                && snek.positions.len() == snek.len
                && snek
                    .positions
                    .iter()
                    .all(|cell| cell.col < cols && cell.row < rows)
                && total_segments.checked_add(snek.len).is_some_and(|total| {
                    total_segments = total;
                    total <= MAX_SNEKS * MAX_SNEK_LEN
                })
        }) {
            return false;
        }
        let carried_juice = self
            .sneks
            .iter()
            .map(|snek| snek.carried_juice_segments)
            .sum::<usize>();
        let construction_demand = self
            .construction_sites
            .iter()
            .map(|site| site.remaining_segments)
            .sum::<usize>();
        if carried_juice > construction_demand {
            return false;
        }

        if !self
            .apples
            .iter()
            .all(|cell| cell.col < cols && cell.row < rows)
        {
            return false;
        }
        let mut target_lengths = HashSet::new();
        let mut target_count = 0usize;
        if self.apple_capacity == 0
            || self.apple_capacity > MAX_SNEKS
            || self.apples.len() > self.apple_capacity
            || self
                .max_snek_len
                .is_some_and(|max| !(MIN_SNEK_LEN..=MAX_SNEK_LEN).contains(&max))
            || self.length_targets.iter().any(|target| {
                target.length < MIN_SNEK_LEN
                    || target.length > self.max_snek_len.unwrap_or(MIN_SNEK_LEN)
                    || target.count == 0
                    || !target_lengths.insert(target.length)
                    || target_count.checked_add(target.count).is_none_or(|total| {
                        target_count = total;
                        total > MAX_SNEKS
                    })
            })
        {
            return false;
        }
        if !self
            .length_targets
            .iter()
            .any(|target| target.length == MIN_SNEK_LEN && target.count >= 1)
        {
            return false;
        }
        if (self.juicer_placement_unlocked && !self.juicer_unlocked)
            || (self.population_unlocked && !self.splitter_unlocked)
            || self.bootstrap_juicer_id.is_some_and(|id| {
                !self
                    .splitter_stations
                    .iter()
                    .any(|station| station.id == id && station.design == StationDesign::Juicer)
            })
        {
            return false;
        }
        let bootstrap_station_valid = self.bootstrap_juicer_id.is_some_and(|id| {
            self.splitter_stations
                .iter()
                .any(|station| station.id == id && station.design == StationDesign::Juicer)
        });
        let bootstrap_can_pay = |snek_id| {
            self.sneks.iter().any(|snek| {
                snek.id == snek_id
                    && snek
                        .len
                        .saturating_sub(MIN_SNEK_LEN.max(snek.carried_juice_segments))
                        >= JUICER_UNLOCK_COST
            })
        };
        let bootstrap_valid = match self.bootstrap_phase {
            BootstrapPhase::Locked => {
                !self.juicer_unlocked
                    && self.bootstrap_juicer_id.is_none()
                    && self.bootstrap_cost_segments.is_empty()
                    && self
                        .sneks
                        .iter()
                        .all(|snek| snek.carried_juice_segments == 0)
            }
            BootstrapPhase::Routing { snek_id } => {
                !self.juicer_unlocked
                    && self.bootstrap_juicer_id.is_none()
                    && self.bootstrap_cost_segments.is_empty()
                    && snek_ids.contains(&snek_id)
                    && bootstrap_can_pay(snek_id)
                    && self.juicer_jobs.is_empty()
            }
            BootstrapPhase::Cutting {
                snek_id, center, ..
            } => {
                !self.juicer_unlocked
                    && self.bootstrap_juicer_id.is_none()
                    && self.bootstrap_cost_segments.is_empty()
                    && center.col < cols
                    && center.row < rows
                    && self
                        .sneks
                        .iter()
                        .any(|snek| snek.id == snek_id && snek.positions.last() == Some(&center))
                    && bootstrap_can_pay(snek_id)
                    && self.juicer_jobs.is_empty()
            }
            BootstrapPhase::Walking {
                snek_id, center, ..
            }
            | BootstrapPhase::Flash {
                snek_id, center, ..
            } => {
                !self.juicer_unlocked
                    && self.bootstrap_juicer_id.is_none()
                    && self.bootstrap_cost_segments.len() == JUICER_UNLOCK_COST
                    && self
                        .bootstrap_cost_segments
                        .iter()
                        .all(|cell| cell.col < cols && cell.row < rows)
                    && snek_ids.contains(&snek_id)
                    && center.col < cols
                    && center.row < rows
                    && self.juicer_jobs.is_empty()
            }
            BootstrapPhase::Complete => {
                self.juicer_unlocked
                    && bootstrap_station_valid
                    && self.bootstrap_cost_segments.is_empty()
            }
        };
        if !bootstrap_valid {
            return false;
        }

        let mut unique_purchases = HashSet::new();
        if self.pending_purchases.len() > MAX_SNEKS
            || self.apple_capacity.saturating_add(
                self.pending_purchases
                    .iter()
                    .filter(|purchase| matches!(purchase, JuicePurchase::Apple))
                    .count(),
            ) > MAX_SNEKS
            || self.pending_purchases.iter().any(|purchase| {
                (!matches!(purchase, JuicePurchase::Apple) && !unique_purchases.insert(*purchase))
                    || !self.juicer_unlocked
                    || match purchase {
                        JuicePurchase::AutoSnek => self.auto_snek_unlocked,
                        JuicePurchase::JuicerPlacement => self.juicer_placement_unlocked,
                        JuicePurchase::Splitter => self.splitter_unlocked,
                        JuicePurchase::Apple => false,
                    }
            })
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
                        .any(|right| stations_conflict(*left, *right))
                })
        {
            return false;
        }

        if !self.splitter_jobs.is_empty()
            && (!self.splitter_unlocked || self.splitter_stations.is_empty())
        {
            return false;
        }
        if self.splitter_jobs.iter().any(|job| {
            job.station().is_some_and(|station| {
                self.construction_sites
                    .iter()
                    .any(|site| site.station_id == station.id)
            })
        }) {
            return false;
        }
        if self.sneks.iter().any(|snek| {
            let reserved = self
                .splitter_jobs
                .iter()
                .filter(|job| job.source_snek_id == snek.id && job.reserves_segments())
                .count()
                * MIN_SNEK_LEN;
            reserved > snek.len.saturating_sub(MIN_SNEK_LEN)
        }) {
            return false;
        }

        let mut active_sneks = HashSet::new();
        let mut active_stations = HashSet::new();
        let splitters_valid = self.splitter_jobs.len() <= MAX_SNEKS
            && self.splitter_jobs.iter().all(|job| {
                job.is_valid_ids(&snek_ids, &self.splitter_stations)
                    && job.is_valid_saved_positions(&self.sneks)
                    && job
                        .active_snek_ids()
                        .into_iter()
                        .flatten()
                        .all(|id| active_sneks.insert(id))
                    && job
                        .station()
                        .is_none_or(|station| active_stations.insert(station.id))
            });
        if !splitters_valid || self.juicer_jobs.len() > MAX_SNEKS {
            return false;
        }
        self.juicer_jobs.iter().all(|job| {
            let station_valid = self.splitter_stations.iter().any(|station| {
                station.id == job.station.id
                    && *station == job.station
                    && station.design == StationDesign::Juicer
            }) && !self
                .construction_sites
                .iter()
                .any(|site| site.station_id == job.station.id);
            let owner_valid = match job.retained_snek_id {
                Some(id) => id == job.source_snek_id && self.sneks.iter().any(|snek| snek.id == id),
                None => !snek_ids.contains(&job.source_snek_id),
            };
            let feed_end_valid = job.feed_segments.back().is_some_and(|cell| {
                (!job.approach_path.is_empty()
                    && job.approach_path.back() == Some(&job.station.cut_target()))
                    || juicer_feed_on_approach(job.station, *cell, cols, rows)
                    || job
                        .station
                        .design
                        .channel(SplitterChannel::Entry)
                        .iter()
                        .any(|(row, col)| job.station.world_cell(*row, *col) == *cell)
                    || JUICER_FEED
                        .iter()
                        .any(|(row, col)| job.station.world_cell(*row, *col) == *cell)
            });
            station_valid
                && owner_valid
                && feed_end_valid
                && active_stations.insert(job.station.id)
                && active_sneks.insert(job.source_snek_id)
                && !job.feed_segments.is_empty()
                && job.gradient_len >= job.feed_segments.len()
                && job.gradient_len <= MAX_SNEK_LEN
                && total_segments
                    .checked_add(job.feed_segments.len())
                    .is_some_and(|total| {
                        total_segments = total;
                        total <= MAX_SNEKS * MAX_SNEK_LEN
                    })
                && job
                    .feed_segments
                    .iter()
                    .all(|cell| cell.col < cols && cell.row < rows)
                && job
                    .approach_path
                    .iter()
                    .all(|cell| cell.col < cols && cell.row < rows)
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

impl LengthTarget {
    pub(super) fn length(self) -> usize {
        self.length
    }

    pub(super) fn count(self) -> usize {
        self.count
    }
}

impl JuicePurchase {
    const fn cost(self) -> usize {
        match self {
            Self::AutoSnek => AUTO_SNEK_COST,
            Self::JuicerPlacement => JUICER_PLACEMENT_UNLOCK_COST,
            Self::Splitter => SPLITTER_STATION_UNLOCK_COST,
            Self::Apple => APPLE_COST,
        }
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
            apple_capacity: 1,
            juice_segments: 0,
            juice_discovered: false,
            lifetime_apls: 0,
            juicer_unlocked: false,
            juicer_placement_unlocked: false,
            bootstrap_phase: BootstrapPhase::Locked,
            bootstrap_juicer_id: None,
            bootstrap_free_juicer_id: None,
            bootstrap_cost_segments: VecDeque::new(),
            auto_snek_unlocked: false,
            auto_snek_offered: false,
            splitter_unlocked: false,
            population_unlocked: false,
            splitter_stations: Vec::new(),
            construction_sites: Vec::new(),
            pending_purchases: VecDeque::new(),
            splitter_jobs: Vec::new(),
            juicer_jobs: Vec::new(),
            max_snek_len: None,
            length_targets: vec![LengthTarget {
                length: MIN_SNEK_LEN,
                count: 1,
            }],
            manual_snek: Some(1),
            wiggle_t: 0.0,
            apls_produced: 0,
            sneks_produced: 0,
            juice_produced: 0,
            juicer_effects: Vec::new(),
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
        let construction_sites = save.construction_sites;
        let pending_purchases = save.pending_purchases;
        let juicer_jobs = save.juicer_jobs;
        let splitter_jobs: Vec<_> = save
            .splitter_jobs
            .into_iter()
            .filter(|job| job.is_valid(&sneks, &splitter_stations))
            .collect();
        let splitter_unlocked = save.splitter_unlocked
            || splitter_stations
                .iter()
                .any(|station| station.design == StationDesign::Splitter)
            || !splitter_jobs.is_empty();
        let juice_discovered = save.juice_discovered || save.juice_segments > 0;
        let auto_snek_offered = save.auto_snek_offered || save.auto_snek_unlocked;
        let manual_snek = if save.auto_snek_unlocked {
            None
        } else {
            sneks.first().map(|snek| snek.id)
        };
        let mut game = Self {
            cols,
            rows,
            sneks,
            apples: save.apples,
            apple_capacity: save.apple_capacity,
            juice_segments: save.juice_segments,
            juice_discovered,
            lifetime_apls: save.lifetime_apls,
            juicer_unlocked: save.juicer_unlocked,
            juicer_placement_unlocked: save.juicer_placement_unlocked,
            bootstrap_phase: save.bootstrap_phase,
            bootstrap_juicer_id: save.bootstrap_juicer_id,
            bootstrap_free_juicer_id: save.bootstrap_free_juicer_id,
            bootstrap_cost_segments: save.bootstrap_cost_segments,
            auto_snek_unlocked: save.auto_snek_unlocked,
            auto_snek_offered,
            splitter_unlocked,
            population_unlocked: save.population_unlocked,
            splitter_stations,
            construction_sites,
            pending_purchases,
            splitter_jobs: if splitter_unlocked {
                splitter_jobs
            } else {
                Vec::new()
            },
            juicer_jobs,
            max_snek_len: save.max_snek_len,
            length_targets: save.length_targets,
            manual_snek,
            wiggle_t: 0.0,
            apls_produced: 0,
            sneks_produced: 0,
            juice_produced: 0,
            juicer_effects: Vec::new(),
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
            apple_capacity: self.apple_capacity,
            juice_segments: self.juice_segments,
            juice_discovered: self.juice_discovered,
            lifetime_apls: self.lifetime_apls,
            juicer_unlocked: self.juicer_unlocked,
            juicer_placement_unlocked: self.juicer_placement_unlocked,
            bootstrap_phase: self.bootstrap_phase,
            bootstrap_juicer_id: self.bootstrap_juicer_id,
            bootstrap_free_juicer_id: self.bootstrap_free_juicer_id,
            bootstrap_cost_segments: self.bootstrap_cost_segments.clone(),
            auto_snek_unlocked: self.auto_snek_unlocked,
            auto_snek_offered: self.auto_snek_offered,
            splitter_unlocked: self.splitter_unlocked,
            population_unlocked: self.population_unlocked,
            splitter_stations: self.splitter_stations.clone(),
            construction_sites: self.construction_sites.clone(),
            pending_purchases: self.pending_purchases.clone(),
            splitter_jobs: self.splitter_jobs.clone(),
            juicer_jobs: self.juicer_jobs.clone(),
            max_snek_len: self.max_snek_len,
            length_targets: self.length_targets.clone(),
        }
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let (next_cols, next_rows) = self.required_world_size(cols, rows);
        let target_apple_count = self.apple_capacity;
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
        self.refresh_unlock_offers();
        self.juicer_effects.retain_mut(|effect| {
            effect.remaining = effect.remaining.saturating_sub(1);
            effect.remaining > 0
        });
        self.advance_juicer_jobs();
        self.settle_juice_purchases(rng);
        self.advance_bootstrap();
        self.advance_cutting_jobs();
        self.finish_releasing_jobs();
        self.reconcile_population_plan();
        self.suspend_uncommitted_splitter_jobs();
        self.assign_splitter_jobs();
        let splitter_dispatch = self.splitter_dispatch();
        self.update_autopilot_directions(&splitter_dispatch);

        let initial_snek_count = self.sneks.len();
        let growth_targets = self.growth_targets();
        let task_growth_needed = self.task_growth_needed();
        let mut juiced_sneks = Vec::new();
        for (index, growth_target) in growth_targets
            .into_iter()
            .enumerate()
            .take(initial_snek_count)
        {
            let snek_id = self.sneks[index].id;
            if self.snek_is_cutting(snek_id) || self.bootstrap_snek_is_cutting(snek_id) {
                continue;
            }
            if matches!(
                self.bootstrap_phase,
                BootstrapPhase::Routing { snek_id: id } if id == snek_id
            ) && self.sneks[index].head() == self.bootstrap_center()
            {
                let center = self.bootstrap_center();
                self.bootstrap_phase = BootstrapPhase::Cutting {
                    snek_id,
                    elapsed: 0,
                    center,
                };
                continue;
            }
            let old_head = self.sneks[index].head();
            let next = self.next_snek_cell(index, &splitter_dispatch);
            let ate_apple = self
                .apples
                .iter()
                .position(|apple| {
                    *apple == next
                        && (task_growth_needed
                            || growth_target.is_none_or(|target| self.sneks[index].len < target))
                })
                .map(|apple_index| {
                    self.apples.remove(apple_index);
                })
                .is_some();
            self.sneks[index].step_to(next, ate_apple);
            if ate_apple {
                self.apls_produced = self.apls_produced.saturating_add(1);
                self.lifetime_apls = self.lifetime_apls.saturating_add(1);
            }
            self.sneks[index].growth_stall_steps = if ate_apple {
                0
            } else if growth_target.is_some_and(|target| self.sneks[index].len < target) {
                self.sneks[index].growth_stall_steps.saturating_add(1)
            } else {
                0
            };
            self.advance_job_after_move(snek_id, old_head, next);
            if matches!(
                self.bootstrap_phase,
                BootstrapPhase::Routing { snek_id: id } if id == snek_id
            ) && next == self.bootstrap_center()
            {
                let center = self.bootstrap_center();
                self.bootstrap_phase = BootstrapPhase::Cutting {
                    snek_id,
                    elapsed: 0,
                    center,
                };
                continue;
            }
            if let Some(station) = self
                .station_cutting(next)
                .filter(|station| next == station.cut_target())
            {
                match station.design {
                    StationDesign::Juicer
                        if !self.station_busy(station)
                            && !self.snek_has_active_splitter_job(snek_id)
                            && !self.snek_has_juicer_job(snek_id) =>
                    {
                        let juicing = splitter_dispatch.juicing_assignment(index);
                        let task_demand = self.construction_self_juice_needed() > 0
                            || self.purchase_juice_needed() > 0;
                        let dispose = juicing.is_none()
                            && !task_demand
                            && self.sneks[index].len == MIN_SNEK_LEN
                            && self.sneks.len().saturating_sub(juiced_sneks.len()) > 1;
                        if self.sneks[index].len > MIN_SNEK_LEN || dispose {
                            let reserved = self.reserved_segments_for_snek(self.sneks[index].id);
                            let available = self.sneks[index]
                                .available_tail_segments()
                                .saturating_sub(reserved);
                            let feed_count = if let Some(assignment) = juicing {
                                Some(assignment.segments.min(available))
                            } else if task_demand {
                                Some(0)
                            } else if reserved > 0 {
                                Some(available)
                            } else {
                                None
                            };
                            let started = self.start_juicer_job(
                                index,
                                station,
                                dispose,
                                feed_count,
                                juicing.map(|assignment| assignment.purpose),
                            );
                            if dispose && started {
                                juiced_sneks.push(snek_id);
                            } else if !started {
                                self.sneks[index].exit_override =
                                    Some(station.old_exit_direction());
                            }
                        } else {
                            self.sneks[index].exit_override = Some(station.old_exit_direction());
                        }
                    }
                    StationDesign::Splitter
                        if !self.snek_has_splitter_job(snek_id)
                            && !self.snek_has_juicer_job(snek_id) =>
                    {
                        if self.pending_purchases.is_empty()
                            && self.sneks[index].len >= MIN_SNEK_LEN * 2
                            && !self.station_busy(station)
                        {
                            self.splitter_jobs.push(SplitterJob {
                                source_snek_id: snek_id,
                                phase: SplitterJobPhase::Feeding {
                                    snek_id,
                                    station,
                                    advanced: 0,
                                },
                            });
                        } else {
                            self.sneks[index].exit_override = Some(station.old_exit_direction());
                        }
                    }
                    _ => {}
                }
            }
        }

        if !juiced_sneks.is_empty() {
            self.sneks.retain(|snek| !juiced_sneks.contains(&snek.id));
            if self
                .manual_snek
                .is_some_and(|id| juiced_sneks.contains(&id))
            {
                self.manual_snek = None;
            }
        }

        self.finish_releasing_jobs();
        self.settle_juice_purchases(rng);
        self.ensure_apples(rng);
        self.refresh_unlock_offers();
        self.tick_wiggle();
    }

    fn start_juicer_job(
        &mut self,
        snek_index: usize,
        station: SplitterStation,
        dispose: bool,
        feed_count: Option<usize>,
        purpose: Option<JuicingPurpose>,
    ) -> bool {
        let snek = &mut self.sneks[snek_index];
        if self
            .juicer_jobs
            .iter()
            .any(|job| job.source_snek_id == snek.id || job.station.id == station.id)
        {
            return false;
        }
        let source_snek_id = snek.id;
        let (retained_snek_id, feed_segments, approach_path, returned_cargo) = if dispose {
            (
                None,
                std::mem::take(&mut snek.positions),
                VecDeque::new(),
                std::mem::take(&mut snek.carried_juice_segments),
            )
        } else {
            let feed_count = feed_count
                .unwrap_or_else(|| snek.len.saturating_sub(MIN_SNEK_LEN))
                .min(snek.available_tail_segments());
            if feed_count == 0 {
                return false;
            }
            let retained_len = snek.len - feed_count;
            let retained_cargo = snek.carried_juice_segments.min(retained_len);
            let returned_cargo = snek.carried_juice_segments - retained_cargo;
            snek.carried_juice_segments = retained_cargo;
            let retained = snek.positions.split_off(feed_count);
            let approach_path = retained.iter().copied().collect();
            let feed_segments = std::mem::replace(&mut snek.positions, retained);
            snek.len = snek.positions.len();
            snek.exit_override = Some(station.old_exit_direction());
            (
                Some(source_snek_id),
                feed_segments,
                approach_path,
                returned_cargo,
            )
        };
        if feed_segments.is_empty() {
            return false;
        }
        self.juicer_jobs.push(JuicerJob {
            station,
            source_snek_id,
            retained_snek_id,
            gradient_len: feed_segments.len(),
            feed_segments,
            approach_path,
            purpose,
        });
        self.juice_segments = self
            .juice_segments
            .saturating_add(returned_cargo.saturating_mul(SEGMENTS_PER_JUICE));
        true
    }

    fn advance_juicer_jobs(&mut self) {
        let mut completed = Vec::new();
        for (index, job) in self.juicer_jobs.iter_mut().enumerate() {
            for _ in 0..2 {
                if job.feed_segments.back() == Some(&job.station.juicer_feed_target()) {
                    job.feed_segments.pop_back();
                    self.juice_segments = self.juice_segments.saturating_add(1);
                    self.juice_produced = self.juice_produced.saturating_add(1);
                    self.juice_discovered = true;
                    self.juicer_effects.push(JuicerEffect {
                        station: job.station,
                        remaining: JUICER_EFFECT_STEPS,
                    });
                } else if let Some(feed_end) = job.feed_segments.back().copied() {
                    let previous = job.feed_segments.clone();
                    let last = job.feed_segments.len() - 1;
                    let next = job.approach_path.pop_front().unwrap_or_else(|| {
                        juicer_feed_step(job.station, feed_end, self.cols, self.rows)
                    });
                    job.feed_segments[last] = next;
                    for segment in 0..last {
                        job.feed_segments[segment] = previous[segment + 1];
                    }
                    if job.station.occupies(next) {
                        break;
                    }
                }
                if job.feed_segments.is_empty() {
                    break;
                }
            }
            if job.feed_segments.is_empty() {
                completed.push(index);
            }
        }
        for index in completed.into_iter().rev() {
            self.juicer_jobs.remove(index);
        }
    }

    pub(super) fn set_direction(&mut self, direction: Direction) {
        let Some(id) = self.controlled_snek_id() else {
            return;
        };
        if self.snek_has_juicer_job(id) {
            return;
        }
        if let Some(snek) = self.sneks.iter_mut().find(|snek| snek.id == id) {
            snek.direction = direction;
        }
    }

    pub(super) fn tap_cell(&mut self, cell: Cell) -> bool {
        if !self.auto_snek_unlocked {
            return false;
        }

        let selected = self
            .sneks
            .iter()
            .find(|snek| snek.contains_near(cell, self.cols, self.rows))
            .map(|snek| snek.id);
        self.manual_snek = selected;
        selected.is_some()
    }

    pub(super) fn buy_auto_snek(&mut self) -> bool {
        self.queue_juice_purchase(JuicePurchase::AutoSnek)
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
        if !self.can_place_station(design)
            || self
                .splitter_stations
                .iter()
                .any(|existing| stations_conflict(*existing, station))
        {
            return false;
        }
        let station = self.fit_station_with_buffer(station);
        self.splitter_stations.push(station);
        if design == StationDesign::Splitter {
            self.population_unlocked = true;
            self.refresh_unlock_offers();
        }
        self.construction_sites.push(ConstructionSite {
            station_id: station.id,
            remaining_segments: design.cost(),
        });
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
                other_index != index && stations_conflict(*existing, moved)
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
        let bootstrap_juicer = self.bootstrap_juicer_id == Some(station.id);
        let free_juicer = self.bootstrap_free_juicer_id == Some(station.id);
        let replacement_bootstrap = self
            .splitter_stations
            .iter()
            .find(|other| {
                other.id != station.id
                    && other.design == StationDesign::Juicer
                    && self.station_complete(**other)
            })
            .map(|station| station.id);
        if bootstrap_juicer && replacement_bootstrap.is_none() {
            return false;
        }
        self.splitter_stations.remove(index);
        if bootstrap_juicer {
            self.bootstrap_juicer_id = replacement_bootstrap;
        }
        let total_cost = station.design.cost();
        let refund_cups = if free_juicer {
            0
        } else {
            self.construction_sites
                .iter()
                .find(|site| site.station_id == station.id)
                .map_or(total_cost, |site| total_cost - site.remaining_segments)
        };
        self.construction_sites
            .retain(|site| site.station_id != station.id);
        self.reclaim_excess_construction_cargo();
        self.juice_segments = self
            .juice_segments
            .saturating_add(refund_cups.saturating_mul(SEGMENTS_PER_JUICE));
        true
    }

    pub(super) fn can_buy_auto_snek(&self) -> bool {
        self.can_queue_juice_purchase(JuicePurchase::AutoSnek)
    }

    pub(super) fn can_unlock_juicer(&self) -> bool {
        self.bootstrap_phase == BootstrapPhase::Locked
            && self.spendable_apls() >= JUICER_UNLOCK_COST
    }

    pub(super) fn unlock_queue_available(&self) -> bool {
        self.bootstrap_phase != BootstrapPhase::Locked || self.apls() >= JUICER_UNLOCK_COST
    }

    pub(super) fn splitter_unlock_visible(&self) -> bool {
        self.lifetime_apls >= 50
    }

    pub(super) fn juicer_upgrade_visible(&self) -> bool {
        self.lifetime_apls >= 250
    }

    pub(super) fn apple_unlock_visible(&self) -> bool {
        self.lifetime_apls >= 250
    }

    pub(super) fn auto_snek_offer_visible(&self) -> bool {
        self.auto_snek_offered || self.juicer_unlocked
    }

    fn refresh_unlock_offers(&mut self) {
        if self.juicer_unlocked {
            self.auto_snek_offered = true;
        }
    }

    fn bootstrap_snek_is_cutting(&self, snek_id: usize) -> bool {
        matches!(
            self.bootstrap_phase,
            BootstrapPhase::Cutting { snek_id: id, .. }
                | BootstrapPhase::Walking { snek_id: id, .. }
                | BootstrapPhase::Flash { snek_id: id, .. }
                if id == snek_id
        )
    }

    fn advance_bootstrap(&mut self) {
        match self.bootstrap_phase {
            BootstrapPhase::Cutting {
                snek_id,
                mut elapsed,
                center,
            } => {
                elapsed = elapsed.saturating_add(1);
                if elapsed >= BOOTSTRAP_CUT_STEPS {
                    let direction = if (snek_id + self.lifetime_apls).is_multiple_of(2) {
                        Direction::Left
                    } else {
                        Direction::Right
                    };
                    if self.detach_bootstrap_cost(snek_id) {
                        self.bootstrap_phase = BootstrapPhase::Walking {
                            snek_id,
                            direction,
                            elapsed: 0,
                            center,
                        };
                    }
                } else {
                    self.bootstrap_phase = BootstrapPhase::Cutting {
                        snek_id,
                        elapsed,
                        center,
                    };
                }
            }
            BootstrapPhase::Walking {
                snek_id,
                direction,
                mut elapsed,
                center,
            } => {
                elapsed = elapsed.saturating_add(1);
                self.bootstrap_phase = if elapsed >= BOOTSTRAP_WALK_STEPS {
                    BootstrapPhase::Flash {
                        snek_id,
                        elapsed: 0,
                        direction,
                        center,
                    }
                } else {
                    BootstrapPhase::Walking {
                        snek_id,
                        direction,
                        elapsed,
                        center,
                    }
                };
            }
            BootstrapPhase::Flash {
                snek_id,
                mut elapsed,
                direction,
                center,
            } => {
                elapsed = elapsed.saturating_add(1);
                self.bootstrap_phase = if elapsed >= BOOTSTRAP_FLASH_STEPS {
                    self.place_bootstrap_juicer_home();
                    BootstrapPhase::Complete
                } else {
                    BootstrapPhase::Flash {
                        snek_id,
                        elapsed,
                        direction,
                        center,
                    }
                };
            }
            BootstrapPhase::Locked | BootstrapPhase::Routing { .. } | BootstrapPhase::Complete => {}
        }
    }

    fn detach_bootstrap_cost(&mut self, snek_id: usize) -> bool {
        let Some(snek) = self.sneks.iter_mut().find(|snek| snek.id == snek_id) else {
            return false;
        };
        if snek.available_tail_segments() < JUICER_UNLOCK_COST {
            return false;
        }
        let retained = snek.positions.split_off(JUICER_UNLOCK_COST);
        self.bootstrap_cost_segments = std::mem::replace(&mut snek.positions, retained);
        snek.len -= JUICER_UNLOCK_COST;
        true
    }

    fn place_bootstrap_juicer_home(&mut self) {
        let origin = Cell::new(
            BUILDING_BUFFER,
            self.rows
                .saturating_sub(StationDesign::Juicer.height() + BUILDING_BUFFER),
        );
        let station = SplitterStation::new(origin, 0, StationDesign::Juicer, self.cols, self.rows)
            .with_id(self.next_station_id());
        self.bootstrap_juicer_id = Some(station.id);
        self.bootstrap_free_juicer_id = Some(station.id);
        self.splitter_stations.push(station);
        self.bootstrap_cost_segments.clear();
        self.juicer_unlocked = true;
        self.auto_snek_offered = true;
        self.remove_blocked_apples();
        self.ensure_apple_count_without_rng(self.apple_capacity);
    }

    fn bootstrap_center(&self) -> Cell {
        Cell::new(self.cols / 2, self.rows / 2)
    }

    pub(super) fn bootstrap_activity(&self) -> Option<BootstrapActivity> {
        let (phase, progress, direction, flash_origin) = match self.bootstrap_phase {
            BootstrapPhase::Routing { .. } => (
                BootstrapActivityPhase::Routing,
                0.0,
                Direction::Left,
                self.bootstrap_center(),
            ),
            BootstrapPhase::Cutting {
                elapsed, center, ..
            } => (
                BootstrapActivityPhase::Cutting,
                elapsed as f64 / BOOTSTRAP_CUT_STEPS as f64,
                Direction::Left,
                center,
            ),
            BootstrapPhase::Walking {
                direction,
                elapsed,
                center,
                ..
            } => (
                BootstrapActivityPhase::Walking,
                elapsed as f64 / BOOTSTRAP_WALK_STEPS as f64,
                direction,
                Cell::new(
                    if direction == Direction::Left {
                        0
                    } else {
                        self.cols.saturating_sub(1)
                    },
                    center.row,
                ),
            ),
            BootstrapPhase::Flash {
                elapsed,
                direction,
                center,
                ..
            } => (
                BootstrapActivityPhase::Flash,
                elapsed as f64 / BOOTSTRAP_FLASH_STEPS.saturating_sub(1).max(1) as f64,
                direction,
                Cell::new(
                    if direction == Direction::Left {
                        0
                    } else {
                        self.cols.saturating_sub(1)
                    },
                    center.row,
                ),
            ),
            BootstrapPhase::Locked | BootstrapPhase::Complete => return None,
        };
        Some(BootstrapActivity {
            phase,
            progress,
            center: match self.bootstrap_phase {
                BootstrapPhase::Cutting { center, .. }
                | BootstrapPhase::Walking { center, .. }
                | BootstrapPhase::Flash { center, .. } => center,
                _ => self.bootstrap_center(),
            },
            flash_origin,
            direction,
        })
    }

    pub(super) fn bootstrap_cost_segments(&self) -> &VecDeque<Cell> {
        &self.bootstrap_cost_segments
    }

    pub(super) fn unlock_juicer(&mut self) -> bool {
        let Some(snek_id) = self
            .controlled_snek_id()
            .or_else(|| self.sneks.first().map(|s| s.id))
        else {
            return false;
        };
        if !self.can_unlock_juicer() {
            return false;
        }
        self.bootstrap_phase = BootstrapPhase::Routing { snek_id };
        true
    }

    pub(super) fn juicer_unlocked(&self) -> bool {
        self.juicer_unlocked
    }

    pub(super) fn can_unlock_juicer_placement(&self) -> bool {
        self.can_queue_juice_purchase(JuicePurchase::JuicerPlacement)
    }

    pub(super) fn buy_juicer_placement_unlock(&mut self) -> bool {
        self.queue_juice_purchase(JuicePurchase::JuicerPlacement)
    }

    pub(super) fn juicer_placement_unlocked(&self) -> bool {
        self.juicer_placement_unlocked
    }

    pub(super) fn juice(&self) -> usize {
        self.juice_segments / SEGMENTS_PER_JUICE
    }

    pub(super) fn juice_discovered(&self) -> bool {
        self.juice_discovered
    }

    pub(super) fn can_unlock_splitter_station(&self) -> bool {
        self.can_queue_juice_purchase(JuicePurchase::Splitter)
    }

    pub(super) fn buy_splitter_unlock(&mut self) -> bool {
        self.queue_juice_purchase(JuicePurchase::Splitter)
    }

    pub(super) fn splitter_unlocked(&self) -> bool {
        self.splitter_unlocked
    }

    pub(super) fn toolbelt_available(&self) -> bool {
        self.juicer_placement_unlocked || self.splitter_unlocked
    }

    pub(super) fn can_place_station(&self, design: StationDesign) -> bool {
        match design {
            StationDesign::Juicer => self.juicer_placement_unlocked,
            StationDesign::Splitter => self.splitter_unlocked,
        }
    }

    pub(super) fn station_draft_valid(
        &self,
        origin: Cell,
        rotation: u8,
        design: StationDesign,
        moving_index: Option<usize>,
    ) -> bool {
        if moving_index.is_none() && !self.can_place_station(design) {
            return false;
        }
        let candidate = SplitterStation::new(origin, rotation, design, self.cols, self.rows);
        !self
            .splitter_stations
            .iter()
            .enumerate()
            .any(|(index, station)| {
                moving_index != Some(index) && stations_conflict(*station, candidate)
            })
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

    #[cfg(test)]
    pub(super) fn buy_snek(&mut self) -> bool {
        let Some(source_snek_id) = self.select_snek_for_order() else {
            return false;
        };
        self.splitter_jobs.push(SplitterJob::queued(source_snek_id));
        true
    }

    fn reconcile_population_plan(&mut self) {
        if !self.pending_purchases.is_empty()
            || !self.splitter_unlocked
            || !self.population_unlocked()
        {
            return;
        }
        let desired = self
            .length_targets
            .iter()
            .map(|target| target.count)
            .sum::<usize>()
            .clamp(1, MAX_SNEKS);
        let planned = self.sneks.len().saturating_add(self.splitter_jobs.len());
        for _ in planned..desired {
            let Some(source_snek_id) = self.select_snek_for_order() else {
                break;
            };
            self.splitter_jobs.push(SplitterJob::queued(source_snek_id));
        }
        let mut excess = planned.saturating_sub(desired);
        while excess > 0 {
            let Some(index) = self
                .splitter_jobs
                .iter()
                .rposition(|job| job.phase == SplitterJobPhase::Queued)
            else {
                break;
            };
            self.splitter_jobs.remove(index);
            excess -= 1;
        }
    }

    pub(super) fn can_buy_apple(&self) -> bool {
        self.can_queue_juice_purchase(JuicePurchase::Apple)
    }

    pub(super) fn buy_apple(&mut self, _rng: &mut Lcg) -> bool {
        self.queue_juice_purchase(JuicePurchase::Apple)
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

    pub(super) fn max_snek_len(&self) -> Option<usize> {
        self.max_snek_len
    }

    pub(super) fn adjust_max_snek_len(&mut self, delta: isize) {
        let current_longest = self
            .sneks
            .iter()
            .map(Snek::len)
            .max()
            .unwrap_or(MIN_SNEK_LEN);
        let next = self
            .max_snek_len
            .unwrap_or(current_longest)
            .saturating_add_signed(delta)
            .clamp(MIN_SNEK_LEN, MAX_SNEK_LEN);
        self.max_snek_len = Some(next);
        self.length_targets.retain(|target| target.length <= next);
    }

    pub(super) fn length_targets(&self) -> &[LengthTarget] {
        &self.length_targets
    }

    pub(super) fn adjust_length_target(&mut self, length: usize, delta: isize) {
        let Some(max_snek_len) = self.max_snek_len else {
            return;
        };
        if !(MIN_SNEK_LEN..max_snek_len).contains(&length) {
            return;
        }
        if length == MIN_SNEK_LEN
            && delta < 0
            && self
                .length_targets
                .iter()
                .find(|target| target.length == length)
                .is_some_and(|target| target.count == 1)
        {
            return;
        }
        if delta > 0
            && self
                .length_targets
                .iter()
                .map(|target| target.count)
                .sum::<usize>()
                >= MAX_SNEKS
        {
            return;
        }
        if let Some(target) = self
            .length_targets
            .iter_mut()
            .find(|target| target.length == length)
        {
            target.count = target.count.saturating_add_signed(delta).min(MAX_SNEKS);
            if target.count == 0 {
                self.length_targets.retain(|target| target.count > 0);
            }
        } else if delta > 0 {
            self.length_targets.push(LengthTarget {
                length,
                count: delta as usize,
            });
            self.length_targets.sort_by_key(|target| target.length);
        }
    }

    pub(super) fn splitter_stations(&self) -> &[SplitterStation] {
        &self.splitter_stations
    }

    pub(super) fn population_unlocked(&self) -> bool {
        self.population_unlocked
    }

    pub(super) fn juicer_activity(&self, station: SplitterStation) -> Option<f64> {
        self.juicer_effects
            .iter()
            .filter(|effect| effect.station.id == station.id)
            .map(|effect| effect.remaining as f64 / JUICER_EFFECT_STEPS as f64)
            .max_by(f64::total_cmp)
    }

    pub(super) fn juicer_busy(&self, station: SplitterStation) -> bool {
        self.juicer_jobs
            .iter()
            .any(|job| job.station.id == station.id)
    }

    pub(super) fn juicer_feeds(&self) -> impl Iterator<Item = (&VecDeque<Cell>, usize)> + '_ {
        self.juicer_jobs
            .iter()
            .map(|job| (&job.feed_segments, job.gradient_len))
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

    pub(super) fn production_totals(&self) -> (usize, usize, usize) {
        (self.apls_produced, self.sneks_produced, self.juice_produced)
    }

    #[cfg(test)]
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
        self.lifetime_apls = self.lifetime_apls.saturating_add(count);
        self.refresh_unlock_offers();
    }

    #[cfg(debug_assertions)]
    pub(super) fn debug_add_juice(&mut self, cups: usize) {
        self.juice_segments = self
            .juice_segments
            .saturating_add(cups * SEGMENTS_PER_JUICE);
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
        let logistics = self.logistics_assignments();
        let mut navigation =
            NavigationCache::new(self.cols, self.rows, self.splitter_stations.clone());
        for (index, assignment) in assignments.into_iter().enumerate() {
            let snek = &self.sneks[index];
            let bootstrap_target = matches!(
                self.bootstrap_phase,
                BootstrapPhase::Routing { snek_id } if snek_id == snek.id
            )
            .then(|| self.bootstrap_center());
            let station_assignment = splitter_dispatch.station_for_snek(index);
            let logistics_assignment = logistics[index];
            if bootstrap_target.is_none()
                && self.auto_snek_unlocked
                && self.manual_snek == Some(snek.id)
                && !self.snek_has_active_splitter_job(snek.id)
                && !self.snek_has_juicer_job(snek.id)
            {
                continue;
            }
            if bootstrap_target.is_none()
                && station_assignment.is_none()
                && logistics_assignment.is_none()
                && (self.manual_snek == Some(snek.id) || !self.auto_snek_unlocked)
            {
                continue;
            }
            let direction = if let Some(target) = bootstrap_target {
                navigation.direction_toward(
                    snek.head(),
                    target,
                    snek.direction,
                    PathMode::AvoidStation,
                )
            } else if let Some((approach, service)) = logistics_assignment {
                if snek.head() == approach {
                    direction_between(approach, service).unwrap_or(snek.direction)
                } else {
                    navigation.direction_toward(
                        snek.head(),
                        approach,
                        snek.direction,
                        PathMode::AvoidStation,
                    )
                }
            } else if let Some(station) = station_assignment {
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

    fn logistics_assignments(&self) -> Vec<Option<(Cell, Cell)>> {
        let mut assignments = vec![None; self.sneks.len()];
        let construction: Vec<_> = self
            .construction_sites
            .iter()
            .filter_map(|site| {
                self.splitter_stations
                    .iter()
                    .copied()
                    .find(|station| station.id == site.station_id)
            })
            .collect();
        if construction.is_empty() {
            return assignments;
        }
        let loaders: Vec<_> = self
            .splitter_stations
            .iter()
            .copied()
            .filter(|station| {
                station.design == StationDesign::Juicer && self.station_complete(*station)
            })
            .collect();
        let mut used_sneks = vec![false; self.sneks.len()];
        let unallocated = self.unallocated_construction_segments();
        if self.construction_juice_available() >= SEGMENTS_PER_JUICE
            && unallocated > 0
            && !loaders.is_empty()
        {
            let mut used_loaders = vec![false; loaders.len()];
            let mut pairs = Vec::new();
            for (snek_index, snek) in self.sneks.iter().enumerate() {
                if snek.carried_juice_segments >= snek.len
                    || self.manual_snek == Some(snek.id)
                    || self.snek_has_active_splitter_job(snek.id)
                    || self.snek_has_juicer_job(snek.id)
                {
                    continue;
                }
                for (loader_index, station) in loaders.iter().copied().enumerate() {
                    let approach = station.service_approach_cell(self.cols, self.rows);
                    pairs.push((
                        toroidal_distance(snek.head(), approach, self.cols, self.rows),
                        snek_index,
                        loader_index,
                        station,
                    ));
                }
            }
            pairs.sort_by_key(|(distance, snek, loader, _)| (*distance, *loader, *snek));
            for (_, snek_index, loader_index, station) in pairs {
                if used_sneks[snek_index] || used_loaders[loader_index] {
                    continue;
                }
                used_sneks[snek_index] = true;
                used_loaders[loader_index] = true;
                assignments[snek_index] = Some((
                    station.service_approach_cell(self.cols, self.rows),
                    station.service_cell(),
                ));
            }
        }

        let mut pairs = Vec::new();
        for (snek_index, snek) in self.sneks.iter().enumerate() {
            if used_sneks[snek_index]
                || snek.carried_juice_segments == 0
                || self.manual_snek == Some(snek.id)
                || self.snek_has_active_splitter_job(snek.id)
                || self.snek_has_juicer_job(snek.id)
            {
                continue;
            }
            for station in construction.iter().copied() {
                if let Some((approach, target)) =
                    self.station_delivery_contact(station, snek.head())
                {
                    pairs.push((
                        toroidal_distance(snek.head(), approach, self.cols, self.rows),
                        snek_index,
                        station.id,
                        approach,
                        target,
                    ));
                }
            }
        }
        pairs.sort_by_key(|(distance, snek, station, _, _)| (*distance, *station, *snek));
        for (_, snek_index, _, approach, target) in pairs {
            if used_sneks[snek_index] {
                continue;
            }
            used_sneks[snek_index] = true;
            assignments[snek_index] = Some((approach, target));
        }
        assignments
    }

    fn unallocated_construction_segments(&self) -> usize {
        let remaining = self
            .construction_sites
            .iter()
            .map(|site| site.remaining_segments)
            .sum::<usize>();
        let carried = self
            .sneks
            .iter()
            .map(|snek| snek.carried_juice_segments)
            .sum::<usize>();
        remaining.saturating_sub(carried)
    }

    fn reclaim_excess_construction_cargo(&mut self) {
        let demand = self
            .construction_sites
            .iter()
            .map(|site| site.remaining_segments)
            .sum::<usize>();
        let carried = self
            .sneks
            .iter()
            .map(|snek| snek.carried_juice_segments)
            .sum::<usize>();
        let mut excess = carried.saturating_sub(demand);
        let mut reclaimed = 0usize;
        for snek in self.sneks.iter_mut().rev() {
            let take = excess.min(snek.carried_juice_segments);
            snek.carried_juice_segments -= take;
            excess -= take;
            reclaimed += take;
            if excess == 0 {
                break;
            }
        }
        self.juice_segments = self
            .juice_segments
            .saturating_add(reclaimed.saturating_mul(SEGMENTS_PER_JUICE));
    }

    fn station_delivery_contact(
        &self,
        station: SplitterStation,
        from: Cell,
    ) -> Option<(Cell, Cell)> {
        let mut best = None;
        for row in station.origin.row..station.origin.row + station.height() {
            for col in station.origin.col..station.origin.col + station.width() {
                let target = Cell::new(col, row);
                if !station.occupies(target) {
                    continue;
                }
                for direction in [
                    Direction::Up,
                    Direction::Down,
                    Direction::Left,
                    Direction::Right,
                ] {
                    let approach = target.stepped(direction, self.cols, self.rows);
                    if station.occupies(approach) || self.cell_blocked(approach) {
                        continue;
                    }
                    let key = (
                        toroidal_distance(from, approach, self.cols, self.rows),
                        approach.row,
                        approach.col,
                        target.row,
                        target.col,
                    );
                    if best.is_none_or(|(current, _, _)| key < current) {
                        best = Some((key, approach, target));
                    }
                }
            }
        }
        best.map(|(_, approach, target)| (approach, target))
    }

    fn apple_assignments(&self) -> Vec<Option<Cell>> {
        let mut assignments = vec![None; self.sneks.len()];
        if self.apples.is_empty() {
            return assignments;
        }

        let growth_targets = self.growth_targets();
        let task_growth_needed = self.task_growth_needed();
        let mut pairs = Vec::new();
        for (snek_index, snek) in self.sneks.iter().enumerate() {
            if self.manual_snek == Some(snek.id)
                || self.snek_has_active_splitter_job(snek.id)
                || self.snek_has_juicer_job(snek.id)
                || (!task_growth_needed
                    && growth_targets[snek_index].is_some_and(|target| snek.len >= target))
            {
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

    fn growth_targets(&self) -> Vec<Option<usize>> {
        let mut targets = Vec::new();
        for target in &self.length_targets {
            targets.extend(std::iter::repeat_n(target.length, target.count));
        }
        targets.sort_unstable();

        let mut snek_indices: Vec<_> = (0..self.sneks.len()).collect();
        snek_indices.sort_by_key(|index| self.sneks[*index].len);
        snek_indices.pop();

        let mut result = vec![None; self.sneks.len()];
        for (index, target) in snek_indices.into_iter().zip(targets) {
            result[index] = Some(target);
        }
        result
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
            || self
                .juicer_jobs
                .iter()
                .any(|job| job.station.id == station.id)
    }

    fn snek_has_splitter_job(&self, snek_id: usize) -> bool {
        self.splitter_jobs.iter().any(|job| job.uses_snek(snek_id))
    }

    fn snek_has_active_splitter_job(&self, snek_id: usize) -> bool {
        self.splitter_jobs
            .iter()
            .any(|job| job.phase != SplitterJobPhase::Queued && job.uses_snek(snek_id))
    }

    fn snek_has_juicer_job(&self, snek_id: usize) -> bool {
        self.juicer_jobs
            .iter()
            .any(|job| job.retained_snek_id == Some(snek_id))
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
        if !self.pending_purchases.is_empty() || !self.construction_sites.is_empty() {
            return;
        }
        if !self.splitter_stations.iter().any(|station| {
            station.design == StationDesign::Splitter && self.station_complete(*station)
        }) {
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
                    if station.design != StationDesign::Splitter || !self.station_complete(station)
                    {
                        continue;
                    }
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

    fn suspend_uncommitted_splitter_jobs(&mut self) {
        if self.pending_purchases.is_empty() && self.construction_sites.is_empty() {
            return;
        }
        for job in &mut self.splitter_jobs {
            if matches!(job.phase, SplitterJobPhase::Routing { .. }) {
                job.phase = SplitterJobPhase::Queued;
            }
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
            self.sneks[snek_index].split_head(new_snek_id, station.new_exit_direction())
        else {
            self.splitter_jobs[job_index].phase = SplitterJobPhase::Queued;
            return;
        };
        self.sneks[snek_index].direction = station.old_exit_direction();
        self.sneks[snek_index].exit_override = Some(station.old_exit_direction());
        self.sneks.push(new_snek);
        self.sneks_produced = self.sneks_produced.saturating_add(1);
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
        let target_count = self.apple_capacity;
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

    #[cfg(test)]
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
                .filter(|(_, snek)| {
                    snek.available_tail_segments() > self.reserved_segments_for_snek(snek.id)
                })
                .max_by_key(|(_, snek)| {
                    snek.available_tail_segments() - self.reserved_segments_for_snek(snek.id)
                })
                .map(|(index, _)| index)
            else {
                return false;
            };
            let spendable = self.sneks[index]
                .available_tail_segments()
                .saturating_sub(self.reserved_segments_for_snek(self.sneks[index].id));
            let spend = spendable.min(remaining);
            self.sneks[index].remove_tail_segments(spend);
            remaining -= spend;
        }

        true
    }

    fn spend_juice(&mut self, cups: usize) -> bool {
        let Some(segments) = cups.checked_mul(SEGMENTS_PER_JUICE) else {
            return false;
        };
        if self.juice_segments < segments {
            return false;
        }
        self.juice_segments -= segments;
        true
    }

    fn purchase_prerequisite_met(&self, purchase: JuicePurchase) -> bool {
        match purchase {
            JuicePurchase::AutoSnek => self.auto_snek_offer_visible() && !self.auto_snek_unlocked,
            JuicePurchase::JuicerPlacement => {
                self.juicer_unlocked
                    && self.juicer_upgrade_visible()
                    && !self.juicer_placement_unlocked
            }
            JuicePurchase::Splitter => {
                self.juicer_unlocked && self.splitter_unlock_visible() && !self.splitter_unlocked
            }
            JuicePurchase::Apple => {
                self.apple_unlock_visible()
                    && self.apple_capacity < MAX_SNEKS
                    && self.has_open_apple_cell()
            }
        }
    }

    fn pending_purchase_segments(&self) -> usize {
        self.pending_purchases
            .iter()
            .filter_map(|purchase| purchase.cost().checked_mul(SEGMENTS_PER_JUICE))
            .sum()
    }

    fn future_juice_segments(&self) -> usize {
        let total = self
            .juice_segments
            .saturating_add(
                self.juicer_jobs
                    .iter()
                    .map(|job| job.feed_segments.len())
                    .sum::<usize>(),
            )
            .saturating_add(self.spendable_apls());
        total.saturating_sub(
            self.unallocated_construction_segments()
                .saturating_mul(SEGMENTS_PER_JUICE),
        )
    }

    fn can_queue_juice_purchase(&self, purchase: JuicePurchase) -> bool {
        if !self.purchase_prerequisite_met(purchase)
            || (!matches!(purchase, JuicePurchase::Apple)
                && self.pending_purchases.contains(&purchase))
            || (matches!(purchase, JuicePurchase::Apple)
                && self.apple_capacity.saturating_add(
                    self.pending_purchases
                        .iter()
                        .filter(|pending| matches!(pending, JuicePurchase::Apple))
                        .count(),
                ) >= MAX_SNEKS)
        {
            return false;
        }
        let Some(cost) = purchase.cost().checked_mul(SEGMENTS_PER_JUICE) else {
            return false;
        };
        self.future_juice_segments() >= self.pending_purchase_segments().saturating_add(cost)
    }

    fn queue_juice_purchase(&mut self, purchase: JuicePurchase) -> bool {
        if !self.can_queue_juice_purchase(purchase) {
            return false;
        }
        self.pending_purchases.push_back(purchase);
        true
    }

    fn settle_juice_purchases(&mut self, rng: &mut Lcg) {
        if !self.construction_sites.is_empty() {
            return;
        }
        while let Some(purchase) = self.pending_purchases.front().copied() {
            if !self.purchase_prerequisite_met(purchase) || !self.spend_juice(purchase.cost()) {
                break;
            }
            self.pending_purchases.pop_front();
            match purchase {
                JuicePurchase::AutoSnek => {
                    self.auto_snek_unlocked = true;
                    self.manual_snek = None;
                }
                JuicePurchase::JuicerPlacement => self.juicer_placement_unlocked = true,
                JuicePurchase::Splitter => self.splitter_unlocked = true,
                JuicePurchase::Apple => {
                    self.apple_capacity = self.apple_capacity.saturating_add(1).min(MAX_SNEKS);
                    self.add_apple(rng);
                }
            }
        }
    }

    fn purchase_juice_needed(&self) -> usize {
        self.pending_purchase_segments()
            .saturating_sub(self.juice_segments)
            .saturating_sub(
                self.juicer_jobs
                    .iter()
                    .filter(|job| job.purpose != Some(JuicingPurpose::Construction))
                    .map(|job| job.feed_segments.len())
                    .sum::<usize>(),
            )
    }

    fn construction_juice_available(&self) -> usize {
        if self.construction_sites.is_empty() {
            self.juice_segments
                .saturating_sub(self.pending_purchase_segments())
        } else {
            self.juice_segments
        }
    }

    fn construction_self_juice_needed(&self) -> usize {
        self.unallocated_construction_segments()
            .saturating_mul(SEGMENTS_PER_JUICE)
            .saturating_sub(self.juice_segments)
            .saturating_sub(
                self.juicer_jobs
                    .iter()
                    .map(|job| job.feed_segments.len())
                    .sum::<usize>(),
            )
    }

    fn task_growth_needed(&self) -> bool {
        let demand = if self.construction_sites.is_empty() {
            self.purchase_juice_needed()
        } else {
            self.construction_self_juice_needed()
        };
        demand > self.spendable_apls()
    }

    fn spendable_apls(&self) -> usize {
        self.sneks
            .iter()
            .map(|snek| {
                if self.auto_snek_unlocked && self.manual_snek == Some(snek.id) {
                    0
                } else {
                    snek.available_tail_segments()
                        .saturating_sub(self.reserved_segments_for_snek(snek.id))
                }
            })
            .sum()
    }

    fn next_snek_cell(&mut self, index: usize, splitter_dispatch: &SplitterDispatch) -> Cell {
        let head = self.sneks[index].head();
        let guided_channel = splitter_dispatch.channel_for_snek(index);
        if let Some(station) = self.station_cutting(head) {
            if let Some((assigned_station, channel)) = guided_channel {
                if assigned_station == station {
                    if let Some((direction, next)) =
                        station.route_step(head, channel, self.cols, self.rows)
                    {
                        self.sneks[index].direction = direction;
                        return next;
                    }
                }
            }
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
            if head != station.cut_target() {
                if let Some((direction, next)) =
                    station.route_step(head, SplitterChannel::Entry, self.cols, self.rows)
                {
                    self.sneks[index].direction = direction;
                    return next;
                }
            }
            if let Some((direction, next)) = station.escape_step(head, self.cols, self.rows) {
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
        if self.can_enter_splitter(index, head, next, splitter_dispatch)
            || !self.cell_blocked(next)
            || self.station_escape_allowed(head, next)
        {
            return next;
        }
        if self.handle_service_bump(index, next) {
            return head;
        }

        let reverse = self.sneks[index].direction.reversed();
        self.sneks[index].direction = reverse;
        let bounced = self.sneks[index].next_cell(self.cols, self.rows);
        if self.cell_blocked(bounced)
            && !self.can_enter_splitter(index, head, bounced, splitter_dispatch)
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
                .juicer_jobs
                .iter()
                .any(|job| job.feed_segments.contains(&cell))
            || self
                .splitter_stations
                .iter()
                .any(|station| station.occupies(cell))
    }

    fn handle_service_bump(&mut self, snek_index: usize, target: Cell) -> bool {
        if let Some(site_index) = self.construction_sites.iter().position(|site| {
            self.splitter_stations
                .iter()
                .any(|station| station.id == site.station_id && station.occupies(target))
        }) {
            if self.sneks[snek_index].carried_juice_segments == 0 {
                return false;
            }
            self.sneks[snek_index].carried_juice_segments -= 1;
            self.construction_sites[site_index].remaining_segments -= 1;
            if self.construction_sites[site_index].remaining_segments == 0 {
                self.construction_sites.remove(site_index);
            }
            return true;
        }
        let Some(station) = self
            .splitter_stations
            .iter()
            .copied()
            .find(|station| station.service_cell() == target)
        else {
            return false;
        };
        if station.design == StationDesign::Juicer
            && self.construction_juice_available() >= SEGMENTS_PER_JUICE
            && self.sneks[snek_index].carried_juice_segments < self.sneks[snek_index].len
            && self.unallocated_construction_segments() > 0
        {
            self.juice_segments -= SEGMENTS_PER_JUICE;
            self.sneks[snek_index].carried_juice_segments += 1;
            return true;
        }
        false
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

    fn can_enter_splitter(
        &self,
        index: usize,
        from: Cell,
        cell: Cell,
        splitter_dispatch: &SplitterDispatch,
    ) -> bool {
        self.splitter_stations.iter().copied().any(|station| {
            if !self.station_complete(station) {
                return false;
            }
            let assigned = splitter_dispatch
                .station_for_snek(index)
                .is_some_and(|assigned| assigned.id == station.id);
            let active_station = splitter_dispatch
                .channel_for_snek(index)
                .map(|(active, _)| active.id);
            station.cuts(cell)
                && (station.cuts(from)
                    || (active_station.is_none_or(|active| active == station.id)
                        && (assigned || !self.station_busy(station))
                        && station.legal_entry_step(from, cell, self.cols, self.rows)))
        })
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
        for job in &self.juicer_jobs {
            for cell in &job.feed_segments {
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
        for cell in &mut self.bootstrap_cost_segments {
            cell.col += cols;
            cell.row += rows;
        }
        match &mut self.bootstrap_phase {
            BootstrapPhase::Cutting { center, .. }
            | BootstrapPhase::Walking { center, .. }
            | BootstrapPhase::Flash { center, .. } => {
                center.col += cols;
                center.row += rows;
            }
            BootstrapPhase::Locked | BootstrapPhase::Routing { .. } | BootstrapPhase::Complete => {}
        }
        for station in &mut self.splitter_stations {
            station.origin.col += cols;
            station.origin.row += rows;
        }
        for job in &mut self.splitter_jobs {
            job.shift_station(cols, rows);
        }
        for job in &mut self.juicer_jobs {
            job.station.origin.col += cols;
            job.station.origin.row += rows;
            for cell in &mut job.feed_segments {
                cell.col += cols;
                cell.row += rows;
            }
            for cell in &mut job.approach_path {
                cell.col += cols;
                cell.row += rows;
            }
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
                && !self
                    .juicer_jobs
                    .iter()
                    .any(|job| job.feed_segments.contains(apple))
        });
    }

    fn tick_wiggle(&mut self) {
        self.wiggle_t += TAU / 9.0;
        self.wiggle_t %= TAU;
    }

    fn station_complete(&self, station: SplitterStation) -> bool {
        !self
            .construction_sites
            .iter()
            .any(|site| site.station_id == station.id)
    }

    pub(super) fn construction_progress(&self, station: SplitterStation) -> Option<f64> {
        self.construction_sites
            .iter()
            .find(|site| site.station_id == station.id)
            .map(|site| 1.0 - site.remaining_segments as f64 / station.design.cost() as f64)
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
            && self.exterior_port_cells().into_iter().all(|(col, row)| {
                col >= 0 && row >= 0 && (col as usize) < cols && (row as usize) < rows
            })
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
        station_width(self.design, self.rotation)
    }

    pub(super) fn height(self) -> usize {
        station_height(self.design, self.rotation)
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
        let (row, col) = self.design.cut_cell();
        self.world_cell(row, col)
    }

    fn juicer_feed_target(self) -> Cell {
        let (row, col) = JUICER_FEED[JUICER_FEED.len() - 1];
        self.world_cell(row, col)
    }

    fn service_cell(self) -> Cell {
        self.world_cell(0, 4)
    }

    fn service_approach_cell(self, cols: usize, rows: usize) -> Cell {
        self.service_cell()
            .stepped(self.old_exit_direction(), cols, rows)
    }

    pub(super) fn entry_cell(self) -> Cell {
        self.channel_cell(SplitterChannel::Entry, 0)
    }

    fn pre_entry_cell(self, cols: usize, rows: usize) -> Cell {
        self.entry_cell()
            .stepped(self.flow_direction().reversed(), cols, rows)
    }

    pub(super) fn new_exit_cell(self) -> Cell {
        self.channel_edge(SplitterChannel::NewSnek)
    }

    pub(super) fn old_exit_cell(self) -> Cell {
        self.channel_edge(SplitterChannel::OldSnek)
    }

    pub(super) fn legal_entry_step(self, from: Cell, to: Cell, cols: usize, rows: usize) -> bool {
        to == self.entry_cell()
            && from
                == self
                    .entry_cell()
                    .stepped(self.flow_direction().reversed(), cols, rows)
    }

    pub(super) fn flow_direction(self) -> Direction {
        self.channel_direction(SplitterChannel::Entry, true)
    }

    pub(super) fn new_exit_direction(self) -> Direction {
        self.channel_direction(SplitterChannel::NewSnek, false)
    }

    pub(super) fn old_exit_direction(self) -> Direction {
        self.channel_direction(SplitterChannel::OldSnek, false)
    }

    fn route_step(
        self,
        from: Cell,
        channel: SplitterChannel,
        cols: usize,
        rows: usize,
    ) -> Option<(Direction, Cell)> {
        let path = self.design.channel(channel);
        let index = path
            .iter()
            .position(|(row, col)| self.world_cell(*row, *col) == from)?;
        if let Some((row, col)) = path.get(index + 1) {
            let next = self.world_cell(*row, *col);
            let direction = direction_between(from, next)?;
            return Some((direction, next));
        }
        let direction = self.channel_direction(channel, false);
        Some((direction, from.stepped(direction, cols, rows)))
    }

    fn escape_step(self, from: Cell, cols: usize, rows: usize) -> Option<(Direction, Cell)> {
        for channel in [SplitterChannel::OldSnek, SplitterChannel::NewSnek] {
            if let Some(step) = self.route_step(from, channel, cols, rows) {
                return Some(step);
            }
        }

        let entry = self.design.channel(SplitterChannel::Entry);
        let index = entry
            .iter()
            .position(|(row, col)| self.world_cell(*row, *col) == from)?;
        if index == 0 {
            let direction = self.flow_direction().reversed();
            return Some((direction, from.stepped(direction, cols, rows)));
        }
        let (row, col) = entry[index - 1];
        let next = self.world_cell(row, col);
        Some((direction_between(from, next)?, next))
    }

    fn local_cell(self, cell: Cell) -> Option<(usize, usize)> {
        if !self.contains(cell) {
            return None;
        }
        Some((cell.row - self.origin.row, cell.col - self.origin.col))
    }

    fn world_cell(self, source_row: usize, source_col: usize) -> Cell {
        let (row, col) =
            rotated_station_position(source_row, source_col, self.design, self.rotation);
        Cell::new(self.origin.col + col, self.origin.row + row)
    }

    fn channel_cell(self, channel: SplitterChannel, index: usize) -> Cell {
        let (row, col) = self.design.channel(channel)[index];
        self.world_cell(row, col)
    }

    fn channel_edge(self, channel: SplitterChannel) -> Cell {
        let path = self.design.channel(channel);
        self.channel_cell(channel, path.len() - 1)
    }

    fn channel_direction(self, channel: SplitterChannel, from_start: bool) -> Direction {
        let path = self.design.channel(channel);
        let (from_index, to_index) = if from_start {
            (0, 1)
        } else {
            (path.len() - 2, path.len() - 1)
        };
        direction_between(
            self.channel_cell(channel, from_index),
            self.channel_cell(channel, to_index),
        )
        .expect("splitter channels must use adjacent cells")
    }

    fn exterior_port_cells(self) -> [(isize, isize); 3] {
        [
            offset_cell_signed(self.entry_cell(), self.flow_direction().reversed()),
            offset_cell_signed(self.new_exit_cell(), self.new_exit_direction()),
            offset_cell_signed(self.old_exit_cell(), self.old_exit_direction()),
        ]
    }

    fn occupies_signed(self, col: isize, row: isize) -> bool {
        col >= 0 && row >= 0 && self.occupies(Cell::new(col as usize, row as usize))
    }
}

fn direction_between(from: Cell, to: Cell) -> Option<Direction> {
    match (
        to.col as isize - from.col as isize,
        to.row as isize - from.row as isize,
    ) {
        (0, -1) => Some(Direction::Up),
        (0, 1) => Some(Direction::Down),
        (-1, 0) => Some(Direction::Left),
        (1, 0) => Some(Direction::Right),
        _ => None,
    }
}

fn offset_cell_signed(cell: Cell, direction: Direction) -> (isize, isize) {
    let (col, row) = (cell.col as isize, cell.row as isize);
    match direction {
        Direction::Up => (col, row - 1),
        Direction::Down => (col, row + 1),
        Direction::Left => (col - 1, row),
        Direction::Right => (col + 1, row),
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

fn juicer_feed_step(station: SplitterStation, from: Cell, cols: usize, rows: usize) -> Cell {
    if from == station.pre_entry_cell(cols, rows) {
        return station.entry_cell();
    }
    if let Some(index) = JUICER_FEED
        .iter()
        .position(|(row, col)| station.world_cell(*row, *col) == from)
    {
        return JUICER_FEED
            .get(index + 1)
            .map_or(from, |(row, col)| station.world_cell(*row, *col));
    }
    station
        .route_step(from, SplitterChannel::Entry, cols, rows)
        .map_or_else(
            || from.stepped(station.flow_direction(), cols, rows),
            |(_, next)| next,
        )
}

fn juicer_feed_on_approach(station: SplitterStation, cell: Cell, cols: usize, rows: usize) -> bool {
    let pre_entry = station.pre_entry_cell(cols, rows);
    match station.flow_direction() {
        Direction::Up => cell.col == pre_entry.col && cell.row >= pre_entry.row,
        Direction::Down => cell.col == pre_entry.col && cell.row <= pre_entry.row,
        Direction::Left => cell.row == pre_entry.row && cell.col >= pre_entry.col,
        Direction::Right => cell.row == pre_entry.row && cell.col <= pre_entry.col,
    }
}

fn stations_conflict(left: SplitterStation, right: SplitterStation) -> bool {
    let start_col = left.origin.col.max(right.origin.col);
    let end_col = (left.origin.col + left.width()).min(right.origin.col + right.width());
    let start_row = left.origin.row.max(right.origin.row);
    let end_row = (left.origin.row + left.height()).min(right.origin.row + right.height());
    let footprints_overlap = (start_row..end_row).any(|row| {
        (start_col..end_col).any(|col| {
            let cell = Cell::new(col, row);
            left.occupies(cell) && right.occupies(cell)
        })
    });
    if footprints_overlap {
        return true;
    }

    let left_ports = left.exterior_port_cells();
    let right_ports = right.exterior_port_cells();
    left_ports
        .iter()
        .any(|port| right_ports.contains(port) || right.occupies_signed(port.0, port.1))
        || right_ports
            .iter()
            .any(|port| left.occupies_signed(port.0, port.1))
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
            if station.design != StationDesign::Splitter || !stations.contains(&station) {
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

    fn is_valid_saved_positions(self, sneks: &[SavedSnek]) -> bool {
        let saved_snek = |id| sneks.iter().find(|snek| snek.id == id);
        match self.phase {
            SplitterJobPhase::Queued | SplitterJobPhase::Routing { .. } => true,
            SplitterJobPhase::Feeding {
                snek_id,
                station,
                advanced,
            } => saved_snek(snek_id)
                .and_then(|snek| snek.positions.last())
                .is_some_and(|head| {
                    *head == station.channel_cell(SplitterChannel::NewSnek, advanced as usize)
                }),
            SplitterJobPhase::Cutting {
                snek_id, station, ..
            } => {
                let expected: Vec<_> = (1..=MIN_SNEK_LEN)
                    .map(|index| station.channel_cell(SplitterChannel::NewSnek, index))
                    .collect();
                saved_snek(snek_id).is_some_and(|snek| snek.positions.ends_with(&expected))
            }
            SplitterJobPhase::Releasing {
                old_snek_id,
                new_snek_id,
                station,
            } => [old_snek_id, new_snek_id].into_iter().any(|id| {
                saved_snek(id).is_some_and(|snek| {
                    snek.positions
                        .iter()
                        .copied()
                        .any(|cell| station.occupies(cell))
                })
            }),
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
            if station.design != StationDesign::Splitter || !stations.contains(&station) {
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
        let mut channel_by_snek = vec![None; game.sneks.len()];
        let mut juicing_by_snek = vec![None; game.sneks.len()];
        let index_by_id: HashMap<_, _> = game
            .sneks
            .iter()
            .enumerate()
            .map(|(index, snek)| (snek.id, index))
            .collect();
        for job in &game.juicer_jobs {
            if let Some(index) = job
                .retained_snek_id
                .and_then(|id| index_by_id.get(&id).copied())
            {
                channel_by_snek[index] = Some((job.station, SplitterChannel::OldSnek));
            }
        }
        for job in &game.splitter_jobs {
            match job.phase {
                SplitterJobPhase::Queued | SplitterJobPhase::Cutting { .. } => {}
                SplitterJobPhase::Routing { snek_id, station } => {
                    if let Some(index) = index_by_id.get(&snek_id).copied() {
                        station_by_snek[index] = Some(station);
                        channel_by_snek[index] = Some((station, SplitterChannel::Entry));
                    }
                }
                SplitterJobPhase::Feeding {
                    snek_id, station, ..
                } => {
                    if let Some(index) = index_by_id.get(&snek_id).copied() {
                        channel_by_snek[index] = Some((station, SplitterChannel::NewSnek));
                    }
                }
                SplitterJobPhase::Releasing {
                    old_snek_id,
                    new_snek_id,
                    station,
                } => {
                    if let Some(index) = index_by_id.get(&old_snek_id).copied() {
                        channel_by_snek[index] = Some((station, SplitterChannel::OldSnek));
                    }
                    if let Some(index) = index_by_id.get(&new_snek_id).copied() {
                        channel_by_snek[index] = Some((station, SplitterChannel::NewSnek));
                    }
                }
            }
        }

        let available_juicers: Vec<_> = game
            .splitter_stations
            .iter()
            .copied()
            .filter(|station| station.design == StationDesign::Juicer)
            .filter(|station| game.station_complete(*station))
            .filter(|station| {
                game.bootstrap_phase == BootstrapPhase::Complete
                    || game.bootstrap_juicer_id != Some(station.id)
            })
            .filter(|station| {
                !game
                    .juicer_jobs
                    .iter()
                    .any(|job| job.station.id == station.id)
            })
            .collect();
        let construction_demand = game.construction_self_juice_needed();
        let purchase_demand = if construction_demand == 0 {
            game.purchase_juice_needed()
        } else {
            0
        };
        let task_mode = construction_demand > 0 || purchase_demand > 0;
        let desired_population = game
            .length_targets
            .iter()
            .map(|target| target.count)
            .sum::<usize>()
            .max(1);
        let excess_population = game.sneks.len().saturating_sub(desired_population);
        let growth_targets = game.growth_targets();
        let mut dispose_snek = vec![false; game.sneks.len()];
        let mut disposal_candidates: Vec<_> = game
            .sneks
            .iter()
            .enumerate()
            .filter(|(index, snek)| {
                station_by_snek[*index].is_none()
                    && channel_by_snek[*index].is_none()
                    && game.manual_snek != Some(snek.id)
                    && snek.carried_juice_segments == 0
            })
            .map(|(index, snek)| (snek.len, index))
            .collect();
        disposal_candidates.sort_unstable();
        let max_disposals = game.sneks.len().saturating_sub(1);
        let mut marked = 0usize;
        for (_, index) in disposal_candidates.iter().copied().filter(|(_, index)| {
            !task_mode
                && growth_targets[*index].is_some_and(|target| {
                    game.sneks[*index].len < target
                        && game.sneks[*index].growth_stall_steps >= GROWTH_STALL_STEPS
                })
        }) {
            dispose_snek[index] = true;
            marked += 1;
            if marked == max_disposals {
                break;
            }
        }
        for (_, index) in disposal_candidates {
            if task_mode {
                break;
            }
            if marked >= excess_population.min(max_disposals) {
                break;
            }
            if !dispose_snek[index] {
                dispose_snek[index] = true;
                marked += 1;
            }
        }
        let mut pairs = Vec::new();
        let mut construction_remaining = construction_demand;
        let mut purchase_remaining = purchase_demand;
        for (index, snek) in game.sneks.iter().enumerate() {
            let available = snek
                .available_tail_segments()
                .saturating_sub(game.reserved_segments_for_snek(snek.id));
            let task_candidate = (construction_remaining > 0 || purchase_remaining > 0)
                && available > 0
                && (!game.auto_snek_unlocked || game.manual_snek != Some(snek.id));
            let reached_max = !task_mode
                && game.manual_snek != Some(snek.id)
                && snek.excess_len() > 0
                && game.max_snek_len.is_some_and(|max| snek.len >= max);
            if station_by_snek[index].is_some()
                || channel_by_snek[index].is_some()
                || (!dispose_snek[index] && !task_candidate && !reached_max)
            {
                continue;
            }
            for (station_index, station) in available_juicers.iter().copied().enumerate() {
                pairs.push((
                    toroidal_distance(
                        snek.head(),
                        station.pre_entry_cell(game.cols, game.rows),
                        game.cols,
                        game.rows,
                    ),
                    index,
                    station_index,
                    station,
                ));
            }
        }
        pairs.sort_by_key(|(distance, snek_index, station_index, _)| {
            (*distance, *snek_index, *station_index)
        });
        let mut used_sneks = vec![false; game.sneks.len()];
        let mut used_juicers = vec![false; available_juicers.len()];
        for (_, snek_index, station_index, station) in pairs {
            if used_sneks[snek_index] || used_juicers[station_index] {
                continue;
            }
            let available = game.sneks[snek_index]
                .available_tail_segments()
                .saturating_sub(game.reserved_segments_for_snek(game.sneks[snek_index].id));
            let assignment = if construction_remaining > 0 && available > 0 {
                let segments = construction_remaining.min(available);
                construction_remaining -= segments;
                Some(JuicingAssignment {
                    purpose: JuicingPurpose::Construction,
                    segments,
                })
            } else if purchase_remaining > 0 && available > 0 {
                let segments = purchase_remaining.min(available);
                purchase_remaining -= segments;
                Some(JuicingAssignment {
                    purpose: JuicingPurpose::Purchase,
                    segments,
                })
            } else {
                None
            };
            let Some(assignment) = assignment else {
                if !dispose_snek[snek_index] {
                    let snek = &game.sneks[snek_index];
                    let reached_max = !task_mode
                        && game.manual_snek != Some(snek.id)
                        && snek.excess_len() > 0
                        && game.max_snek_len.is_some_and(|max| snek.len >= max);
                    if !reached_max {
                        continue;
                    }
                }
                used_sneks[snek_index] = true;
                used_juicers[station_index] = true;
                station_by_snek[snek_index] = Some(station);
                channel_by_snek[snek_index] = Some((station, SplitterChannel::Entry));
                continue;
            };
            used_sneks[snek_index] = true;
            used_juicers[station_index] = true;
            station_by_snek[snek_index] = Some(station);
            channel_by_snek[snek_index] = Some((station, SplitterChannel::Entry));
            juicing_by_snek[snek_index] = Some(assignment);
        }

        Self {
            station_by_snek,
            channel_by_snek,
            juicing_by_snek,
        }
    }

    fn station_for_snek(&self, index: usize) -> Option<SplitterStation> {
        self.station_by_snek.get(index).copied().flatten()
    }

    fn channel_for_snek(&self, index: usize) -> Option<(SplitterStation, SplitterChannel)> {
        self.channel_by_snek.get(index).copied().flatten()
    }

    fn juicing_assignment(&self, index: usize) -> Option<JuicingAssignment> {
        self.juicing_by_snek.get(index).copied().flatten()
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
            growth_stall_steps: 0,
            carried_juice_segments: 0,
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
            growth_stall_steps: 0,
            carried_juice_segments: 0,
        }
    }

    fn from_save(save: SavedSnek) -> Self {
        Self {
            id: save.id,
            direction: save.direction,
            positions: save.positions.into(),
            len: save.len,
            exit_override: None,
            growth_stall_steps: 0,
            carried_juice_segments: save.carried_juice_segments,
        }
    }

    fn save(&self) -> SavedSnek {
        SavedSnek {
            id: self.id,
            direction: self.direction,
            positions: self.positions.iter().copied().collect(),
            len: self.len,
            carried_juice_segments: self.carried_juice_segments,
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

    pub(super) fn carried_juice_segments(&self) -> usize {
        self.carried_juice_segments
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

    fn available_tail_segments(&self) -> usize {
        self.len
            .saturating_sub(MIN_SNEK_LEN.max(self.carried_juice_segments))
    }

    #[cfg(test)]
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
        let new_cargo = self.carried_juice_segments.min(MIN_SNEK_LEN);
        self.carried_juice_segments -= new_cargo;
        Some(Self {
            id: new_id,
            direction,
            positions: new_positions,
            len: MIN_SNEK_LEN,
            exit_override: Some(direction),
            growth_stall_steps: 0,
            carried_juice_segments: new_cargo,
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
    if row >= station_height(design, rotation) || col >= station_width(design, rotation) {
        return None;
    }
    let (source_row, source_col) = rotated_station_index(row, col, design, rotation);
    station_pattern(design)
        .get(source_row)
        .and_then(|line| line.as_bytes().get(source_col))
        .map(|byte| *byte as char)
}

fn rotated_station_index(
    row: usize,
    col: usize,
    design: StationDesign,
    rotation: u8,
) -> (usize, usize) {
    let source_height = design.height();
    let source_width = design.width();
    match rotation % 4 {
        1 => (source_height - 1 - col, row),
        2 => (source_height - 1 - row, source_width - 1 - col),
        3 => (col, source_width - 1 - row),
        _ => (row, col),
    }
}

fn rotated_station_position(
    source_row: usize,
    source_col: usize,
    design: StationDesign,
    rotation: u8,
) -> (usize, usize) {
    let source_height = design.height();
    let source_width = design.width();
    match rotation % 4 {
        1 => (source_col, source_height - 1 - source_row),
        2 => (
            source_height - 1 - source_row,
            source_width - 1 - source_col,
        ),
        3 => (source_width - 1 - source_col, source_row),
        _ => (source_row, source_col),
    }
}

fn station_pattern(design: StationDesign) -> &'static [&'static str] {
    match design {
        StationDesign::Splitter => &[
            "# #0G0# #",
            "# #0B0# #",
            "# #0A0# #",
            "###000###",
            "  ##0##  ",
            "   #0#   ",
            "   #0#   ",
            "   #0#   ",
            "   #0#   ",
        ],
        StationDesign::Juicer => &[
            "# #0G####",
            "# #0B#SS#",
            "# #0A0SD#",
            "###000###",
            "  ##0##  ",
            "   #0#   ",
            "   #0#   ",
            "   #0#   ",
            "   #0#   ",
        ],
    }
}

fn station_width(design: StationDesign, rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        design.width()
    } else {
        design.height()
    }
}

fn station_height(design: StationDesign, rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        design.height()
    } else {
        design.width()
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
                carried_juice_segments: 0,
            }],
            apples: vec![Cell::new(12, 12)],
            apple_capacity: 1,
            juice_segments: 0,
            juice_discovered: false,
            lifetime_apls: 0,
            juicer_unlocked: false,
            juicer_placement_unlocked: false,
            bootstrap_phase: BootstrapPhase::Locked,
            bootstrap_juicer_id: None,
            bootstrap_free_juicer_id: None,
            bootstrap_cost_segments: VecDeque::new(),
            auto_snek_unlocked: false,
            auto_snek_offered: false,
            splitter_unlocked: false,
            population_unlocked: false,
            splitter_stations: Vec::new(),
            construction_sites: Vec::new(),
            pending_purchases: VecDeque::new(),
            splitter_jobs: Vec::new(),
            juicer_jobs: Vec::new(),
            max_snek_len: None,
            length_targets: vec![LengthTarget {
                length: MIN_SNEK_LEN,
                count: 1,
            }],
        };
        assert!(SnekGame::from_save(save, 5, 5).is_none());
    }

    #[test]
    fn saved_game_preserves_grown_world_dimensions() {
        let station =
            SplitterStation::new(Cell::new(45, 32), 0, StationDesign::Splitter, 60, 45).with_id(1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 60, 45).with_id(2);
        let save = SavedSnekGame {
            version: SNEK_SAVE_VERSION,
            cols: 60,
            rows: 45,
            sneks: vec![SavedSnek {
                id: 1,
                direction: Direction::Left,
                positions: vec![Cell::new(50, 30), Cell::new(51, 30), Cell::new(52, 30)],
                len: MIN_SNEK_LEN,
                carried_juice_segments: 0,
            }],
            apples: vec![Cell::new(55, 40)],
            apple_capacity: 1,
            juice_segments: 0,
            juice_discovered: false,
            lifetime_apls: 0,
            juicer_unlocked: true,
            juicer_placement_unlocked: true,
            bootstrap_phase: BootstrapPhase::Complete,
            bootstrap_juicer_id: Some(2),
            bootstrap_free_juicer_id: Some(2),
            bootstrap_cost_segments: VecDeque::new(),
            auto_snek_unlocked: true,
            auto_snek_offered: true,
            splitter_unlocked: true,
            population_unlocked: true,
            splitter_stations: vec![juicer, station],
            construction_sites: Vec::new(),
            pending_purchases: VecDeque::new(),
            splitter_jobs: Vec::new(),
            juicer_jobs: Vec::new(),
            max_snek_len: Some(13),
            length_targets: vec![LengthTarget {
                length: MIN_SNEK_LEN,
                count: 1,
            }],
        };

        let game = SnekGame::from_save(save, 8, 8).unwrap();

        assert_eq!(game.cols(), 60);
        assert_eq!(game.rows(), 45);
        assert_eq!(
            game.sneks[0].positions(),
            &[Cell::new(50, 30), Cell::new(51, 30), Cell::new(52, 30)]
        );
        assert_eq!(game.apples(), &[Cell::new(55, 40)]);
        assert_eq!(game.splitter_stations(), &[juicer, station]);
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
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30).with_id(1);
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
                carried_juice_segments: 0,
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
        game.apple_capacity = 2;

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
        game.sneks[0].len = MIN_SNEK_LEN + JUICER_UNLOCK_COST;
        game.sneks[0].positions = vec![Cell::new(4, 4); MIN_SNEK_LEN + JUICER_UNLOCK_COST].into();
        game.juicer_unlocked = true;
        game.population_unlocked = true;
        game.juice_segments = AUTO_SNEK_COST * SEGMENTS_PER_JUICE;

        assert!(game.buy_auto_snek());
        game.step(&mut Lcg::new(2));
        assert!(game.auto_snek_unlocked());
        assert_eq!(game.apls(), JUICER_UNLOCK_COST);
        assert_eq!(
            game.sneks[0].positions().len(),
            MIN_SNEK_LEN + JUICER_UNLOCK_COST
        );
    }

    #[test]
    fn bootstrap_juicer_spends_exact_cost_without_processing_the_snek() {
        let mut game = SnekGame::new(30, 30, 1);
        assert!(!game.unlock_queue_available());
        game.sneks[0].add_segments(JUICER_UNLOCK_COST + 5);
        let head = game.sneks[0].head();

        assert!(game.unlock_queue_available());
        assert!(game.unlock_juicer());

        assert!(!game.juicer_unlocked());
        assert_eq!(game.apls(), JUICER_UNLOCK_COST + 5);
        assert_eq!(game.sneks[0].head(), head);
        assert!(game.juicer_jobs.is_empty());
        assert!(!game.juicer_placement_unlocked());
        assert_eq!(game.juice(), 0);
        assert!(!game.auto_snek_offer_visible());
        assert!(!game.can_buy_auto_snek());
        assert!(game.splitter_stations.is_empty());
        assert_eq!(game.juice(), 0);
        assert!(SnekGame::from_save(game.save(), 30, 30).is_some());

        let mut saw_walking = false;
        let mut saw_flash = false;
        game.apples = vec![Cell::new(25, 25)];
        for _ in 0..300 {
            game.step(&mut Lcg::new(4));
            if !saw_walking && matches!(game.bootstrap_phase, BootstrapPhase::Walking { .. }) {
                saw_walking = true;
                assert_eq!(game.bootstrap_cost_segments.len(), JUICER_UNLOCK_COST);
                assert!(matches!(
                    game.bootstrap_activity().unwrap().flash_origin.col,
                    0 | 29
                ));
                assert!(SnekGame::from_save(game.save(), 30, 30).is_some());
            }
            saw_flash |= matches!(game.bootstrap_phase, BootstrapPhase::Flash { .. });
            if game.bootstrap_phase == BootstrapPhase::Complete {
                break;
            }
        }
        assert_eq!(game.bootstrap_phase, BootstrapPhase::Complete);
        assert!(saw_walking);
        assert!(saw_flash);
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN + 5);
        assert_eq!(game.juice_segments, 0);
        assert!(game.juicer_jobs.is_empty());
        assert!(game.juicer_unlocked());
        assert!(game.auto_snek_offer_visible());
        assert!(game.bootstrap_cost_segments.is_empty());
        assert_eq!(game.splitter_stations.len(), 1);
        assert_eq!(game.splitter_stations[0].origin(), Cell::new(5, 16));
        assert!(!game.delete_splitter_station(0));
    }

    #[test]
    fn bootstrap_cut_center_survives_resize_and_unpayable_save_is_rejected() {
        let mut game = SnekGame::new(30, 30, 1);
        game.sneks[0].add_segments(JUICER_UNLOCK_COST);
        assert!(game.unlock_juicer());

        let mut invalid = game.save();
        invalid.sneks[0].len = MIN_SNEK_LEN;
        invalid.sneks[0].positions.truncate(MIN_SNEK_LEN);
        assert!(SnekGame::from_save(invalid, 30, 30).is_none());

        for _ in 0..20 {
            game.step(&mut Lcg::new(5));
            if matches!(game.bootstrap_phase, BootstrapPhase::Cutting { .. }) {
                break;
            }
        }
        let center = game.bootstrap_activity().unwrap().center;
        assert_eq!(game.sneks[0].head(), center);
        game.resize(50, 40);
        assert_eq!(game.bootstrap_activity().unwrap().center, center);
        assert_eq!(game.sneks[0].head(), center);

        for _ in 0..20 {
            game.step(&mut Lcg::new(6));
            if matches!(game.bootstrap_phase, BootstrapPhase::Walking { .. }) {
                break;
            }
        }
        let direction = game.bootstrap_activity().unwrap().direction;
        game.resize(60, 45);
        let expected_col = if direction == Direction::Left {
            0
        } else {
            game.cols - 1
        };
        assert_eq!(
            game.bootstrap_activity().unwrap().flash_origin.col,
            expected_col
        );
    }

    #[test]
    fn auto_snek_purchase_self_juices_without_prior_autopilot() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE);
        game.sneks[0].positions = vec![Cell::new(20, 20); game.sneks[0].len].into();
        game.apples = vec![Cell::new(25, 25)];

        assert!(game.can_buy_auto_snek());
        assert!(game.buy_auto_snek());
        assert!(!game.auto_snek_unlocked());
        for _ in 0..300 {
            game.step(&mut Lcg::new(5));
            if game.auto_snek_unlocked() {
                break;
            }
        }

        assert!(game.auto_snek_unlocked());
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert_eq!(game.juice_segments, 0);
        assert!(game.pending_purchases.is_empty());
    }

    #[test]
    fn partial_purchase_trim_preserves_head_side_cargo() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(18, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 5,
        }];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE);
        game.sneks[0].carried_juice_segments = 5;
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
        game.sneks[0].direction = juicer.flow_direction();
        game.juice_segments = SEGMENTS_PER_JUICE - 2;

        assert!(game.buy_auto_snek());
        for _ in 0..30 {
            game.step(&mut Lcg::new(6));
            if !game.juicer_jobs.is_empty() {
                break;
            }
        }

        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN + SEGMENTS_PER_JUICE - 2);
        assert_eq!(game.sneks[0].carried_juice_segments, 5);
        assert_eq!(game.juicer_jobs[0].feed_segments.len(), 2);
        assert!(SnekGame::from_save(game.save(), 30, 30).is_some());
    }

    #[test]
    fn queued_splitter_order_does_not_block_exact_upgrade_juicing() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let splitter =
            SplitterStation::new(Cell::new(19, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_unlocked = true;
        game.splitter_stations = vec![juicer, splitter];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE + MIN_SNEK_LEN);
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
        game.sneks[0].direction = juicer.flow_direction();
        game.splitter_jobs = vec![SplitterJob::queued(1)];

        assert!(game.buy_auto_snek());
        for _ in 0..100 {
            game.step(&mut Lcg::new(8));
            if game.auto_snek_unlocked() {
                break;
            }
        }

        assert!(game.auto_snek_unlocked());
        assert_eq!(game.sneks.len(), 1);
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN * 2);
        assert!(matches!(
            game.splitter_jobs[0].phase,
            SplitterJobPhase::Queued | SplitterJobPhase::Routing { .. }
        ));
    }

    #[test]
    fn construction_delivery_has_priority_over_queued_splitter_order() {
        let mut game = SnekGame::new(40, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 30).with_id(1);
        let splitter =
            SplitterStation::new(Cell::new(17, 5), 0, StationDesign::Splitter, 40, 30).with_id(2);
        let ghost =
            SplitterStation::new(Cell::new(28, 15), 0, StationDesign::Splitter, 40, 30).with_id(3);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer, splitter, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 3,
            remaining_segments: 1,
        }];
        game.sneks[0].carried_juice_segments = 1;
        game.splitter_jobs = vec![SplitterJob::queued(1)];

        game.assign_splitter_jobs();
        assert_eq!(game.splitter_jobs[0].phase, SplitterJobPhase::Queued);
        assert!(game.logistics_assignments()[0].is_some());
    }

    #[test]
    fn construction_self_juices_before_pending_upgrade() {
        let mut game = SnekGame::new(40, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(26, 14), 0, StationDesign::Splitter, 40, 30).with_id(2);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 5,
        }];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE * 6);
        game.apples = vec![Cell::new(38, 28)];

        assert!(game.buy_auto_snek());
        let dispatch = game.splitter_dispatch();
        assert_eq!(
            dispatch.juicing_assignment(0),
            Some(JuicingAssignment {
                purpose: JuicingPurpose::Construction,
                segments: SEGMENTS_PER_JUICE * 5,
            })
        );
        for _ in 0..1_000 {
            game.step(&mut Lcg::new(10));
            if game.construction_sites.is_empty() {
                break;
            }
        }

        assert!(
            game.construction_sites.is_empty(),
            "sites={:?} juice={} jobs={:?} snek={:?}",
            game.construction_sites,
            game.juice_segments,
            game.juicer_jobs,
            game.sneks[0]
        );
        assert_eq!(game.sneks.len(), 1);
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN + SEGMENTS_PER_JUICE);
        assert!(!game.auto_snek_unlocked());
        assert!(!game.pending_purchases.is_empty());
    }

    #[test]
    fn incomplete_splitter_ghost_cannot_release_a_snek() {
        let mut game = SnekGame::new(40, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(26, 14), 0, StationDesign::Splitter, 40, 30).with_id(2);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.auto_snek_unlocked = true;
        game.splitter_unlocked = true;
        game.population_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 10,
        }];
        game.sneks[0].add_segments(MIN_SNEK_LEN);
        game.length_targets.push(LengthTarget {
            length: MIN_SNEK_LEN,
            count: 2,
        });

        for _ in 0..100 {
            game.step(&mut Lcg::new(11));
        }

        assert_eq!(game.sneks.len(), 1);
        assert!(game
            .splitter_jobs
            .iter()
            .all(|job| job.phase == SplitterJobPhase::Queued));
    }

    #[test]
    fn underfunded_construction_overrides_population_growth_cap() {
        let mut game = SnekGame::new(20, 20, 1);
        let ghost =
            SplitterStation::new(Cell::new(10, 8), 0, StationDesign::Splitter, 20, 20).with_id(1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.max_snek_len = Some(MIN_SNEK_LEN);
        game.splitter_stations = vec![ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 1,
            remaining_segments: 1,
        }];
        game.sneks[0].positions = vec![Cell::new(4, 4); MIN_SNEK_LEN].into();
        game.sneks[0].direction = Direction::Right;
        game.apples = vec![Cell::new(5, 4)];

        game.step(&mut Lcg::new(12));

        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN + 1);
        assert_eq!(game.apls_produced, 1);
    }

    #[test]
    fn carried_construction_packet_precedes_purchase_juicing() {
        let mut game = SnekGame::new(40, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(26, 14), 0, StationDesign::Splitter, 40, 30).with_id(2);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.manual_snek = None;
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 1,
        }];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE);
        game.sneks[0].carried_juice_segments = 1;
        let (approach, target) = game
            .station_delivery_contact(ghost, game.sneks[0].head())
            .unwrap();
        game.sneks[0].positions = vec![approach; game.sneks[0].len].into();
        game.sneks[0].direction = direction_between(approach, target).unwrap();

        assert!(game.buy_auto_snek());
        game.step(&mut Lcg::new(9));

        assert!(game.construction_sites.is_empty());
        assert!(game.juicer_jobs.is_empty());
        assert!(!game.pending_purchases.is_empty());
    }

    #[test]
    fn manually_selected_auto_snek_is_not_claimed_for_construction() {
        let mut game = SnekGame::new(30, 30, 1);
        let ghost =
            SplitterStation::new(Cell::new(15, 12), 0, StationDesign::Splitter, 30, 30).with_id(1);
        game.auto_snek_unlocked = true;
        game.manual_snek = Some(1);
        game.splitter_stations = vec![ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 1,
            remaining_segments: 1,
        }];
        game.sneks[0].carried_juice_segments = 1;

        assert!(game.logistics_assignments()[0].is_none());
    }

    #[test]
    fn manually_selected_auto_snek_is_not_counted_as_upgrade_funding() {
        let mut game = SnekGame::new(30, 30, 1);
        game.juicer_unlocked = true;
        game.auto_snek_unlocked = true;
        game.manual_snek = Some(1);
        game.lifetime_apls = 50;
        game.sneks[0].add_segments(SPLITTER_STATION_UNLOCK_COST * SEGMENTS_PER_JUICE);

        assert!(!game.can_unlock_splitter_station());
        game.manual_snek = None;
        assert!(game.can_unlock_splitter_station());
    }

    #[test]
    fn tapping_snek_over_a_station_selects_manual_control() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let cell = station.cut_target();
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![station];
        game.sneks[0].positions = vec![cell; MIN_SNEK_LEN].into();

        assert!(game.tap_cell(cell));
        assert_eq!(game.manual_snek(), Some(1));
    }

    #[test]
    fn construction_uses_banked_juice_before_queued_purchase() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(18, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.juicer_unlocked = true;
        game.auto_snek_offered = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 1,
        }];
        game.juice_segments = SEGMENTS_PER_JUICE * 2;
        game.sneks[0].positions =
            vec![juicer.service_approach_cell(game.cols, game.rows); MIN_SNEK_LEN].into();

        assert!(game.buy_auto_snek());
        assert_eq!(game.construction_juice_available(), SEGMENTS_PER_JUICE * 2);
        assert!(game.handle_service_bump(0, juicer.service_cell()));
        assert!(!game.auto_snek_unlocked());
        assert_eq!(game.juice_segments, SEGMENTS_PER_JUICE);
        assert_eq!(game.sneks[0].carried_juice_segments, 1);
        assert!(!game.pending_purchases.is_empty());
    }

    #[test]
    fn juicer_placement_requires_its_own_fifty_juice_unlock() {
        let mut game = SnekGame::new(20, 20, 1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.lifetime_apls = 250;
        game.juice_segments = JUICER_PLACEMENT_UNLOCK_COST * SEGMENTS_PER_JUICE;

        assert!(!game.can_place_station(StationDesign::Juicer));
        assert!(game.buy_juicer_placement_unlock());
        game.step(&mut Lcg::new(2));
        assert!(game.can_place_station(StationDesign::Juicer));
        assert_eq!(game.juice(), 0);
    }

    #[test]
    fn unlock_offers_follow_production_and_population_milestones() {
        let mut game = SnekGame::new(20, 20, 1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;

        game.lifetime_apls = 49;
        assert!(!game.splitter_unlock_visible());
        game.lifetime_apls = 50;
        assert!(game.splitter_unlock_visible());
        game.lifetime_apls = 249;
        assert!(!game.apple_unlock_visible());
        assert!(!game.juicer_upgrade_visible());
        game.lifetime_apls = 250;
        assert!(game.apple_unlock_visible());
        assert!(game.juicer_upgrade_visible());

        assert!(game.auto_snek_offer_visible());
        assert!(!game.can_buy_auto_snek());
        game.juice_segments = SEGMENTS_PER_JUICE;
        assert!(game.auto_snek_offer_visible());
        assert!(game.can_buy_auto_snek());
        game.refresh_unlock_offers();
        assert!(game.spend_juice(AUTO_SNEK_COST));
        assert!(game.auto_snek_offer_visible());
    }

    #[test]
    fn autopilot_moves_toward_nearest_apple() {
        let mut game = SnekGame::new(10, 10, 1);
        game.sneks[0].len = MIN_SNEK_LEN + JUICER_UNLOCK_COST;
        game.sneks[0].positions = vec![Cell::new(2, 2); MIN_SNEK_LEN + JUICER_UNLOCK_COST].into();
        game.juicer_unlocked = true;
        game.population_unlocked = true;
        game.juice_segments = AUTO_SNEK_COST * SEGMENTS_PER_JUICE;
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
            StationDesign::Splitter,
            20,
            20,
        )];
        game.sneks[0].positions = vec![Cell::new(7, 4); MIN_SNEK_LEN].into();
        game.set_direction(Direction::Down);

        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].head(), Cell::new(7, 3));
    }

    #[test]
    fn splitter_station_purchase_places_funding_ghost() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.juice_segments = SPLITTER_STATION_COST * SEGMENTS_PER_JUICE;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(4, 4); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(6, 7),
            2,
            StationDesign::Splitter,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(station.origin(), Cell::new(6, 7));
        assert_eq!(station.rotation(), 2);
        assert_eq!(station.design(), StationDesign::Splitter);
        assert_eq!(game.juice(), SPLITTER_STATION_COST);
        assert_eq!(game.construction_progress(station), Some(0.0));
    }

    #[test]
    fn juicer_station_ghost_costs_twenty_juice() {
        let mut game = SnekGame::new(24, 24, 1);
        game.juicer_unlocked = true;
        game.juicer_placement_unlocked = true;

        assert!(game.buy_splitter_station(
            Cell::new(7, 8),
            0,
            StationDesign::Juicer,
            &mut Lcg::new(2),
        ));

        assert_eq!(
            game.construction_sites[0].remaining_segments,
            JUICER_STATION_COST
        );
        assert_eq!(
            game.construction_progress(game.splitter_stations[0]),
            Some(0.0)
        );
    }

    #[test]
    fn juicer_feed_turns_through_path_into_blender() {
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let cut = station.cut_target();
        let path_one = juicer_feed_step(station, cut, 30, 30);
        let path_two = juicer_feed_step(station, path_one, 30, 30);
        let chamber = juicer_feed_step(station, path_two, 30, 30);

        assert_eq!(path_one, station.world_cell(3, 5));
        assert_eq!(path_two, station.world_cell(2, 5));
        assert_eq!(chamber, station.juicer_feed_target());
    }

    #[test]
    fn detached_juicer_tail_advances_two_path_cells_per_tick() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_jobs = vec![JuicerJob {
            station,
            source_snek_id: 1,
            retained_snek_id: Some(1),
            feed_segments: vec![Cell::new(2, 2)].into(),
            approach_path: vec![
                Cell::new(3, 2),
                Cell::new(4, 2),
                Cell::new(5, 2),
                station.cut_target(),
            ]
            .into(),
            gradient_len: 1,
            purpose: Some(JuicingPurpose::Construction),
        }];

        game.advance_juicer_jobs();

        assert_eq!(game.juicer_jobs[0].approach_path.len(), 2);
        assert_eq!(
            game.juicer_jobs[0].feed_segments.back(),
            Some(&Cell::new(4, 2))
        );
    }

    #[test]
    fn juicer_feed_renders_in_chamber_before_conversion() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_jobs = vec![JuicerJob {
            station,
            source_snek_id: 1,
            retained_snek_id: Some(1),
            feed_segments: vec![station.world_cell(2, 5)].into(),
            approach_path: VecDeque::new(),
            gradient_len: 1,
            purpose: Some(JuicingPurpose::Construction),
        }];

        game.advance_juicer_jobs();
        assert_eq!(
            game.juicer_jobs[0].feed_segments.back(),
            Some(&station.juicer_feed_target())
        );
        assert_eq!(game.juice_segments, 0);

        game.advance_juicer_jobs();
        assert!(game.juicer_jobs.is_empty());
        assert_eq!(game.juice_segments, 1);
    }

    #[test]
    fn splitter_station_purchase_allows_snek_overlap() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.juice_segments = SPLITTER_STATION_COST * SEGMENTS_PER_JUICE;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(6, 7); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(5, 5),
            0,
            StationDesign::Splitter,
            &mut Lcg::new(2)
        ));

        assert!(!game.splitter_stations.is_empty());
        assert_eq!(game.juice(), SPLITTER_STATION_COST);
    }

    #[test]
    fn splitter_station_purchase_replaces_covered_apples() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.juice_segments = SPLITTER_STATION_COST * SEGMENTS_PER_JUICE;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(15, 15); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();
        game.apples = vec![Cell::new(6, 7)];

        assert!(game.buy_splitter_station(
            Cell::new(5, 5),
            0,
            StationDesign::Splitter,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(game.apples.len(), 1);
        assert!(!station.occupies(game.apples[0]));
    }

    #[test]
    fn splitter_station_move_allows_snek_overlap_and_replaces_covered_apples() {
        let mut game = SnekGame::new(20, 20, 1);
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(1, 1),
            0,
            StationDesign::Splitter,
            20,
            20,
        )];
        game.sneks[0].positions = vec![Cell::new(10, 10); MIN_SNEK_LEN].into();

        game.apples = vec![Cell::new(6, 6)];
        assert!(game.move_splitter_station(0, Cell::new(5, 5), 0, &mut Lcg::new(3)));
        let station = game.splitter_stations.first().copied().unwrap();
        assert_eq!(game.apples.len(), 1);
        assert!(!station.occupies(game.apples[0]));
    }

    #[test]
    fn snek_inside_splitter_wall_can_route_out() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(5, 5),
            0,
            StationDesign::Splitter,
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
        game.juice_segments = SPLITTER_STATION_COST * SEGMENTS_PER_JUICE;
        game.sneks[0].len = MIN_SNEK_LEN + SPLITTER_STATION_COST;
        game.sneks[0].positions =
            vec![Cell::new(10, 10); MIN_SNEK_LEN + SPLITTER_STATION_COST].into();

        assert!(game.buy_splitter_station(
            Cell::new(18, 18),
            0,
            StationDesign::Splitter,
            &mut Lcg::new(2)
        ));

        let station = game.splitter_stations.first().copied().unwrap();
        assert!(game.cols() >= station.origin().col + station.width() + BUILDING_BUFFER);
        assert!(game.rows() >= station.origin().row + station.height() + BUILDING_BUFFER);
    }

    #[test]
    fn buying_snek_queues_order_and_spends_when_splitter_is_entered() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(8, 8),
            0,
            StationDesign::Splitter,
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
            SplitterStation::new(Cell::new(10, 10), 0, StationDesign::Splitter, 30, 30).with_id(1);
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
    fn splitter_dispatch_uses_closest_unassigned_snek_splitter_pairs() {
        let mut game = SnekGame::new(40, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![
            SplitterStation::new(Cell::new(4, 4), 0, StationDesign::Splitter, 40, 20).with_id(1),
            SplitterStation::new(Cell::new(28, 4), 0, StationDesign::Splitter, 40, 20).with_id(2),
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
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 40, 20).with_id(1);
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob::queued(2)];
        game.sneks = vec![
            Snek::new_with_direction(
                1,
                station
                    .entry_cell()
                    .stepped(station.flow_direction().reversed(), 40, 20),
                station.flow_direction(),
                MIN_SNEK_LEN * 2,
            ),
            Snek::new_with_direction(
                2,
                station.entry_cell(),
                station.flow_direction(),
                MIN_SNEK_LEN * 2,
            ),
        ];

        game.assign_splitter_jobs();
        let dispatch = game.splitter_dispatch();

        assert_eq!(dispatch.station_for_snek(0), None);
        assert_eq!(dispatch.station_for_snek(1), Some(station));
    }

    #[test]
    fn splitter_design_patterns_have_declared_semantic_geometry() {
        let expected_dimensions = [(StationDesign::Splitter, 9, 9)];

        for (design, width, height) in expected_dimensions {
            let pattern = station_pattern(design);
            assert_eq!(design.width(), width, "{design:?} width");
            assert_eq!(design.height(), height, "{design:?} height");
            assert_eq!(pattern.len(), height, "{design:?} row count");
            assert!(
                pattern.iter().all(|row| row.len() == width),
                "{design:?} has an incorrectly sized row"
            );
            assert!(
                pattern
                    .iter()
                    .flat_map(|row| row.bytes())
                    .all(|tile| b"0#GBA ".contains(&tile)),
                "{design:?} contains a non-semantic tile"
            );

            let cut = design.cut_cell();
            for channel in [
                SplitterChannel::Entry,
                SplitterChannel::NewSnek,
                SplitterChannel::OldSnek,
            ] {
                let path = design.channel(channel);
                assert!(path.len() >= 2, "{design:?} {channel:?} is too short");
                assert!(
                    path.iter()
                        .all(|(row, col)| pattern[*row].as_bytes()[*col] == b'0'),
                    "{design:?} {channel:?} is not represented by its process lane"
                );
                assert!(
                    path.windows(2).all(|cells| {
                        cells[0].0.abs_diff(cells[1].0) + cells[0].1.abs_diff(cells[1].1) == 1
                    }),
                    "{design:?} {channel:?} contains a disconnected step"
                );
            }
            for (row, line) in pattern.iter().enumerate() {
                for (col, tile) in line.bytes().enumerate() {
                    if tile != b'0' {
                        continue;
                    }
                    assert!(
                        [
                            SplitterChannel::Entry,
                            SplitterChannel::NewSnek,
                            SplitterChannel::OldSnek,
                        ]
                        .into_iter()
                        .any(|channel| design.channel(channel).contains(&(row, col))),
                        "{design:?} has a decorative process cell at ({row}, {col})"
                    );
                }
            }
            assert_eq!(design.channel(SplitterChannel::Entry).last(), Some(&cut));
            assert_eq!(design.channel(SplitterChannel::NewSnek).first(), Some(&cut));
            assert_eq!(design.channel(SplitterChannel::OldSnek).first(), Some(&cut));

            for channel in [
                SplitterChannel::Entry,
                SplitterChannel::NewSnek,
                SplitterChannel::OldSnek,
            ] {
                let edge = if channel == SplitterChannel::Entry {
                    design.channel(channel)[0]
                } else {
                    *design.channel(channel).last().unwrap()
                };
                assert!(
                    edge.0 == 0 || edge.1 == 0 || edge.0 + 1 == height || edge.1 + 1 == width,
                    "{design:?} {channel:?} does not reach an edge"
                );
            }
        }
    }

    #[test]
    fn juicer_pattern_has_a_walled_hall_left_exit_and_slurry_chamber() {
        let pattern = station_pattern(StationDesign::Juicer);

        assert_eq!(pattern.len(), StationDesign::Juicer.height());
        assert!(pattern
            .iter()
            .all(|row| row.len() == StationDesign::Juicer.width()));
        assert!(pattern.iter().any(|row| row.contains('S')));
        assert!(pattern.iter().any(|row| row.contains('D')));
        assert_eq!(
            StationDesign::Juicer.channel(SplitterChannel::OldSnek),
            SPLITTER_OLD
        );
        assert_eq!(pattern[0].as_bytes()[5], b'#');
        assert_eq!(pattern[1].as_bytes()[5], b'#');
        assert_eq!(pattern[2].as_bytes()[5], b'0');
        assert_eq!(&pattern[5..9], &["   #0#   "; 4]);
    }

    #[test]
    fn splitter_design_geometry_rotates_with_each_footprint() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let station = SplitterStation::new(Cell::new(8, 8), rotation, design, 40, 40);
                let (expected_width, expected_height) = if rotation.is_multiple_of(2) {
                    (design.width(), design.height())
                } else {
                    (design.height(), design.width())
                };

                assert_eq!(station.width(), expected_width);
                assert_eq!(station.height(), expected_height);
                assert!(station.cuts(station.cut_target()));
                assert!(station.cuts(station.entry_cell()));
                assert!(station.cuts(station.new_exit_cell()));
                assert!(station.cuts(station.old_exit_cell()));
                assert_eq!(station_cell(design, expected_height, 0, rotation), None);
                assert_eq!(station_cell(design, 0, expected_width, rotation), None);
            }
        }
    }

    #[test]
    fn splitter_ports_are_reserved_from_neighboring_stations() {
        let left = SplitterStation::new(Cell::new(10, 10), 0, StationDesign::Splitter, 40, 40);
        let blocking = SplitterStation::new(Cell::new(11, 2), 0, StationDesign::Splitter, 40, 40);
        let clear = SplitterStation::new(Cell::new(22, 2), 0, StationDesign::Splitter, 40, 40);

        assert!(stations_conflict(left, blocking));
        assert!(!stations_conflict(left, clear));
    }

    #[test]
    fn legacy_splitter_design_names_are_not_deserialized() {
        for legacy in [
            "Gate",
            "Saw",
            "Press",
            "Butterfly",
            "Mantis",
            "CrookedForge",
            "SalvagePunch",
            "CounterweightWorks",
            "WedgeMill",
            "YJunction",
        ] {
            let json = format!("\"{legacy}\"");
            assert!(serde_json::from_str::<StationDesign>(&json).is_err());
        }
    }

    #[test]
    fn splitter_channels_follow_their_declared_paths_after_rotation() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let station = SplitterStation::new(Cell::new(8, 8), rotation, design, 40, 40);
                for channel in [
                    SplitterChannel::Entry,
                    SplitterChannel::NewSnek,
                    SplitterChannel::OldSnek,
                ] {
                    let path = design.channel(channel);
                    for (index, (row, col)) in path.iter().copied().enumerate() {
                        let current = station.world_cell(row, col);
                        assert!(station.cuts(current));
                        let (direction, next) = station
                            .route_step(current, channel, 40, 40)
                            .expect("declared channel cell should route");
                        if let Some((next_row, next_col)) = path.get(index + 1).copied() {
                            assert_eq!(next, station.world_cell(next_row, next_col));
                            assert_eq!(direction_between(current, next), Some(direction));
                        } else {
                            assert_eq!(direction, station.channel_direction(channel, false));
                            if channel != SplitterChannel::Entry {
                                assert!(
                                    !station.contains(next),
                                    "{design:?} rotation {rotation} {channel:?} does not exit"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn splitter_sends_each_snek_along_its_design_channel() {
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
                let old_path = design.channel(SplitterChannel::OldSnek);
                let new_path = design.channel(SplitterChannel::NewSnek);
                let released_old_head = station.world_cell(old_path[1].0, old_path[1].1);
                let released_new_head = station.world_cell(new_path[4].0, new_path[4].1);
                assert_eq!(game.sneks[0].head(), released_old_head);
                assert_eq!(
                    game.sneks[1].head(),
                    released_new_head,
                    "{design:?} rotation {rotation} released newborn on the wrong channel"
                );
                let expected_new_positions: VecDeque<_> = new_path[2..=4]
                    .iter()
                    .map(|(row, col)| station.world_cell(*row, *col))
                    .collect();
                assert_eq!(game.sneks[1].positions, expected_new_positions);
                assert!(matches!(
                    game.splitter_activities().as_slice(),
                    [SplitterActivity {
                        phase: SplitterActivityPhase::Releasing,
                        ..
                    }]
                ));

                let expected_old_next = station
                    .route_step(released_old_head, SplitterChannel::OldSnek, 40, 40)
                    .unwrap()
                    .1;
                let expected_new_next = station
                    .route_step(released_new_head, SplitterChannel::NewSnek, 40, 40)
                    .unwrap()
                    .1;
                game.step(&mut Lcg::new(4));
                assert_eq!(game.sneks[0].head(), expected_old_next);
                assert_eq!(game.sneks[1].head(), expected_new_next);

                for _ in 0..design.width() + design.height() {
                    if !station.contains(game.sneks[1].head()) {
                        break;
                    }
                    game.step(&mut Lcg::new(5));
                }
                assert!(
                    !station.contains(game.sneks[1].head()),
                    "{design:?} rotation {rotation} newborn did not leave the process lane"
                );
            }
        }
    }

    #[test]
    fn stray_snek_on_output_channel_routes_out_instead_of_into_station_wall() {
        let mut game = SnekGame::new(30, 30, 1);
        let station = SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30);
        let old_path = station.design.channel(SplitterChannel::OldSnek);
        let side_cell = station.world_cell(old_path[1].0, old_path[1].1);
        let expected = station
            .route_step(side_cell, SplitterChannel::OldSnek, 30, 30)
            .unwrap()
            .1;
        game.splitter_stations = vec![station];
        game.sneks[0].positions = vec![side_cell; MIN_SNEK_LEN].into();
        game.sneks[0].direction = station.flow_direction();
        game.apples = vec![Cell::new(1, 1)];

        game.step(&mut Lcg::new(2));

        assert_eq!(game.sneks[0].head(), expected);
        assert!(station.cuts(game.sneks[0].head()));

        for _ in 0..station.height() {
            game.step(&mut Lcg::new(3));
            if !station.contains(game.sneks[0].head()) {
                return;
            }
        }

        panic!("snek stayed inside splitter side-exit corridor");
    }

    #[test]
    fn snek_inside_splitter_footprint_prefers_to_leave() {
        let mut game = SnekGame::new(20, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Splitter, 20, 20);
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
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Splitter, 20, 20);
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
        game.sneks.push(Snek::new(2, Cell::new(12, 12)));
        game.lifetime_apls = 250;
        game.juice_segments = APPLE_COST * SEGMENTS_PER_JUICE;
        let initial_apples = game.apples().len();

        assert!(game.buy_apple(&mut Lcg::new(2)));
        game.step(&mut Lcg::new(3));

        assert_eq!(game.apples().len(), initial_apples + 1);
        assert_eq!(game.juice(), 0);
    }

    #[test]
    fn splitter_station_rotation_swaps_footprint_dimensions() {
        for design in StationDesign::ALL {
            let station = SplitterStation::new(Cell::new(18, 18), 1, design, 40, 40);

            assert_eq!(station.origin(), Cell::new(18, 18));
            assert_eq!(station.width(), design.height());
            assert_eq!(station.height(), design.width());
            assert!(station.contains(Cell::new(19, 19)));
            assert!(!station.contains(Cell::new(17, 19)));
        }
    }

    #[test]
    fn splitter_station_delete_refunds_segments() {
        let mut game = SnekGame::new(20, 20, 1);
        game.splitter_stations = vec![SplitterStation::new(
            Cell::new(5, 5),
            1,
            StationDesign::Splitter,
            20,
            20,
        )];

        assert!(game.delete_splitter_station(0));

        assert!(game.splitter_stations.is_empty());
        assert_eq!(game.juice(), SPLITTER_STATION_COST);
    }

    #[test]
    fn deleting_ghost_reclaims_whole_juice_cargo() {
        let mut game = SnekGame::new(30, 30, 1);
        let ghost =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30).with_id(1);
        game.splitter_stations = vec![ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 1,
            remaining_segments: 3,
        }];
        game.sneks[0].carried_juice_segments = 2;

        assert!(game.delete_splitter_station(0));
        assert_eq!(game.sneks[0].carried_juice_segments, 0);
        assert_eq!(game.juice(), SPLITTER_STATION_COST - 1);
    }

    #[test]
    fn paid_bootstrap_replacement_remains_refundable() {
        let mut game = SnekGame::new(40, 40, 1);
        let free =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 40).with_id(1);
        let paid =
            SplitterStation::new(Cell::new(16, 5), 0, StationDesign::Juicer, 40, 40).with_id(2);
        let replacement =
            SplitterStation::new(Cell::new(27, 5), 0, StationDesign::Juicer, 40, 40).with_id(3);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.bootstrap_free_juicer_id = Some(1);
        game.splitter_stations = vec![free, paid, replacement];

        assert!(game.delete_splitter_station(0));
        assert_eq!(game.juice(), 0);
        assert_eq!(game.bootstrap_juicer_id, Some(2));

        assert!(game.delete_splitter_station(0));
        assert_eq!(game.juice(), JUICER_STATION_COST);
        assert_eq!(game.bootstrap_juicer_id, Some(3));
    }

    #[test]
    fn max_length_snek_routes_through_juicer_and_converts_excess() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer];
        game.max_snek_len = Some(13);
        game.sneks[0].len = game.max_snek_len.unwrap();
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.max_snek_len.unwrap()].into();
        game.apples = vec![Cell::new(2, 2)];

        for _ in 0..100 {
            game.step(&mut Lcg::new(2));
            if game.juicer_jobs.is_empty() && game.sneks[0].len == MIN_SNEK_LEN {
                break;
            }
        }

        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert_eq!(game.juice(), 1);
        assert!(game.juicer_activity(juicer).is_some());
    }

    #[test]
    fn juicer_does_not_claim_a_snek_until_maximum_is_set() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer];
        game.sneks[0].add_segments(30);
        game.max_snek_len = None;

        let dispatch = game.splitter_dispatch();

        assert_eq!(dispatch.station_for_snek(0), None);
    }

    #[test]
    fn manually_controlled_snek_can_make_the_first_juice_without_a_maximum() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.splitter_stations = vec![juicer];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE);
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
        game.sneks[0].direction = juicer.flow_direction();
        assert!(!game.juice_discovered());

        for _ in 0..100 {
            game.step(&mut Lcg::new(2));
            if game.juice() > 0 {
                break;
            }
        }

        assert_eq!(game.juice(), 1);
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert!(game.juice_discovered());
    }

    #[test]
    fn juicer_feeds_and_credits_detached_tail_one_segment_at_a_time() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer];
        game.sneks[0].add_segments(SEGMENTS_PER_JUICE);
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
        game.sneks[0].direction = juicer.flow_direction();

        for _ in 0..20 {
            game.step(&mut Lcg::new(2));
            if !game.juicer_jobs.is_empty() {
                break;
            }
        }

        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert_eq!(game.sneks[0].head(), juicer.cut_target());
        assert_eq!(game.juice_segments, 0);
        assert_eq!(game.juicer_jobs[0].feed_segments.len(), SEGMENTS_PER_JUICE);

        let mut previous = 0;
        for _ in 0..100 {
            game.step(&mut Lcg::new(3));
            assert!(game.juice_segments.saturating_sub(previous) <= 1);
            previous = game.juice_segments;
            if game.juicer_jobs.is_empty() {
                break;
            }
        }

        assert!(game.juicer_jobs.is_empty());
        assert_eq!(game.juice_segments, SEGMENTS_PER_JUICE);
    }

    #[test]
    fn active_juicer_feed_round_trips_through_save() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer];
        game.sneks[0].add_segments(4);
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
        game.sneks[0].direction = juicer.flow_direction();
        for _ in 0..20 {
            game.step(&mut Lcg::new(2));
            if !game.juicer_jobs.is_empty() {
                break;
            }
        }
        game.step(&mut Lcg::new(3));

        let expected = game.juicer_jobs.clone();
        let resumed = SnekGame::from_save(game.save(), 30, 30).unwrap();

        assert_eq!(resumed.juicer_jobs, expected);
        assert!(resumed.station_busy(juicer));
    }

    #[test]
    fn juicer_accepts_less_than_three_excess_segments() {
        for excess in 1..MIN_SNEK_LEN {
            let mut game = SnekGame::new(30, 30, 1);
            let juicer =
                SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
            game.juicer_unlocked = true;
            game.bootstrap_phase = BootstrapPhase::Complete;
            game.splitter_stations = vec![juicer];
            game.sneks[0].add_segments(excess);
            game.sneks[0].positions =
                vec![juicer.pre_entry_cell(game.cols, game.rows); game.sneks[0].len].into();
            game.sneks[0].direction = juicer.flow_direction();

            for _ in 0..100 {
                game.step(&mut Lcg::new(2));
                if game.juicer_jobs.is_empty() && game.sneks[0].len == MIN_SNEK_LEN {
                    break;
                }
            }

            assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
            assert_eq!(game.juice_segments, excess);
        }
    }

    #[test]
    fn final_minimum_snek_passes_through_juicer_unchanged() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer];
        game.sneks[0].positions =
            vec![juicer.pre_entry_cell(game.cols, game.rows); MIN_SNEK_LEN].into();
        game.sneks[0].direction = juicer.flow_direction();

        for _ in 0..40 {
            game.step(&mut Lcg::new(2));
            if !juicer.contains(game.sneks[0].head())
                && game.sneks[0].head() != juicer.pre_entry_cell(game.cols, game.rows)
            {
                break;
            }
        }

        assert_eq!(game.sneks.len(), 1);
        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert_eq!(game.juice_segments, 0);
        assert!(game.juicer_jobs.is_empty());
    }

    #[test]
    fn nonfinal_minimum_snek_contributes_all_segments_to_juice() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer];
        game.sneks = vec![
            Snek::new(1, juicer.pre_entry_cell(game.cols, game.rows)),
            Snek::new(2, Cell::new(2, 2)),
        ];
        game.sneks[0].direction = juicer.flow_direction();
        game.manual_snek = Some(1);
        game.apples = vec![Cell::new(25, 25)];

        for _ in 0..80 {
            game.step(&mut Lcg::new(2));
            if game.juicer_jobs.is_empty() && game.sneks.len() == 1 {
                break;
            }
        }

        assert_eq!(game.sneks.len(), 1);
        assert_eq!(game.sneks[0].id, 2);
        assert_eq!(game.juice_segments, MIN_SNEK_LEN);
    }

    #[test]
    fn juicer_service_block_loads_one_whole_juice_per_bump() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        let ghost =
            SplitterStation::new(Cell::new(18, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: MIN_SNEK_LEN,
        }];
        game.juice_segments = 4 * SEGMENTS_PER_JUICE;
        game.sneks[0].positions =
            vec![juicer.service_approach_cell(game.cols, game.rows); MIN_SNEK_LEN].into();
        game.sneks[0].direction = direction_between(
            juicer.service_approach_cell(game.cols, game.rows),
            juicer.service_cell(),
        )
        .unwrap();

        for expected in 1..=MIN_SNEK_LEN {
            let dispatch = game.splitter_dispatch();
            let head = game.sneks[0].head();
            assert_eq!(game.next_snek_cell(0, &dispatch), head);
            assert_eq!(game.sneks[0].carried_juice_segments, expected);
            assert_eq!(game.juice_segments, (4 - expected) * SEGMENTS_PER_JUICE);
        }

        assert_eq!(game.unallocated_construction_segments(), 0);
        let dispatch = game.splitter_dispatch();
        assert_ne!(game.next_snek_cell(0, &dispatch), game.sneks[0].head());
    }

    #[test]
    fn juicer_service_block_never_loads_partial_juice() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(18, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 1,
        }];
        game.juice_segments = SEGMENTS_PER_JUICE - 1;

        assert!(!game.handle_service_bump(0, juicer.service_cell()));
        assert_eq!(game.sneks[0].carried_juice_segments, 0);
        assert_eq!(game.juice_segments, SEGMENTS_PER_JUICE - 1);

        game.juice_segments += 1;
        assert!(game.handle_service_bump(0, juicer.service_cell()));
        assert_eq!(game.sneks[0].carried_juice_segments, 1);
        assert_eq!(game.juice_segments, 0);
    }

    #[test]
    fn cargo_can_fund_any_occupied_ghost_tile() {
        let mut game = SnekGame::new(30, 30, 1);
        let ghost =
            SplitterStation::new(Cell::new(10, 10), 0, StationDesign::Splitter, 30, 30).with_id(1);
        game.splitter_unlocked = true;
        game.splitter_stations = vec![ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 1,
            remaining_segments: 2,
        }];
        game.sneks[0].carried_juice_segments = 2;
        let wall = Cell::new(ghost.origin.col, ghost.origin.row);
        assert!(ghost.occupies(wall));
        assert_ne!(wall, ghost.service_cell());

        assert!(game.handle_service_bump(0, wall));
        assert_eq!(game.sneks[0].carried_juice_segments, 1);
        assert_eq!(game.construction_sites[0].remaining_segments, 1);
    }

    #[test]
    fn auto_snek_loads_and_delivers_juice_to_construction() {
        let mut game = SnekGame::new(40, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Juicer, 40, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(24, 12), 0, StationDesign::Splitter, 40, 30).with_id(2);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_unlocked = true;
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 10,
        }];
        game.juice_segments = 6;
        game.apples = vec![Cell::new(35, 25)];

        for _ in 0..500 {
            game.step(&mut Lcg::new(8));
            if game.construction_sites[0].remaining_segments < 10 {
                break;
            }
        }

        assert!(game.construction_sites[0].remaining_segments < 10);
        assert!(game.juice_segments < 6 || game.sneks[0].carried_juice_segments > 0);
    }

    #[test]
    fn carried_juice_round_trips_and_cannot_exceed_snek_length() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let ghost =
            SplitterStation::new(Cell::new(18, 12), 0, StationDesign::Splitter, 30, 30).with_id(2);
        game.juicer_unlocked = true;
        game.bootstrap_phase = BootstrapPhase::Complete;
        game.bootstrap_juicer_id = Some(1);
        game.splitter_stations = vec![juicer, ghost];
        game.construction_sites = vec![ConstructionSite {
            station_id: 2,
            remaining_segments: 2,
        }];
        game.sneks[0].carried_juice_segments = 2;

        let saved = game.save();
        let resumed = SnekGame::from_save(saved.clone(), 30, 30).unwrap();
        assert_eq!(resumed.sneks[0].carried_juice_segments, 2);

        let mut invalid = saved;
        invalid.sneks[0].carried_juice_segments = MIN_SNEK_LEN + 1;
        assert!(SnekGame::from_save(invalid, 30, 30).is_none());
    }

    #[test]
    fn locked_bootstrap_save_rejects_unreachable_carried_juice() {
        let mut game = SnekGame::new(30, 30, 1);
        game.sneks[0].carried_juice_segments = 1;

        assert!(SnekGame::from_save(game.save(), 30, 30).is_none());
    }

    #[test]
    fn unassigned_splitter_passes_short_sneks_and_splits_six_plus() {
        for length in [MIN_SNEK_LEN, MIN_SNEK_LEN * 2] {
            let mut game = SnekGame::new(30, 30, 1);
            let splitter =
                SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30)
                    .with_id(1);
            game.splitter_unlocked = true;
            game.splitter_stations = vec![splitter];
            game.sneks[0] = Snek::new_with_direction(
                1,
                splitter.pre_entry_cell(game.cols, game.rows),
                splitter.flow_direction(),
                length,
            );
            if length == MIN_SNEK_LEN * 2 {
                game.sneks[0].carried_juice_segments = MIN_SNEK_LEN;
            }

            for _ in 0..80 {
                game.step(&mut Lcg::new(3));
                if game.sneks.len() == 2
                    || (length < MIN_SNEK_LEN * 2
                        && !splitter.contains(game.sneks[0].head())
                        && game.sneks[0].head() != splitter.pre_entry_cell(game.cols, game.rows))
                {
                    break;
                }
            }

            if length < MIN_SNEK_LEN * 2 {
                assert_eq!(game.sneks.len(), 1);
                assert_eq!(game.sneks[0].len, length);
            } else {
                assert_eq!(game.sneks.len(), 2);
                assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
                assert_eq!(game.sneks[1].len, MIN_SNEK_LEN);
                assert_eq!(game.sneks[0].carried_juice_segments, 0);
                assert_eq!(game.sneks[1].carried_juice_segments, MIN_SNEK_LEN);
            }
        }
    }

    #[test]
    fn active_machine_job_cannot_enter_a_second_station() {
        let mut game = SnekGame::new(40, 30, 1);
        let active =
            SplitterStation::new(Cell::new(4, 4), 0, StationDesign::Splitter, 40, 30).with_id(1);
        let other =
            SplitterStation::new(Cell::new(22, 8), 0, StationDesign::Juicer, 40, 30).with_id(2);
        let entry = other.pre_entry_cell(game.cols, game.rows);
        game.splitter_unlocked = true;
        game.juicer_unlocked = true;
        game.splitter_stations = vec![active, other];
        game.sneks[0] =
            Snek::new_with_direction(1, entry, other.flow_direction(), MIN_SNEK_LEN * 2);
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Feeding {
                snek_id: 1,
                station: active,
                advanced: 0,
            },
        }];

        let dispatch = game.splitter_dispatch();
        assert!(!game.can_enter_splitter(0, entry, other.entry_cell(), &dispatch,));
    }

    #[test]
    fn population_management_unlocks_when_first_splitter_is_placed() {
        let mut game = SnekGame::new(30, 30, 1);
        game.splitter_unlocked = true;
        assert!(!game.population_unlocked());

        assert!(game.buy_splitter_station(
            Cell::new(8, 8),
            0,
            StationDesign::Splitter,
            &mut Lcg::new(2),
        ));

        assert!(game.population_unlocked());
    }

    #[test]
    fn excess_min_length_snek_can_be_juiced_but_last_snek_is_protected() {
        let mut game = SnekGame::new(30, 30, 1);
        let juicer =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Juicer, 30, 30).with_id(1);
        let entry = juicer.pre_entry_cell(game.cols, game.rows);
        game.juicer_unlocked = true;
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        game.splitter_stations = vec![juicer];
        game.sneks = vec![Snek::new(1, entry), Snek::new(2, Cell::new(2, 2))];
        game.apples = vec![Cell::new(25, 25)];

        for _ in 0..100 {
            game.step(&mut Lcg::new(2));
            if game.sneks.len() == 1 && game.juicer_jobs.is_empty() {
                break;
            }
        }

        assert_eq!(game.sneks.len(), 1);
        assert_eq!(game.juice_segments, MIN_SNEK_LEN);
        for _ in 0..20 {
            game.step(&mut Lcg::new(3));
        }
        assert_eq!(game.sneks.len(), 1);
    }

    #[test]
    fn construction_ghost_requires_delivered_juice_before_becoming_active() {
        let mut game = SnekGame::new(30, 30, 1);
        game.juicer_unlocked = true;
        game.splitter_unlocked = true;
        assert!(game.buy_splitter_station(
            Cell::new(8, 8),
            0,
            StationDesign::Splitter,
            &mut Lcg::new(2),
        ));
        let station = game.splitter_stations[0];
        let initial_progress = game.construction_progress(station);
        game.step(&mut Lcg::new(9));
        assert_eq!(game.construction_progress(station), initial_progress);

        for _ in 0..SPLITTER_STATION_COST {
            game.sneks[0].carried_juice_segments = 1;
            assert!(game.handle_service_bump(0, station.service_cell()));
        }

        assert_eq!(game.sneks[0].carried_juice_segments, 0);
        assert_eq!(game.construction_progress(station), None);
        assert!(game.station_complete(station));
    }

    #[test]
    fn splitter_phase_round_trips_through_save() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30).with_id(7);
        game.auto_snek_unlocked = true;
        game.splitter_unlocked = true;
        game.splitter_stations = vec![station];
        let new_path = station.design.channel(SplitterChannel::NewSnek);
        let mut cutting_positions = vec![station.cut_target(); MIN_SNEK_LEN];
        cutting_positions.extend(
            new_path[1..=MIN_SNEK_LEN]
                .iter()
                .map(|(row, col)| station.world_cell(*row, *col)),
        );
        game.sneks[0] = Snek::new_with_direction(
            1,
            station.cut_target(),
            station.new_exit_direction(),
            MIN_SNEK_LEN * 2,
        );
        game.sneks[0].positions = cutting_positions.into();
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
    fn saved_splitter_phase_must_match_the_snek_channel_position() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30).with_id(1);
        game.auto_snek_unlocked = true;
        game.splitter_unlocked = true;
        game.splitter_stations = vec![station];
        game.sneks[0] =
            Snek::new_with_direction(1, Cell::new(1, 1), Direction::Right, MIN_SNEK_LEN * 2);
        game.splitter_jobs = vec![SplitterJob {
            source_snek_id: 1,
            phase: SplitterJobPhase::Feeding {
                snek_id: 1,
                station,
                advanced: 1,
            },
        }];

        assert!(SnekGame::from_save(game.save(), 30, 30).is_none());
    }

    #[test]
    fn saved_splitter_orders_must_fit_source_excess_segments() {
        let mut game = SnekGame::new(30, 30, 1);
        let station =
            SplitterStation::new(Cell::new(8, 8), 0, StationDesign::Splitter, 30, 30).with_id(1);
        game.auto_snek_unlocked = true;
        game.splitter_unlocked = true;
        game.splitter_stations = vec![station];
        game.splitter_jobs = vec![SplitterJob::queued(1)];

        assert_eq!(game.sneks[0].len, MIN_SNEK_LEN);
        assert!(SnekGame::from_save(game.save(), 30, 30).is_none());
    }

    #[test]
    fn queued_order_reserves_segments_on_its_source_snek() {
        let mut game = SnekGame::new(30, 20, 1);
        game.auto_snek_unlocked = true;
        game.manual_snek = None;
        let station =
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Splitter, 30, 20).with_id(1);
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
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Splitter, 30, 20).with_id(1);
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
            SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Splitter, 30, 20).with_id(1);
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
