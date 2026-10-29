use petunia_schema::check_params;
use petunia_world::{PersistentId, World};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::builtin::query_specs;
use crate::error::CommandError;

/// A query as data. Queries only read and return copies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryRecord {
    pub query: String,
    #[serde(default)]
    pub payload: Json,
}

impl QueryRecord {
    pub fn new(query: &str, payload: Json) -> Self {
        Self {
            query: query.to_string(),
            payload,
        }
    }
}

/// Runs a read-only query against the world.
pub fn run_query(world: &World, record: &QueryRecord) -> Result<Json, CommandError> {
    let specs = query_specs();
    let spec = specs
        .iter()
        .find(|spec| spec.id == record.query)
        .ok_or_else(|| CommandError::UnknownCommand {
            name: record.query.clone(),
            suggestions: specs.iter().map(|spec| spec.id.clone()).take(3).collect(),
        })?;
    check_params(&spec.params, &record.payload).map_err(|message| {
        CommandError::InvalidPayload {
            command: record.query.clone(),
            message,
        }
    })?;
    match record.query.as_str() {
        "world.get" => query_get(world, &record.payload),
        "world.find" => query_find(world, &record.payload),
        "terrain.sample" => query_terrain_sample(world, &record.payload),
        "path.report" => query_path_report(world, &record.payload),
        other => Err(CommandError::UnknownCommand {
            name: other.to_string(),
            suggestions: Vec::new(),
        }),
    }
}

fn text<'a>(payload: &'a Json, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Json::as_str)
}

fn query_get(world: &World, payload: &Json) -> Result<Json, CommandError> {
    let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
    let handle = world.handle_of(PersistentId(entity))?;
    let table = text(payload, "table").unwrap_or_default();
    match text(payload, "column") {
        Some(column) => Ok(world.get(handle, table, column)?.to_json()),
        None => {
            let row = world.row(handle, table)?;
            Ok(Json::Object(world.row_to_json(table, row)?))
        }
    }
}

fn query_terrain_sample(world: &World, payload: &Json) -> Result<Json, CommandError> {
    use petunia_world::Value;
    let terrain = payload.get("terrain").and_then(Json::as_u64).unwrap_or(0);
    let x = payload.get("x").and_then(Json::as_f64).unwrap_or(f64::NAN) as f32;
    let z = payload.get("z").and_then(Json::as_f64).unwrap_or(f64::NAN) as f32;
    if !x.is_finite() || !z.is_finite() {
        return Err(CommandError::InvalidPayload {
            command: "terrain.sample".to_string(),
            message: "`x` e `z` precisam ser finitos".to_string(),
        });
    }
    for handle in world.entities_with_table("TerrainChunk")? {
        let owner = match world.get(handle, "TerrainChunk", "owner") {
            Ok(Value::Int(owner)) => *owner,
            _ => continue,
        };
        if owner != terrain as i64 {
            continue;
        }
        let chunk_x = match world.get(handle, "TerrainChunk", "chunk_x") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let chunk_z = match world.get(handle, "TerrainChunk", "chunk_z") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let size = match world.get(handle, "TerrainChunk", "size") {
            Ok(Value::Int(value)) => *value as usize,
            _ => continue,
        };
        let cell_size = match world.get(handle, "TerrainChunk", "cell_size") {
            Ok(Value::Float(value)) => *value as f32,
            _ => continue,
        };
        let Ok(Value::Floats(heights)) = world.get(handle, "TerrainChunk", "heights") else {
            continue;
        };
        if size < 2 || heights.len() != size * size || cell_size <= 0.0 {
            continue;
        }
        let origin_x = chunk_x as f32 * size as f32 * cell_size;
        let origin_z = chunk_z as f32 * size as f32 * cell_size;
        let span = size as f32 * cell_size;
        if x < origin_x || x >= origin_x + span || z < origin_z || z >= origin_z + span {
            continue;
        }
        let local_x = ((x - origin_x) / cell_size).clamp(0.0, (size - 1) as f32);
        let local_z = ((z - origin_z) / cell_size).clamp(0.0, (size - 1) as f32);
        let x0 = local_x.floor() as usize;
        let z0 = local_z.floor() as usize;
        let x1 = (x0 + 1).min(size - 1);
        let z1 = (z0 + 1).min(size - 1);
        let blend_x = local_x - x0 as f32;
        let blend_z = local_z - z0 as f32;
        let height = |ix: usize, iz: usize| heights.get(iz * size + ix).copied().unwrap_or(0.0);
        let first = height(x0, z0) * (1.0 - blend_x) + height(x1, z0) * blend_x;
        let second = height(x0, z1) * (1.0 - blend_x) + height(x1, z1) * blend_x;
        let sampled = first * (1.0 - blend_z) + second * blend_z;
        return Ok(json!({
            "height": sampled,
            "chunk": { "x": chunk_x, "z": chunk_z },
        }));
    }
    Ok(Json::Null)
}

fn query_path_report(world: &World, payload: &Json) -> Result<Json, CommandError> {
    use petunia_world::Value;
    let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
    let handle = world.handle_of(PersistentId(entity))?;
    let (kind, spline) = if let Ok(Value::Spline(spline)) = world.get(handle, "River", "spline") {
        ("river", spline)
    } else if let Ok(Value::Spline(spline)) = world.get(handle, "Road", "spline") {
        ("road", spline)
    } else {
        return Err(CommandError::InvalidPayload {
            command: "path.report".to_string(),
            message: "a entidade não é um rio nem uma estrada".to_string(),
        });
    };
    let segments = spline.segments();
    let samples = 64usize;
    let mut previous = spline.evaluate_position(0.0);
    let mut length = 0.0f64;
    let mut maximum_ascent = 0.0f32;
    for index in 1..=samples {
        let parameter = if segments == 0 {
            0.0
        } else {
            (index as f32 / samples as f32) * segments as f32
        };
        let Some(position) = spline.evaluate_position(parameter) else {
            continue;
        };
        if let Some(previous) = previous {
            length += f64::from((position - previous).length());
            maximum_ascent = maximum_ascent.max(position.y - previous.y);
        }
        previous = Some(position);
    }
    Ok(json!({
        "kind": kind,
        "points": spline.points.len(),
        "closed": spline.closed,
        "length_meters": length,
        "maximum_ascent": maximum_ascent,
        "descends": maximum_ascent <= 1.0e-4,
    }))
}

fn query_find(world: &World, payload: &Json) -> Result<Json, CommandError> {
    let table = text(payload, "table");
    let tag = text(payload, "tag");
    if table.is_none() && tag.is_none() {
        return Err(CommandError::InvalidPayload {
            command: "world.find".to_string(),
            message: "informe `table`, `tag` ou os dois".to_string(),
        });
    }
    let mut found = match table {
        Some(table) => world.entities_with_table(table)?,
        None => world.all_entities(),
    };
    if let Some(tag) = tag {
        let tagged = world.entities_with_tag(tag);
        found.retain(|handle| tagged.contains(handle));
    }
    let ids: Vec<u64> = found
        .into_iter()
        .filter_map(|handle| world.persistent_id(handle).ok())
        .map(|id| id.0)
        .collect();
    Ok(json!(ids))
}
