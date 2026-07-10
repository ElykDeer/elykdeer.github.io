#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{HashMap, VecDeque};

use super::state::{Cell, Direction, SplitterStation};

pub(super) fn idle_direction(
    id: usize,
    from: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    stations: &[SplitterStation],
) -> Direction {
    // TODO: Add optional idle choreography for long, nearby idle sneks to spell SNEK.
    let options = idle_options(id, from, cols, rows, current, stations);
    unique_directions(options)
        .into_iter()
        .find(|direction| {
            !direction_blocked(
                from,
                *direction,
                cols,
                rows,
                stations,
                PathMode::AvoidStation,
            )
        })
        .unwrap_or(current.reversed())
}

fn idle_options(
    id: usize,
    from: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    stations: &[SplitterStation],
) -> [Direction; 4] {
    let cadence = 4 + id % 5;
    match id % 8 {
        0 => idle_box_options(from.col.is_multiple_of(cadence), current, true),
        1 => idle_box_options(from.row.is_multiple_of(cadence), current, false),
        2 => idle_weave_options(id, from, cadence, current),
        3 => idle_lawnmower_options(id, from, cadence, current),
        4 => idle_orbit_options(id, from, cols, rows, current, stations),
        5 => idle_figure_eight_options(id, from, cadence, current),
        6 => idle_corner_patrol_options(id, from, cols, rows, current, stations),
        _ => idle_breathe_options(id, from, cadence, current),
    }
}

