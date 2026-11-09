//! The `grid.*` natives. A grid is plain data (`{"kind": "square"|"hex", "size": Float}`) and a
//! cell is `{"q": Int, "r": Int}`; both can live in a table, cross a script boundary or be
//! printed without any hidden registry. The math lives in `petunia_grid`; nothing here touches
//! the world.

use std::collections::BTreeSet;

use aipo_vm::{Value, Vm, VmError};
use petunia_grid::{Cell, Grid, GridKind};
use petunia_math::Vec3;
use serde_json::{Value as Json, json};

use crate::convert::{json_to_value, value_to_json};
use crate::natives::{Native, failure, float_arg, internal, vec3_to_value, wrong_type};

fn argument(args: &[Value], index: usize) -> Result<&Value, VmError> {
    args.get(index)
        .ok_or_else(|| internal("faltou o argumento"))
}

fn argument_json(value: &Value, expected: &str, operation: &str) -> Result<Json, VmError> {
    value_to_json(value).map_err(|_| wrong_type(&format!("{expected} em {operation}"), value))
}

fn grid_arg(args: &[Value], index: usize, operation: &str) -> Result<Grid, VmError> {
    let value = argument(args, index)?;
    let json = argument_json(value, "Dict (grade)", operation)?;
    let Some(kind) = json
        .get("kind")
        .and_then(Json::as_str)
        .and_then(GridKind::parse)
    else {
        return Err(wrong_type(
            &format!("Dict (grade com kind `square` ou `hex`) em {operation}"),
            value,
        ));
    };
    let Some(size) = json.get("size").and_then(Json::as_f64) else {
        return Err(wrong_type(
            &format!("Dict (grade com size) em {operation}"),
            value,
        ));
    };
    Grid::from_parts(kind, size as f32).map_err(|_| {
        wrong_type(
            &format!("Dict (grade com size entre 0,001 e 10000) em {operation}"),
            value,
        )
    })
}

fn cell_from_json(json: &Json) -> Option<Cell> {
    let q = json.get("q")?.as_i64()?;
    let r = json.get("r")?.as_i64()?;
    Some(Cell {
        q: i32::try_from(q).ok()?,
        r: i32::try_from(r).ok()?,
    })
}

fn cell_arg(args: &[Value], index: usize, operation: &str) -> Result<Cell, VmError> {
    let value = argument(args, index)?;
    let json = argument_json(value, "Dict {q, r}", operation)?;
    cell_from_json(&json).ok_or_else(|| wrong_type(&format!("Dict {{q, r}} em {operation}"), value))
}

fn point_arg(args: &[Value], index: usize, operation: &str) -> Result<Vec3, VmError> {
    let value = argument(args, index)?;
    let json = argument_json(value, "Dict {x, z}", operation)?;
    let coordinate = |axis: &str| json.get(axis).and_then(Json::as_f64);
    let (Some(x), Some(z)) = (coordinate("x"), coordinate("z")) else {
        return Err(wrong_type(&format!("Dict {{x, z}} em {operation}"), value));
    };
    let y = coordinate("y").unwrap_or(0.0);
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
        return Err(wrong_type(
            &format!("Dict {{x, z}} com números finitos em {operation}"),
            value,
        ));
    }
    Ok(Vec3::new(x as f32, y as f32, z as f32))
}

fn blocked_arg(args: &[Value], index: usize, operation: &str) -> Result<BTreeSet<Cell>, VmError> {
    let value = argument(args, index)?;
    let json = argument_json(value, "List (células bloqueadas)", operation)?;
    let Json::Array(items) = json else {
        return Err(wrong_type(
            &format!("List (células bloqueadas) em {operation}"),
            value,
        ));
    };
    let mut blocked = BTreeSet::new();
    for item in &items {
        let Some(cell) = cell_from_json(item) else {
            return Err(wrong_type(
                &format!("List de Dict {{q, r}} em {operation}"),
                value,
            ));
        };
        blocked.insert(cell);
    }
    Ok(blocked)
}

fn grid_value(grid: Grid) -> Value {
    json_to_value(&json!({ "kind": grid.kind().name(), "size": grid.size() }))
}

fn cell_value(cell: Cell) -> Value {
    json_to_value(&json!({ "q": cell.q, "r": cell.r }))
}

fn cells_value(cells: &[Cell]) -> Value {
    json_to_value(&Json::Array(
        cells
            .iter()
            .map(|cell| json!({ "q": cell.q, "r": cell.r }))
            .collect(),
    ))
}

pub(crate) fn grid_square(_vm: &mut Vm, args: &[Value]) -> Native {
    let size = float_arg(args, 0, "grid.square")?;
    match Grid::square(size) {
        Ok(grid) => Ok(grid_value(grid)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_hex(_vm: &mut Vm, args: &[Value]) -> Native {
    let size = float_arg(args, 0, "grid.hex")?;
    match Grid::hex(size) {
        Ok(grid) => Ok(grid_value(grid)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_distance(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.distance")?;
    let a = cell_arg(args, 1, "grid.distance")?;
    let b = cell_arg(args, 2, "grid.distance")?;
    match grid.distance(a, b) {
        Ok(distance) => Ok(Value::Int(i64::from(distance))),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_neighbors(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.neighbors")?;
    let cell = cell_arg(args, 1, "grid.neighbors")?;
    match grid.neighbors(cell) {
        Ok(neighbors) => Ok(cells_value(&neighbors)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_range(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.range")?;
    let cell = cell_arg(args, 1, "grid.range")?;
    let Some(Value::Int(radius)) = args.get(2) else {
        return Err(wrong_type(
            "Int (raio) em grid.range",
            args.get(2).unwrap_or(&Value::None),
        ));
    };
    let Ok(radius) = u32::try_from(*radius) else {
        return failure("o raio precisa ser zero ou positivo");
    };
    match grid.range(cell, radius) {
        Ok(cells) => Ok(cells_value(&cells)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_path(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.path")?;
    let from = cell_arg(args, 1, "grid.path")?;
    let to = cell_arg(args, 2, "grid.path")?;
    let blocked = blocked_arg(args, 3, "grid.path")?;
    match grid.path(from, to, &blocked) {
        Ok(Some(cells)) => Ok(cells_value(&cells)),
        Ok(None) => Ok(Value::None),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_to_world(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.to_world")?;
    let cell = cell_arg(args, 1, "grid.to_world")?;
    match grid.to_world(cell) {
        Ok(point) => Ok(vec3_to_value(point)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn grid_from_world(_vm: &mut Vm, args: &[Value]) -> Native {
    let grid = grid_arg(args, 0, "grid.from_world")?;
    let point = point_arg(args, 1, "grid.from_world")?;
    match grid.from_world(point) {
        Ok(cell) => Ok(cell_value(cell)),
        Err(error) => failure(error.to_string()),
    }
}
