//! Square and hexagonal grids: whole-number cells, exact distances, neighbors, ranges and A*.
//!
//! Coordinates are whole numbers: `q` and `r` are axial coordinates. A square grid uses them as
//! its two axes; movement is orthogonal (four neighbors) and a diagonal costs two steps. A
//! hexagonal grid is pointy-topped, with the third cube coordinate `s = -q - r`; movement is six
//! neighbors and the distance is `(|Δq| + |Δr| + |Δs|) / 2`.
//!
//! A [`Grid`] is plain data (kind and cell size) and validates like any other hostile input. Every
//! function is deterministic: the same inputs give the same cells, in the same order. Pathfinding
//! is A* with the exact grid distance as the heuristic, so the first path to a cell is the
//! shortest; ties are broken by the heuristic and then by the cell itself.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

use petunia_math::Vec3;

#[cfg(test)]
mod tests;

/// Cells live inside ±this many steps on each axis; anything beyond is a broken file or a typo.
pub const MAX_CELL_COORDINATE: i32 = 1_000_000;
/// Smallest cell side, in meters (thinner cells lose precision in the world conversion).
pub const MIN_CELL_SIZE: f32 = 0.001;
/// Largest cell side, in meters.
pub const MAX_CELL_SIZE: f32 = 10_000.0;
/// Largest radius of one [`Grid::range`] query.
pub const MAX_RANGE_RADIUS: u32 = 256;
/// Most cells one [`Grid::range`] query may return.
pub const MAX_RANGE_CELLS: usize = 200_000;
/// Most blocked cells one [`Grid::path`] call accepts.
pub const MAX_BLOCKED_CELLS: usize = 65_536;
/// Most cells A* may expand before giving up on a path.
pub const MAX_PATH_CELLS: usize = 65_536;

const SQRT_3: f32 = 1.732_050_8;

/// A cell: axial coordinates. For a square grid they are the two axes; for a hex grid they are
/// the axial pair of the cube coordinate `(q, r, s)`, with `s = -q - r`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell {
    pub q: i32,
    pub r: i32,
}

impl Cell {
    pub const ORIGIN: Self = Self { q: 0, r: 0 };

    /// A cell inside the accepted coordinate range.
    pub fn new(q: i32, r: i32) -> Result<Self, GridError> {
        Self { q, r }.check()
    }

    fn check(self) -> Result<Self, GridError> {
        if self.q < -MAX_CELL_COORDINATE
            || self.q > MAX_CELL_COORDINATE
            || self.r < -MAX_CELL_COORDINATE
            || self.r > MAX_CELL_COORDINATE
        {
            return Err(GridError::CellOutOfRange {
                q: self.q,
                r: self.r,
            });
        }
        Ok(self)
    }
}

/// The two grid shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKind {
    Square,
    Hex,
}

impl GridKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Hex => "hex",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "square" => Some(Self::Square),
            "hex" => Some(Self::Hex),
            _ => None,
        }
    }
}

/// Why a grid value, a cell or a query was refused.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum GridError {
    #[error("o tamanho da célula precisa ser finito, positivo e não gigante; veio {0}")]
    InvalidSize(f32),
    #[error("a célula ({q}, {r}) está fora do alcance aceito")]
    CellOutOfRange { q: i32, r: i32 },
    #[error("o raio {0} é grande demais para uma consulta")]
    RadiusOutOfRange(u32),
    #[error("o alcance passa de {MAX_RANGE_CELLS} células")]
    RangeTooLarge,
    #[error("células bloqueadas demais: {0}")]
    TooManyBlocked(usize),
    #[error("a busca cresceu demais; aproxime os pontos ou divida o mapa")]
    SearchTooLarge,
    #[error("o ponto precisa ter x e z finitos")]
    NonFinitePoint,
}

/// A grid: its shape and the size of one cell, in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    kind: GridKind,
    size: f32,
}

impl Grid {
    pub fn square(size: f32) -> Result<Self, GridError> {
        Self::from_parts(GridKind::Square, size)
    }

