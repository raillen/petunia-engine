// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use petunia_math::Vec3;

use super::*;

fn square() -> Grid {
    Grid::square(1.0).unwrap()
}

fn hex() -> Grid {
    Grid::hex(1.0).unwrap()
}

fn cells(pairs: &[(i32, i32)]) -> BTreeSet<Cell> {
    pairs.iter().map(|&(q, r)| Cell { q, r }).collect()
}

fn assert_continuous(grid: &Grid, path: &[Cell]) {
    for pair in path.windows(2) {
        assert_eq!(grid.distance(pair[0], pair[1]).unwrap(), 1);
    }
}

#[test]
fn a_square_grid_counts_orthogonal_steps() {
    let grid = square();
    let a = Cell::ORIGIN;
    let b = Cell { q: 3, r: -2 };
    assert_eq!(grid.distance(a, b).unwrap(), 5);
    assert_eq!(grid.distance(b, a).unwrap(), 5);
    assert_eq!(grid.neighbors(a).unwrap().len(), 4);
    assert!(grid.neighbors(a).unwrap().contains(&Cell { q: 1, r: 0 }));

    let reach = grid.range(a, 2).unwrap();
    assert_eq!(reach.len(), 13);
    assert!(reach.contains(&a));
    assert!(
        reach
            .windows(2)
            .all(|pair| (pair[0].r, pair[0].q) <= (pair[1].r, pair[1].q))
    );
}

#[test]
fn a_hex_grid_uses_cube_distance_and_six_neighbors() {
    let grid = hex();
    let a = Cell::ORIGIN;
    assert_eq!(grid.neighbors(a).unwrap().len(), 6);
    assert_eq!(grid.distance(a, Cell { q: 1, r: 0 }).unwrap(), 1);
    assert_eq!(grid.distance(a, Cell { q: 1, r: 1 }).unwrap(), 2);
    assert_eq!(grid.distance(a, Cell { q: -2, r: 1 }).unwrap(), 2);

    // 3 * radius * (radius + 1) + 1 cells, the classic hex range.
    assert_eq!(grid.range(a, 2).unwrap().len(), 19);
    assert_eq!(grid.range(a, 5).unwrap().len(), 91);
}

#[test]
fn worlds_and_cells_round_trip() {
    let square_grid = Grid::square(0.5).unwrap();
    for q in -5..=5 {
        for r in -5..=5 {
            let cell = Cell { q, r };
            let point = square_grid.to_world(cell).unwrap();
            assert_eq!(point, Vec3::new(0.5 * q as f32, 0.0, 0.5 * r as f32));
            assert_eq!(square_grid.from_world(point).unwrap(), cell);
        }
    }

    let hex_grid = hex();
    let first = hex_grid.to_world(Cell { q: 1, r: 0 }).unwrap();
    assert!((first.x - SQRT_3).abs() < 1e-5);
    assert_eq!(first.z, 0.0);
    let second = hex_grid.to_world(Cell { q: 0, r: 1 }).unwrap();
    assert!((second.x - SQRT_3 / 2.0).abs() < 1e-5);
    assert!((second.z - 1.5).abs() < 1e-5);
    for q in -6..=6 {
        for r in -6..=6 {
            let cell = Cell { q, r };
            assert_eq!(
                hex_grid
                    .from_world(hex_grid.to_world(cell).unwrap())
                    .unwrap(),
                cell
            );
        }
    }
}

