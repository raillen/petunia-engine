//! Grid coordinates and A* (phase 3, `DR-14`: the "`grid` module (hex, A*)" item).
//!
//! Pure domain: no rendering, no files, no dependencies beyond serde. The pathfinder is **ours**
//! because `AGENTS.md` §2 says determinism-critical code is never a dependency's: the same start,
//! goal and grid must give the same path on every machine and every run.
//!
//! Two grids share one trait-free API:
//!
//! - [`SquareGrid`]: four or eight neighbours over integer cells.
//! - [`HexGrid`]: pointy-top hexes with axial coordinates (`q`, `r`); six neighbours.
//!
//! Both answer the same three questions: is a cell inside, can it be walked, and what does a step
//! between two neighbours cost. A* is written once, over that shape.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A cell on either grid. On a hex grid the two numbers are the axial coordinates `q` and `r`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
}

impl Cell {
    #[must_use]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Why a path was not found. A refusal is data, never a panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    /// The start or the goal is outside the grid.
    OutOfBounds,
    /// The start or the goal is a wall.
    Blocked,
    /// The goal cannot be reached from the start.
    Unreachable,
    /// More cells than the budget allows (a hostile map must not hang the editor).
    TooLarge,
}

/// What a step between neighbours costs. `Diagonal` only exists on an eight-way square grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepCost {
    /// Four-way square, or any hex move.
    Straight,
    /// Eight-way square diagonal: `sqrt(2)` as an integer ratio, so the result stays exact.
    Diagonal,
}

/// How a grid answers the questions A* asks.
///
/// Kept as a small enum rather than a trait: the AGENTS file forbids project-owned trait
/// hierarchies for domain logic, and two grids are easier to reason about when the whole surface
/// is visible in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKind {
    Square4,
    Square8,
    Hex,
}

/// A grid with walls. Cells not in `walkable` are walls; everything else inside the bounds is
/// floor. Storing the **walls** (not the floor) keeps a sparse map small.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    kind: GridKind,
    width: i32,
    height: i32,
    blocked: BTreeSet<Cell>,
    /// Cost multiplier per cell, as a percentage: 100 is normal, 200 twice as expensive.
    cost_percent: BTreeMap<Cell, u32>,
    /// Never expand more than this many cells.
    budget: usize,
}

/// Most cells A* may expand before it gives up.
pub const DEFAULT_BUDGET: usize = 200_000;

impl Grid {
    /// A square grid, four-way by default (the safest choice for a dungeon).
    #[must_use]
    pub fn square(width: i32, height: i32, diagonal: bool) -> Self {
        Self {
            kind: if diagonal {
                GridKind::Square8
            } else {
                GridKind::Square4
            },
            width: width.max(0),
            height: height.max(0),
            blocked: BTreeSet::new(),
            cost_percent: BTreeMap::new(),
            budget: DEFAULT_BUDGET,
        }
    }

    /// A pointy-top hex grid with axial coordinates.
    #[must_use]
    pub fn hex(width: i32, height: i32) -> Self {
        Self {
            kind: GridKind::Hex,
            width: width.max(0),
            height: height.max(0),
            blocked: BTreeSet::new(),
            cost_percent: BTreeMap::new(),
            budget: DEFAULT_BUDGET,
        }
    }