    pub fn hex(size: f32) -> Result<Self, GridError> {
        Self::from_parts(GridKind::Hex, size)
    }

    pub fn from_parts(kind: GridKind, size: f32) -> Result<Self, GridError> {
        if !size.is_finite() || !(MIN_CELL_SIZE..=MAX_CELL_SIZE).contains(&size) {
            return Err(GridError::InvalidSize(size));
        }
        Ok(Self { kind, size })
    }

    pub const fn kind(self) -> GridKind {
        self.kind
    }

    pub const fn size(self) -> f32 {
        self.size
    }

    /// Steps between two cells: orthogonal on a square grid, the cube formula on a hex grid.
    pub fn distance(self, a: Cell, b: Cell) -> Result<i32, GridError> {
        a.check()?;
        b.check()?;
        let dq = i64::from(a.q) - i64::from(b.q);
        let dr = i64::from(a.r) - i64::from(b.r);
        let distance = match self.kind {
            GridKind::Square => dq.abs() + dr.abs(),
            GridKind::Hex => {
                let ds = -(dq + dr);
                (dq.abs() + dr.abs() + ds.abs()) / 2
            }
        };
        Ok(distance.min(i64::from(i32::MAX)) as i32)
    }

    /// The cells that share an edge with `cell`: four on a square grid, six on a hex grid.
    /// Neighbors beyond the coordinate limit are left out.
    pub fn neighbors(self, cell: Cell) -> Result<Vec<Cell>, GridError> {
        cell.check()?;
        let offsets: &[(i32, i32)] = match self.kind {
            GridKind::Square => &[(1, 0), (-1, 0), (0, 1), (0, -1)],
            GridKind::Hex => &[(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)],
        };
        let mut cells = Vec::with_capacity(offsets.len());
        for (dq, dr) in offsets {
            let neighbor = Cell {
                q: cell.q + dq,
                r: cell.r + dr,
            };
            if neighbor.check().is_ok() {
                cells.push(neighbor);
            }
        }
        Ok(cells)
    }

    /// Every cell up to `radius` steps away, the cell itself included, sorted by `r` and then
    /// `q`. Cells beyond the coordinate limit are left out.
    pub fn range(self, cell: Cell, radius: u32) -> Result<Vec<Cell>, GridError> {
        cell.check()?;
        if radius > MAX_RANGE_RADIUS {
            return Err(GridError::RadiusOutOfRange(radius));
        }
        let radius = radius as i32;
        let mut cells = Vec::new();
        match self.kind {
            GridKind::Square => {
                for dr in -radius..=radius {
                    for dq in -radius..=radius {
                        if dq.abs() + dr.abs() <= radius {
                            self.push_if_inside(cell, dq, dr, &mut cells);
                        }
                    }
                }
            }
            GridKind::Hex => {
                for dq in -radius..=radius {
                    let lowest = (-radius).max(-dq - radius);
                    let highest = radius.min(-dq + radius);
                    for dr in lowest..=highest {
                        self.push_if_inside(cell, dq, dr, &mut cells);
                    }
                }
            }
        }
        if cells.len() > MAX_RANGE_CELLS {
            return Err(GridError::RangeTooLarge);
        }
        cells.sort_unstable_by_key(|cell| (cell.r, cell.q));
        Ok(cells)
    }

    fn push_if_inside(self, origin: Cell, dq: i32, dr: i32, cells: &mut Vec<Cell>) {
        let cell = Cell {
            q: origin.q + dq,
            r: origin.r + dr,
        };
        if cell.check().is_ok() {
            cells.push(cell);
        }
    }