#[test]
fn the_shortest_path_is_short_continuous_and_deterministic() {
    let grid = square();
    let from = Cell { q: -3, r: 1 };
    let to = Cell { q: 4, r: 5 };
    let blocked = cells(&[(0, 1), (1, 1), (2, 1)]);
    let first = grid.path(from, to, &blocked).unwrap().unwrap();
    let second = grid.path(from, to, &blocked).unwrap().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.first(), Some(&from));
    assert_eq!(first.last(), Some(&to));
    assert_eq!(first.len(), 12, "eleven steps plus the start");
    assert_continuous(&grid, &first);
    assert!(first.iter().all(|cell| !blocked.contains(cell)));

    // A wall in the way makes the path longer than the straight line.
    let wall = cells(&(0..=6).map(|r| (2, r)).collect::<Vec<_>>());
    let around = grid.path(from, to, &wall).unwrap().unwrap();
    assert!(around.len() > 12);
    assert_continuous(&grid, &around);
    assert!(around.iter().all(|cell| !wall.contains(cell)));
}

#[test]
fn a_hex_path_costs_the_hex_distance() {
    let grid = hex();
    let from = Cell { q: 0, r: 0 };
    let to = Cell { q: 4, r: -1 };
    let path = grid.path(from, to, &BTreeSet::new()).unwrap().unwrap();
    assert_eq!(path.len() as i32, grid.distance(from, to).unwrap() + 1);
    assert_continuous(&grid, &path);
}

#[test]
fn blocked_endpoints_and_walls_give_no_path() {
    let grid = square();
    let from = Cell { q: 0, r: 0 };
    let to = Cell { q: 2, r: 0 };
    assert_eq!(
        grid.path(from, to, &cells(&[(0, 0)])).unwrap(),
        None,
        "start blocked"
    );
    assert_eq!(
        grid.path(from, to, &cells(&[(2, 0)])).unwrap(),
        None,
        "goal blocked"
    );
    let closed = cells(&[(1, 0), (-1, 0), (0, 1), (0, -1)]);
    assert_eq!(grid.path(from, from, &closed).unwrap(), Some(vec![from]));
    assert_eq!(
        grid.path(Cell { q: 3, r: 3 }, Cell { q: 3, r: 3 }, &BTreeSet::new())
            .unwrap(),
        Some(vec![Cell { q: 3, r: 3 }])
    );
}

#[test]
fn hostile_values_are_refused_without_panicking() {
    for size in [f32::NAN, f32::INFINITY, 0.0, -1.0, 0.000_1, 20_000.0] {
        assert!(matches!(Grid::square(size), Err(GridError::InvalidSize(_))));
    }
    assert!(matches!(
        Cell::new(MAX_CELL_COORDINATE + 1, 0),
        Err(GridError::CellOutOfRange { .. })
    ));
    let grid = square();
    let out_of_range = Cell {
        q: i32::MAX,
        r: i32::MIN,
    };
    assert!(grid.distance(out_of_range, Cell::ORIGIN).is_err());
    assert!(grid.to_world(out_of_range).is_err());
    assert!(grid.range(Cell::ORIGIN, MAX_RANGE_RADIUS + 1).is_err());
    assert!(grid.from_world(Vec3::new(f32::NAN, 0.0, 0.0)).is_err());
    assert!(grid.from_world(Vec3::new(1e30, 0.0, 0.0)).is_err());
    let border = Cell {
        q: MAX_CELL_COORDINATE,
        r: 0,
    };
    assert_eq!(grid.neighbors(border).unwrap().len(), 3);

    let too_many: BTreeSet<Cell> = (0..=MAX_BLOCKED_CELLS as i32)
        .map(|q| Cell { q, r: 0 })
        .collect();
    assert!(matches!(
        grid.path(Cell::ORIGIN, Cell { q: 1, r: 1 }, &too_many),
        Err(GridError::TooManyBlocked(_))
    ));
}

#[test]
fn an_endless_search_stops_at_the_budget() {
    let grid = square();
    let to = Cell { q: 400, r: 0 };
    // The goal's four neighbors are blocked: no path exists, and the search would walk the whole
    // diamond around the start.
    let blocked = cells(&[(401, 0), (399, 0), (400, 1), (400, -1)]);
    assert!(matches!(
        grid.path(Cell::ORIGIN, to, &blocked),
        Err(GridError::SearchTooLarge)
    ));
}