fn idle_box_options(turn: bool, current: Direction, clockwise: bool) -> [Direction; 4] {
    let first = if turn {
        if clockwise {
            current.clockwise()
        } else {
            current.counter_clockwise()
        }
    } else {
        current
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_weave_options(id: usize, from: Cell, cadence: usize, current: Direction) -> [Direction; 4] {
    let stripe = (from.col / cadence + from.row / cadence + id) % 2;
    let first = match current {
        Direction::Left | Direction::Right if from.col.is_multiple_of(cadence) => {
            if stripe == 0 {
                Direction::Down
            } else {
                Direction::Up
            }
        }
        Direction::Up | Direction::Down if from.row.is_multiple_of(cadence) => {
            if stripe == 0 {
                Direction::Right
            } else {
                Direction::Left
            }
        }
        _ => current,
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_lawnmower_options(
    id: usize,
    from: Cell,
    cadence: usize,
    current: Direction,
) -> [Direction; 4] {
    let horizontal = if (from.row / cadence + id).is_multiple_of(2) {
        Direction::Right
    } else {
        Direction::Left
    };
    let first = if matches!(current, Direction::Left | Direction::Right)
        && from.col % cadence == cadence.saturating_sub(1)
    {
        if id.is_multiple_of(2) {
            Direction::Down
        } else {
            Direction::Up
        }
    } else {
        horizontal
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_orbit_options(
    id: usize,
    from: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    stations: &[SplitterStation],
) -> [Direction; 4] {
    let center = stations
        .first()
        .map(|station| {
            Cell::new(
                station.origin().col + station.width() / 2,
                station.origin().row + station.height() / 2,
            )
        })
        .unwrap_or(Cell::new(cols / 2, rows / 2));
    let dx = wrapped_delta(center.col, from.col, cols);
    let dy = wrapped_delta(center.row, from.row, rows);
    let first = if dx.abs() >= dy.abs() {
        if (dx >= 0) == id.is_multiple_of(2) {
            Direction::Down
        } else {
            Direction::Up
        }
    } else if (dy >= 0) == id.is_multiple_of(2) {
        Direction::Left
    } else {
        Direction::Right
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_figure_eight_options(
    id: usize,
    from: Cell,
    cadence: usize,
    current: Direction,
) -> [Direction; 4] {
    let phase = (from.col / cadence + from.row / cadence + id) % 4;
    let turn = (from.col + from.row + id).is_multiple_of(cadence);
    let first = if turn {
        match phase {
            0 => current.clockwise(),
            1 => current.counter_clockwise(),
            2 => current.counter_clockwise(),
            _ => current.clockwise(),
        }
    } else {
        current
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_corner_patrol_options(
    id: usize,
    from: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    stations: &[SplitterStation],
) -> [Direction; 4] {
    let margin = 3 + id % 4;
    let targets = [
        Cell::new(
            margin.min(cols.saturating_sub(1)),
            margin.min(rows.saturating_sub(1)),
        ),
        Cell::new(
            cols.saturating_sub(1 + margin),
            margin.min(rows.saturating_sub(1)),
        ),
        Cell::new(
            cols.saturating_sub(1 + margin),
            rows.saturating_sub(1 + margin),
        ),
        Cell::new(
            margin.min(cols.saturating_sub(1)),
            rows.saturating_sub(1 + margin),
        ),
    ];
    let target = targets[(id + from.row / 5 + from.col / 7) % targets.len()];
    let first = greedy_direction_toward(
        from,
        target,
        cols,
        rows,
        current,
        stations,
        PathMode::AvoidStation,
    );
    [first, current, first.clockwise(), first.counter_clockwise()]
}

fn idle_breathe_options(
    id: usize,
    from: Cell,
    cadence: usize,
    current: Direction,
) -> [Direction; 4] {
    let pulse = (from.col / cadence + from.row / cadence + id) % 3;
    let first = match pulse {
        0 if from.col.is_multiple_of(cadence) => current.clockwise(),
        1 if from.row.is_multiple_of(cadence) => current.counter_clockwise(),
        _ => current,
    };
    [first, current, first.clockwise(), first.counter_clockwise()]
}

pub(super) struct NavigationCache {
    cols: usize,
    rows: usize,
    stations: Vec<SplitterStation>,
    fields: HashMap<NavigationKey, Vec<usize>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct NavigationKey {
    target: Cell,
    mode: PathMode,
}

impl NavigationCache {
    pub(super) fn new(cols: usize, rows: usize, stations: Vec<SplitterStation>) -> Self {
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
            stations,
            fields: HashMap::new(),
        }
    }

    pub(super) fn direction_toward(
        &mut self,
        from: Cell,
        target: Cell,
        current: Direction,
        mode: PathMode,
    ) -> Direction {
        if from == target {
            return current;
        }
        let key = NavigationKey { target, mode };
        if !self.fields.contains_key(&key) {
            let field = self.build_distance_field(target, mode);
            self.fields.insert(key, field);
        }
        let distances = &self.fields[&key];
        if let Some(direction) = ordered_directions(from, target, self.cols, self.rows, current)
            .into_iter()
            .enumerate()
            .filter_map(|(order, direction)| {
                let next = from.stepped(direction, self.cols, self.rows);
                let distance = distances[cell_index(next, self.cols)];
                (distance != usize::MAX).then_some((distance, order, direction))
            })
            .min_by_key(|(distance, order, _)| (*distance, *order))
            .map(|(_, _, direction)| direction)
        {
            return direction;
        }
        greedy_direction_toward(
            from,
            target,
            self.cols,
            self.rows,
            current,
            &self.stations,
            mode,
        )
    }

    fn build_distance_field(&self, target: Cell, mode: PathMode) -> Vec<usize> {
        let mut distances = vec![usize::MAX; self.cols * self.rows];
        if !path_cell_passable(target, &self.stations, mode) {
            return distances;
        }
        let mut queue = VecDeque::new();
        distances[cell_index(target, self.cols)] = 0;
        queue.push_back(target);
        while let Some(cell) = queue.pop_front() {
            let next_distance = distances[cell_index(cell, self.cols)] + 1;
            for direction in [
                Direction::Up,
                Direction::Right,
                Direction::Down,
                Direction::Left,
            ] {
                let next = cell.stepped(direction, self.cols, self.rows);
                let index = cell_index(next, self.cols);
                if distances[index] != usize::MAX || !path_cell_passable(next, &self.stations, mode)
                {
                    continue;
                }
                distances[index] = next_distance;
                queue.push_back(next);
            }
        }
        distances
    }
}

fn greedy_direction_toward(
    from: Cell,
    to: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    stations: &[SplitterStation],
    path_mode: PathMode,
) -> Direction {
    let dx = wrapped_delta(from.col, to.col, cols);
    let dy = wrapped_delta(from.row, to.row, rows);
    let preferred = if dx.abs() > 5 && dy.abs() > 5 {
        match current {
            Direction::Left | Direction::Right if dx != 0 => horizontal_direction(dx),
            Direction::Up | Direction::Down if dy != 0 => vertical_direction(dy),
            _ if dx.abs() >= dy.abs() && dx != 0 => horizontal_direction(dx),
            _ => vertical_direction(dy),
        }
    } else if dx.abs() >= dy.abs() && dx != 0 {
        horizontal_direction(dx)
    } else {
        vertical_direction(dy)
    };

    if !direction_blocked(from, preferred, cols, rows, stations, path_mode) {
        return preferred;
    }

    let mut options = [
        horizontal_direction(dx),
        vertical_direction(dy),
        preferred.clockwise(),
        preferred.counter_clockwise(),
    ];
    options.sort_by_key(|direction| {
        if direction_blocked(from, *direction, cols, rows, stations, path_mode) {
            usize::MAX
        } else {
            toroidal_distance(from.stepped(*direction, cols, rows), to, cols, rows)
        }
    });
    options
        .into_iter()
        .find(|direction| !direction_blocked(from, *direction, cols, rows, stations, path_mode))
        .unwrap_or(preferred.reversed())
}

fn ordered_directions(
    from: Cell,
    to: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
) -> [Direction; 4] {
    let dx = wrapped_delta(from.col, to.col, cols);
    let dy = wrapped_delta(from.row, to.row, rows);
    let first = if dx.abs() > 5 && dy.abs() > 5 {
        match current {
            Direction::Left | Direction::Right if dx != 0 => horizontal_direction(dx),
            Direction::Up | Direction::Down if dy != 0 => vertical_direction(dy),
            _ if dx.abs() >= dy.abs() && dx != 0 => horizontal_direction(dx),
            _ if dy != 0 => vertical_direction(dy),
            _ => current,
        }
    } else if dx.abs() >= dy.abs() && dx != 0 {
        horizontal_direction(dx)
    } else if dy != 0 {
        vertical_direction(dy)
    } else {
        current
    };
    let second = if matches!(first, Direction::Left | Direction::Right) {
        if dy == 0 {
            first.clockwise()
        } else {
            vertical_direction(dy)
        }
    } else if dx == 0 {
        first.clockwise()
    } else {
        horizontal_direction(dx)
    };
    unique_directions([first, second, first.clockwise(), first.counter_clockwise()])
}

pub(super) fn unique_directions(mut directions: [Direction; 4]) -> [Direction; 4] {
    let fallback = [
        Direction::Up,
        Direction::Right,
        Direction::Down,
        Direction::Left,
    ];
    let mut used = Vec::new();
    for direction in &mut directions {
        if used.contains(direction) {
            if let Some(replacement) = fallback
                .iter()
                .copied()
                .find(|direction| !used.contains(direction))
            {
                *direction = replacement;
            }
        }
        used.push(*direction);
    }
    directions
}

fn path_cell_passable(cell: Cell, stations: &[SplitterStation], path_mode: PathMode) -> bool {
    match path_mode {
        PathMode::AvoidStation => !stations.iter().any(|station| station.occupies(cell)),
    }
}

fn cell_index(cell: Cell, cols: usize) -> usize {
    cell.row * cols + cell.col
}

fn direction_blocked(
    from: Cell,
    direction: Direction,
    cols: usize,
    rows: usize,
    stations: &[SplitterStation],
    path_mode: PathMode,
) -> bool {
    !path_cell_passable(from.stepped(direction, cols, rows), stations, path_mode)
}

pub(super) fn direction_out_of_station(
    from: Cell,
    cols: usize,
    rows: usize,
    current: Direction,
    station: SplitterStation,
) -> Option<Direction> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut seen = vec![false; cols * rows];
    let mut queue = VecDeque::new();
    let start_index = cell_index(from, cols);
    seen[start_index] = true;
    queue.push_back((from, None));

    while let Some((cell, first_direction)) = queue.pop_front() {
        if !station.occupies(cell) {
            return first_direction;
        }

        for direction in unique_directions([
            current,
            current.clockwise(),
            current.counter_clockwise(),
            current.reversed(),
        ]) {
            let next = cell.stepped(direction, cols, rows);
            let index = cell_index(next, cols);
            if seen[index] {
                continue;
            }
            seen[index] = true;
            queue.push_back((next, first_direction.or(Some(direction))));
        }
    }

    None
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum PathMode {
    AvoidStation,
}

fn horizontal_direction(dx: isize) -> Direction {
    if dx > 0 {
        Direction::Right
    } else {
        Direction::Left
    }
}

fn vertical_direction(dy: isize) -> Direction {
    if dy > 0 {
        Direction::Down
    } else {
        Direction::Up
    }
}

impl Cell {
    pub(super) fn stepped(self, direction: Direction, cols: usize, rows: usize) -> Self {
        match direction {
            Direction::Up => Self::new(
                self.col,
                if self.row == 0 {
                    rows.saturating_sub(1)
                } else {
                    self.row - 1
                },
            ),
            Direction::Down => Self::new(self.col, (self.row + 1) % rows.max(1)),
            Direction::Left => Self::new(
                if self.col == 0 {
                    cols.saturating_sub(1)
                } else {
                    self.col - 1
                },
                self.row,
            ),
            Direction::Right => Self::new((self.col + 1) % cols.max(1), self.row),
        }
    }
}

impl Direction {
    pub(super) fn reversed(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    pub(super) fn clockwise(self) -> Self {
        match self {
            Self::Up => Self::Right,
            Self::Right => Self::Down,
            Self::Down => Self::Left,
            Self::Left => Self::Up,
        }
    }

    pub(super) fn counter_clockwise(self) -> Self {
        match self {
            Self::Up => Self::Left,
            Self::Left => Self::Down,
            Self::Down => Self::Right,
            Self::Right => Self::Up,
        }
    }
}

pub(super) fn toroidal_distance(a: Cell, b: Cell, cols: usize, rows: usize) -> usize {
    wrapped_delta(a.col, b.col, cols).unsigned_abs()
        + wrapped_delta(a.row, b.row, rows).unsigned_abs()
}

fn wrapped_delta(from: usize, to: usize, size: usize) -> isize {
    let size = size.max(1) as isize;
    let raw = to as isize - from as isize;
    if raw.abs() <= size / 2 {
        raw
    } else if raw > 0 {
        raw - size
    } else {
        raw + size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::snek::state::StationDesign;

    #[test]
    fn idle_sneks_use_varied_holding_patterns() {
        let directions: Vec<_> = (1..=8)
            .map(|id| idle_direction(id, Cell::new(8, 8), 30, 30, Direction::Right, &[]))
            .collect();

        assert!(directions
            .iter()
            .any(|direction| *direction != Direction::Right));
        assert!(directions.windows(2).any(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn long_diagonal_autopilot_keeps_current_axis_for_box_paths() {
        let mut navigation = NavigationCache::new(30, 30, Vec::new());
        assert_eq!(
            navigation.direction_toward(
                Cell::new(2, 2),
                Cell::new(8, 8),
                Direction::Right,
                PathMode::AvoidStation,
            ),
            Direction::Right
        );
        assert_eq!(
            navigation.direction_toward(
                Cell::new(2, 2),
                Cell::new(8, 8),
                Direction::Down,
                PathMode::AvoidStation,
            ),
            Direction::Down
        );
        assert_eq!(
            navigation.direction_toward(
                Cell::new(2, 2),
                Cell::new(7, 8),
                Direction::Right,
                PathMode::AvoidStation,
            ),
            Direction::Down
        );
    }

    #[test]
    fn autopilot_routes_around_splitter_station() {
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 20, 20);
        let mut navigation = NavigationCache::new(20, 20, vec![station]);

        assert_ne!(
            navigation.direction_toward(
                Cell::new(7, 4),
                Cell::new(7, 12),
                Direction::Down,
                PathMode::AvoidStation,
            ),
            Direction::Down
        );
    }

    #[test]
    fn general_pathing_never_enters_a_splitter_lane() {
        for design in StationDesign::ALL {
            for rotation in 0..4 {
                let station = SplitterStation::new(Cell::new(8, 8), rotation, design, 40, 40);

                assert!(
                    !path_cell_passable(station.cut_target(), &[station], PathMode::AvoidStation,),
                    "{design:?} rotation {rotation} exposes cut target to pathing"
                );
                assert!(
                    !path_cell_passable(station.entry_cell(), &[station], PathMode::AvoidStation,),
                    "{design:?} rotation {rotation} exposes entry to general pathing"
                );
            }
        }
    }

    #[test]
    fn navigation_reuses_distance_field_for_shared_target() {
        let mut navigation = NavigationCache::new(30, 30, Vec::new());
        let target = Cell::new(20, 20);

        navigation.direction_toward(
            Cell::new(2, 2),
            target,
            Direction::Right,
            PathMode::AvoidStation,
        );
        navigation.direction_toward(
            Cell::new(4, 4),
            target,
            Direction::Down,
            PathMode::AvoidStation,
        );

        assert_eq!(navigation.fields.len(), 1);
    }

    #[test]
    fn irregular_splitter_corners_are_not_invisible_walls() {
        let station = SplitterStation::new(Cell::new(5, 5), 0, StationDesign::Gate, 20, 20);
        let open_corner = Cell::new(5, 5);
        let frame = Cell::new(7, 5);

        assert!(station.contains(open_corner));
        assert!(!station.occupies(open_corner));
        assert!(station.occupies(frame));
        assert!(path_cell_passable(
            open_corner,
            &[station],
            PathMode::AvoidStation
        ));
    }
}