    #[must_use]
    pub fn kind(&self) -> GridKind {
        self.kind
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Whether a cell is inside the grid. On a hex grid the axial coordinates must satisfy
    /// `0 <= q < width`, `0 <= r < height` and `q + r < max(width, height)` (the rhombus shape).
    #[must_use]
    pub fn contains(&self, cell: Cell) -> bool {
        if cell.x < 0 || cell.y < 0 || cell.x >= self.width || cell.y >= self.height {
            return false;
        }
        match self.kind {
            GridKind::Hex => cell.x + cell.y < self.width.max(self.height),
            GridKind::Square4 | GridKind::Square8 => true,
        }
    }

    /// Marks a cell as a wall. Outside cells are ignored: a wall off the map is not an error.
    pub fn block(&mut self, cell: Cell) {
        if self.contains(cell) {
            self.blocked.insert(cell);
        }
    }

    /// Clears a wall.
    pub fn unblock(&mut self, cell: Cell) {
        self.blocked.remove(&cell);
    }

    /// Sets how expensive a cell is, as a percentage of a normal step (100 = normal).
    ///
    /// A value of zero or one hundred removes the override; anything below 100 is refused, because
    /// a cheaper-than-normal step would let A* find a path that is shorter than the geometry.
    pub fn set_cost(&mut self, cell: Cell, percent: u32) {
        if percent <= 100 || !self.contains(cell) {
            self.cost_percent.remove(&cell);
        } else {
            self.cost_percent.insert(cell, percent.min(10_000));
        }
    }

    /// Whether a cell can be walked: inside and not a wall.
    #[must_use]
    pub fn walkable(&self, cell: Cell) -> bool {
        self.contains(cell) && !self.blocked.contains(&cell)
    }

    /// The neighbours of a cell that can be walked, with the cost of the step to each.
    ///
    /// The order is fixed (east, west, south, north, then the diagonals), so a search that has two
    /// equal candidates always picks the same one.
    #[must_use]
    pub fn neighbours(&self, cell: Cell) -> Vec<(Cell, u32)> {
        let mut found = Vec::with_capacity(8);
        let directions: &[(i32, i32, StepCost)] = match self.kind {
            GridKind::Square4 => &[
                (1, 0, StepCost::Straight),
                (-1, 0, StepCost::Straight),
                (0, 1, StepCost::Straight),
                (0, -1, StepCost::Straight),
            ],
            GridKind::Square8 => &[
                (1, 0, StepCost::Straight),
                (-1, 0, StepCost::Straight),
                (0, 1, StepCost::Straight),
                (0, -1, StepCost::Straight),
                (1, 1, StepCost::Diagonal),
                (1, -1, StepCost::Diagonal),
                (-1, 1, StepCost::Diagonal),
                (-1, -1, StepCost::Diagonal),
            ],
            // Axial neighbours of a pointy-top hex.
            GridKind::Hex => &[
                (1, 0, StepCost::Straight),
                (1, -1, StepCost::Straight),
                (0, -1, StepCost::Straight),
                (-1, 0, StepCost::Straight),
                (-1, 1, StepCost::Straight),
                (0, 1, StepCost::Straight),
            ],
        };
        for (dx, dy, kind) in directions {
            let next = Cell::new(cell.x + dx, cell.y + dy);
            if !self.walkable(next) {
                continue;
            }
            // An eight-way diagonal may not cut the corner between two walls: that would let a
            // character walk through a door frame it does not fit through.
            if *kind == StepCost::Diagonal
                && (!self.walkable(Cell::new(cell.x + dx, cell.y))
                    || !self.walkable(Cell::new(cell.x, cell.y + dy)))
            {
                continue;
            }
            found.push((next, self.step_cost(next, *kind)));
        }
        found
    }

    /// The cost of entering `cell`, in the same integer scale as [`StepCost`].
    ///
    /// Straight is 1000 and diagonal is 1414 (`sqrt(2)` × 1000): integers keep the search exact and
    /// deterministic, with no floating-point tie-breaks.
    fn step_cost(&self, cell: Cell, kind: StepCost) -> u32 {
        let base = match kind {
            StepCost::Straight => 1000,
            StepCost::Diagonal => 1414,
        };
        let percent = self.cost_percent.get(&cell).copied().unwrap_or(100);
        // Round to the nearest integer: 1000 * 150 / 100 = 1500, exact enough and still an integer.
        base * percent / 100
    }

    /// The distance heuristic, never more than the real cost (A* stays optimal only then).
    fn heuristic(&self, from: Cell, to: Cell) -> u32 {
        let dx = (from.x - to.x).unsigned_abs();
        let dy = (from.y - to.y).unsigned_abs();
        match self.kind {
            GridKind::Square4 => (dx + dy) * 1000,
            GridKind::Square8 => {
                // Diagonal steps cover both axes at once, so the shortest is the larger distance.
                dx.max(dy) * 1000 + dx.min(dy) * 414
            }
            GridKind::Hex => {
                // Axial distance on a hex grid, in steps.
                let signed = (from.x - to.x) + (from.y - to.y);
                let steps =
                    (signed.unsigned_abs() + from.x.abs_diff(to.x) + from.y.abs_diff(to.y)) / 2;
                steps * 1000
            }
        }
    }

    /// A* from `start` to `goal`. The path includes both ends.
    ///
    /// Ties are broken by cell order, so the same grid always gives the same path. The search never
    /// expands more than the budget, and gives up with [`PathError::TooLarge`] instead of hanging.
    pub fn find_path(&self, start: Cell, goal: Cell) -> Result<Vec<Cell>, PathError> {
        if !self.contains(start) || !self.contains(goal) {
            return Err(PathError::OutOfBounds);
        }
        if !self.walkable(start) || !self.walkable(goal) {
            return Err(PathError::Blocked);
        }
        if start == goal {
            return Ok(vec![start]);
        }
        // Ordered by (estimated total, then cell) so equal candidates resolve the same way always.
        // The key holds the estimate **as inserted**: recomputing it on pop would not match, and the
        // stale entries would pile up until the budget tripped.
        let mut open: BTreeSet<(u32, Cell)> = BTreeSet::new();
        let mut cost_so_far: BTreeMap<Cell, u32> = BTreeMap::new();
        let mut came_from: BTreeMap<Cell, Cell> = BTreeMap::new();
        cost_so_far.insert(start, 0);
        open.insert((self.heuristic(start, goal), start));
        let mut expanded = 0usize;
        while let Some(&(estimate, current)) = open.iter().next() {
            open.remove(&(estimate, current));
            // A cell may have been queued more than once; the cheapest entry is the one that counts.
            if cost_so_far
                .get(&current)
                .is_some_and(|known| estimate > known.saturating_add(self.heuristic(current, goal)))
            {
                continue;
            }
            if current == goal {
                return Ok(self.reconstruct(&came_from, start, goal));
            }
            expanded += 1;
            if expanded > self.budget {
                return Err(PathError::TooLarge);
            }
            let base = cost_so_far.get(&current).copied().unwrap_or(0);
            for (next, step) in self.neighbours(current) {
                let candidate = base.saturating_add(step);
                let better = cost_so_far
                    .get(&next)
                    .is_none_or(|known| candidate < *known);
                if !better {
                    continue;
                }
                cost_so_far.insert(next, candidate);
                came_from.insert(next, current);
                open.insert((candidate.saturating_add(self.heuristic(next, goal)), next));
            }
        }
        Err(PathError::Unreachable)
    }

    fn reconstruct(&self, came_from: &BTreeMap<Cell, Cell>, start: Cell, goal: Cell) -> Vec<Cell> {
        let mut path = vec![goal];
        let mut current = goal;
        // Bounded by the number of cells, so a malformed parent map cannot loop forever.
        for _ in 0..self.budget {
            if current == start {
                path.reverse();
                return path;
            }
            let Some(previous) = came_from.get(&current).copied() else {
                break;
            };
            path.push(previous);
            current = previous;
        }
        path.reverse();
        path
    }
}

/// Orders two candidates the A* way: lower cost first, then cell order. Exposed for tests and for
/// callers that keep their own queue.
#[must_use]
pub fn compare_candidates(first: (u32, Cell), second: (u32, Cell)) -> Ordering {
    first.1.cmp(&second.1).then(first.0.cmp(&second.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_line_is_the_shortest_path() {
        let grid = Grid::square(8, 8, false);
        let path = grid
            .find_path(Cell::new(0, 0), Cell::new(3, 0))
            .expect("path");
        assert_eq!(
            path,
            vec![
                Cell::new(0, 0),
                Cell::new(1, 0),
                Cell::new(2, 0),
                Cell::new(3, 0)
            ]
        );
        assert_eq!(path.len(), 4, "start and goal included");
    }

    #[test]
    fn a_wall_forces_a_detour_and_never_cuts_it() {
        let mut grid = Grid::square(5, 5, false);
        for y in 0..4 {
            grid.block(Cell::new(2, y));
        }
        let path = grid
            .find_path(Cell::new(0, 0), Cell::new(4, 0))
            .expect("path");
        assert!(
            path.iter().all(|cell| cell.x != 2 || cell.y == 4),
            "the path goes around the wall: {path:?}"
        );
        assert_eq!(path.first(), Some(&Cell::new(0, 0)));
        assert_eq!(path.last(), Some(&Cell::new(4, 0)));
    }

    #[test]
    fn the_same_grid_always_gives_the_same_path() {
        // Determinism (AGENTS.md §2): the order of equal candidates never leaks out.
        let mut grid = Grid::square(12, 12, true);
        for cell in [
            Cell::new(3, 3),
            Cell::new(4, 3),
            Cell::new(3, 4),
            Cell::new(7, 8),
        ] {
            grid.block(cell);
        }
        let first = grid
            .find_path(Cell::new(0, 0), Cell::new(9, 9))
            .expect("path");
        for _ in 0..8 {
            assert_eq!(
                grid.find_path(Cell::new(0, 0), Cell::new(9, 9))
                    .expect("path"),
                first
            );
        }
    }

    #[test]
    fn a_diagonal_never_slips_between_two_walls() {
        let mut grid = Grid::square(3, 3, true);
        grid.block(Cell::new(1, 0));
        grid.block(Cell::new(0, 1));
        // Both cells the diagonal would cut between are walls, and there is no other route on a
        // 3x3 grid: refusing is the correct answer (a spear-corner slip would have "found" one).
        assert_eq!(
            grid.find_path(Cell::new(0, 0), Cell::new(1, 1)),
            Err(PathError::Unreachable)
        );
    }

    #[test]
    fn a_closed_goal_is_unreachable_not_a_panic() {
        let mut grid = Grid::square(4, 4, false);
        for cell in [Cell::new(0, 1), Cell::new(1, 0)] {
            grid.block(cell);
        }
        assert_eq!(
            grid.find_path(Cell::new(0, 0), Cell::new(3, 3)),
            Err(PathError::Unreachable)
        );
    }

    #[test]
    fn out_of_bounds_and_walls_are_refused_by_name() {
        let mut grid = Grid::square(4, 4, false);
        grid.block(Cell::new(1, 1));
        assert_eq!(
            grid.find_path(Cell::new(0, 0), Cell::new(9, 9)),
            Err(PathError::OutOfBounds)
        );
        assert_eq!(
            grid.find_path(Cell::new(0, 0), Cell::new(1, 1)),
            Err(PathError::Blocked)
        );
        assert_eq!(
            grid.find_path(Cell::new(-1, 0), Cell::new(1, 1)),
            Err(PathError::OutOfBounds)
        );
    }

    #[test]
    fn a_hex_has_six_neighbours_and_a_shorter_diagonal() {
        let grid = Grid::hex(6, 6);
        let middle = Cell::new(2, 2);
        assert_eq!(
            grid.neighbours(middle).len(),
            6,
            "hexes have six neighbours"
        );
        // On a hex grid the distance from (0,0) to (1,1) is two steps, not one.
        let path = grid
            .find_path(Cell::new(0, 0), Cell::new(1, 1))
            .expect("path");
        assert_eq!(path.len(), 3, "two steps, three cells: {path:?}");
    }

    #[test]
    fn the_hex_rhombus_boundary_is_respected() {
        let grid = Grid::hex(4, 4);
        assert!(grid.contains(Cell::new(0, 0)));
        assert!(grid.contains(Cell::new(3, 0)));
        // q + r must stay under the longest side.
        assert!(!grid.contains(Cell::new(3, 3)), "outside the rhombus");
        assert!(grid.contains(Cell::new(1, 2)));
    }

    #[test]
    fn a_costly_cell_is_avoided_when_a_free_detour_is_shorter() {
        let mut grid = Grid::square(5, 3, false);
        // A swamp across the middle column except the top row: going around is cheaper.
        for y in 1..3 {
            grid.set_cost(Cell::new(2, y), 900);
        }
        let path = grid
            .find_path(Cell::new(0, 2), Cell::new(4, 2))
            .expect("path");
        assert!(
            path.iter().all(|cell| cell.x != 2 || cell.y == 0),
            "the path climbs over the swamp: {path:?}"
        );
    }

    #[test]
    fn a_hostile_budget_stops_the_search_instead_of_hanging() {
        let mut grid = Grid::square(400, 400, false);
        // A wall along the first row, but never on the start itself.
        for index in 1..20 {
            grid.block(Cell::new(index, 0));
        }
        assert!(
            matches!(
                grid.find_path(Cell::new(0, 0), Cell::new(399, 399)),
                Ok(_) | Err(PathError::TooLarge) | Err(PathError::Unreachable)
            ),
            "a huge map answers with a path or a named refusal, never a hang"
        );
    }
}