    /// Shortest path from `from` to `to` through cells that are not blocked, `from` and `to`
    /// included; `None` when an endpoint is blocked or no path exists.
    pub fn path(
        self,
        from: Cell,
        to: Cell,
        blocked: &BTreeSet<Cell>,
    ) -> Result<Option<Vec<Cell>>, GridError> {
        from.check()?;
        to.check()?;
        if blocked.len() > MAX_BLOCKED_CELLS {
            return Err(GridError::TooManyBlocked(blocked.len()));
        }
        for cell in blocked {
            cell.check()?;
        }
        if blocked.contains(&from) || blocked.contains(&to) {
            return Ok(None);
        }
        if from == to {
            return Ok(Some(vec![from]));
        }
        let start = self.distance(from, to)? as u32;
        let mut open: BinaryHeap<Reverse<(u32, u32, i32, i32)>> = BinaryHeap::new();
        let mut best: HashMap<Cell, u32> = HashMap::new();
        let mut came_from: HashMap<Cell, Cell> = HashMap::new();
        best.insert(from, 0);
        open.push(Reverse((start, start, from.q, from.r)));
        let mut expansions = 0usize;
        while let Some(Reverse((cost, remaining, q, r))) = open.pop() {
            let cell = Cell { q, r };
            let Some(&distance) = best.get(&cell) else {
                continue;
            };
            // A stale entry: a shorter way to this cell was found after it was pushed.
            if distance + remaining != cost {
                continue;
            }
            if cell == to {
                return Ok(Some(reconstruct(from, to, &came_from)));
            }
            expansions += 1;
            if expansions > MAX_PATH_CELLS {
                return Err(GridError::SearchTooLarge);
            }
            for neighbor in self.neighbors(cell)? {
                if blocked.contains(&neighbor) {
                    continue;
                }
                let next = distance + 1;
                let known = best.get(&neighbor).copied().unwrap_or(u32::MAX);
                if next < known {
                    best.insert(neighbor, next);
                    came_from.insert(neighbor, cell);
                    let remaining = self.distance(neighbor, to)? as u32;
                    open.push(Reverse((
                        next + remaining,
                        remaining,
                        neighbor.q,
                        neighbor.r,
                    )));
                }
            }
        }
        Ok(None)
    }

    /// The center of `cell` in the world, at ground level (`y = 0`), in meters.
    pub fn to_world(self, cell: Cell) -> Result<Vec3, GridError> {
        cell.check()?;
        let q = cell.q as f32;
        let r = cell.r as f32;
        Ok(match self.kind {
            GridKind::Square => Vec3::new(self.size * q, 0.0, self.size * r),
            GridKind::Hex => {
                Vec3::new(self.size * SQRT_3 * (q + r / 2.0), 0.0, self.size * 1.5 * r)
            }
        })
    }

    /// The cell under `point` (its `x` and `z`; `y` is ignored).
    pub fn from_world(self, point: Vec3) -> Result<Cell, GridError> {
        if !point.x.is_finite() || !point.z.is_finite() {
            return Err(GridError::NonFinitePoint);
        }
        let cell = match self.kind {
            GridKind::Square => Cell {
                q: (point.x / self.size).round() as i32,
                r: (point.z / self.size).round() as i32,
            },
            GridKind::Hex => {
                let q = (SQRT_3 / 3.0 * point.x - point.z / 3.0) / self.size;
                let r = (2.0 / 3.0 * point.z) / self.size;
                round_hex(q, r)
            }
        };
        cell.check()
    }
}

/// Cube rounding: the fractional axial pair becomes the nearest cell of the hex grid.
fn round_hex(q: f32, r: f32) -> Cell {
    let s = -q - r;
    let mut rounded_q = q.round();
    let mut rounded_r = r.round();
    let rounded_s = s.round();
    let error_q = (rounded_q - q).abs();
    let error_r = (rounded_r - r).abs();
    let error_s = (rounded_s - s).abs();
    if error_q > error_r && error_q > error_s {
        rounded_q = -rounded_r - rounded_s;
    } else if error_r > error_s {
        rounded_r = -rounded_q - rounded_s;
    }
    Cell {
        q: rounded_q as i32,
        r: rounded_r as i32,
    }
}

fn reconstruct(from: Cell, to: Cell, came_from: &HashMap<Cell, Cell>) -> Vec<Cell> {
    let mut path = vec![to];
    let mut current = to;
    while current != from {
        let Some(parent) = came_from.get(&current).copied() else {
            break;
        };
        current = parent;
        path.push(current);
    }
    path.reverse();
    path
}
