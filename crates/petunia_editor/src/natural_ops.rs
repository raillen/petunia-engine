//! Natural-world commands: terrain chunks, rivers and roads as scene data.
//!
//! Every change is one undoable command. Terrain painting touches only the chunks that the brush
//! reaches; a missing chunk is created in the same step and removed again by undo.

use std::collections::BTreeMap;

use petunia_commands::{CommandError, CommandRecord, Outcome};
use petunia_math::{CubicBezierSpline, Vec3};
use petunia_schema::{CommandSpec, Cost, FailureSpec, ParamKind, ParamSpec, Reversibility, Status};
use petunia_world::{
    EntityHandle, HEIGHT_CHUNK_SIZE, HeightChunk, Heightfield, PersistentId, StampShape, Value,
    World, WorldError,
};
use serde::Deserialize;
use serde_json::{Map, Value as Json, json};

/// Most brush stamps a single terrain command accepts.
const MAX_STAMPS: usize = 512;
/// A terrain starts with this many meters around the origin (capped to a few chunks).
const DEFAULT_TERRAIN_SIZE: f32 = 128.0;
const MAX_TERRAIN_SIZE: f32 = 512.0;
const MIN_CELL_SIZE: f32 = 0.05;
const MAX_CELL_SIZE: f32 = 16.0;

type Apply = fn(&mut World, &Json) -> Result<Outcome, CommandError>;

pub(crate) fn commands() -> Vec<(CommandSpec, Apply)> {
    vec![
        (terrain_create_spec(), terrain_create),
        (terrain_paint_spec(), terrain_paint),
        (path_create_spec(), path_create),
        (path_set_spec(), path_set),
        (road_structures_spec(), road_detect_structures),
    ]
}

fn parse<T: serde::de::DeserializeOwned>(command: &str, payload: &Json) -> Result<T, CommandError> {
    serde_json::from_value(payload.clone()).map_err(|error| CommandError::InvalidPayload {
        command: command.to_string(),
        message: error.to_string(),
    })
}

fn resolve(world: &World, id: u64) -> Result<EntityHandle, CommandError> {
    Ok(world.handle_of(PersistentId(id))?)
}

fn finite_between(name: &str, value: f32, minimum: f32, maximum: f32) -> Result<f32, CommandError> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(value)
    } else {
        Err(CommandError::InvalidPayload {
            command: "natural".to_string(),
            message: format!("`{name}` precisa estar entre {minimum} e {maximum}"),
        })
    }
}

fn validate_spline(spline: &CubicBezierSpline) -> Result<(), CommandError> {
    if spline.is_valid() {
        Ok(())
    } else {
        Err(CommandError::InvalidPayload {
            command: "natural.path".to_string(),
            message: "a spline precisa de pelo menos dois pontos finitos".to_string(),
        })
    }
}

fn spawn_named(
    world: &mut World,
    name: &str,
    rows: Vec<(&str, Map<String, Json>)>,
) -> Result<u64, CommandError> {
    let handle = world.spawn()?;
    let filled = (|| -> Result<(), CommandError> {
        let mut name_values = Map::new();
        name_values.insert("text".to_string(), json!(name));
        let name_row = world.parse_row("Name", &name_values)?;
        world.insert_row_values(handle, "Name", name_row)?;
        for (table, values) in rows {
            let row = world.parse_row(table, &values)?;
            world.insert_row_values(handle, table, row)?;
        }
        Ok(())
    })();
    if let Err(error) = filled {
        let _ = world.despawn(handle);
        return Err(error);
    }
    Ok(world.persistent_id(handle)?.0)
}

fn map_from_json(value: Json) -> Map<String, Json> {
    match value {
        Json::Object(map) => map,
        _ => Map::new(),
    }
}

fn batch_inverse(records: Vec<Json>) -> Option<CommandRecord> {
    if records.is_empty() {
        None
    } else {
        Some(CommandRecord::new(
            "world.batch",
            json!({ "records": records }),
        ))
    }
}

// ---- terrain.create ---------------------------------------------------------------------------

fn terrain_create_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.create".to_string(),
        summary: "Cria um terreno plano com alguns blocos ao redor da origem.".to_string(),
        params: vec![
            ParamSpec::optional("name", ParamKind::Text, "Nome do terreno."),
            ParamSpec::optional(
                "cell_size",
                ParamKind::Float,
                "Distância entre amostras, em metros (0,05 a 16).",
            ),
            ParamSpec::optional(
                "size_meters",
                ParamKind::Float,
                "Lado da área inicial, em metros (até 512).",
            ),
        ],
        returns: Some(ParamKind::Entity),
        failures: vec![FailureSpec::new(
            "COMMAND_INVALID_PAYLOAD",
            "Confira `cell_size` e `size_meters`.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"cell_size": 1.0, "size_meters": 128}"#.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainCreatePayload {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    cell_size: Option<f32>,
    #[serde(default)]
    size_meters: Option<f32>,
}

fn terrain_create(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: TerrainCreatePayload = parse("terrain.create", payload)?;
    let cell_size = finite_between(
        "cell_size",
        request.cell_size.unwrap_or(1.0),
        MIN_CELL_SIZE,
        MAX_CELL_SIZE,
    )?;
    let size_meters = finite_between(
        "size_meters",
        request.size_meters.unwrap_or(DEFAULT_TERRAIN_SIZE),
        cell_size * 4.0,
        MAX_TERRAIN_SIZE,
    )?;
    let name = request.name.unwrap_or_else(|| "Terreno".to_string());
    let terrain_id = spawn_named(
        world,
        &name,
        vec![("Terrain", map_from_json(json!({ "cell_size": cell_size })))],
    )?;

    let span = HEIGHT_CHUNK_SIZE as f32 * cell_size;
    let half_chunks = ((size_meters * 0.5 / span).ceil() as i32).clamp(1, 8);
    let mut records = Vec::new();
    for chunk_z in -half_chunks..half_chunks {
        for chunk_x in -half_chunks..half_chunks {
            let chunk = HeightChunk::new([chunk_x, chunk_z], HEIGHT_CHUNK_SIZE, cell_size);
            let chunk_id = spawn_named(
                world,
                "Bloco de terreno",
                vec![(
                    "TerrainChunk",
                    map_from_json(json!({
                        "owner": terrain_id,
                        "chunk_x": chunk_x,
                        "chunk_z": chunk_z,
                        "size": chunk.size as i64,
                        "cell_size": cell_size,
                        "heights": Value::Floats(chunk.heights).to_json(),
                    })),
                )],
            )?;
            records.push(json!({
                "command": "world.remove",
                "payload": { "entity": chunk_id },
            }));
        }
    }
    records.push(json!({
        "command": "world.remove",
        "payload": { "entity": terrain_id },
    }));
    Ok(Outcome {
        result: json!({ "entity": terrain_id, "cell_size": cell_size }),
        inverse: batch_inverse(records),
    })
}

// ---- terrain.paint ----------------------------------------------------------------------------

fn terrain_paint_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.paint".to_string(),
        summary: "Aplica carimbos de pincel no terreno, criando blocos quando preciso.".to_string(),
        params: vec![
            ParamSpec::required("terrain", ParamKind::Entity, "Id do terreno."),
            ParamSpec::required("stamps", ParamKind::Any, "Lista de carimbos {center, radius, strength, mode (raise, lower, smooth, flatten, noise, erode, stamp), target_height?, stamp?, rotation?}."),
            ParamSpec::optional("falloff", ParamKind::Float, "Suavidade da borda (0 a 1)."),
        ],
        returns: Some(ParamKind::Int),
        failures: vec![
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
            FailureSpec::new("COMMAND_INVALID_PAYLOAD", "Confira os carimbos."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"terrain": 4, "stamps": [{"center": {"x": 0, "y": 0, "z": 0}, "radius": 4, "strength": 0.5, "mode": "raise"}, {"center": {"x": 12, "y": 0, "z": 0}, "radius": 6, "strength": 2, "mode": "stamp", "stamp": "crater"}]}"#.to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TerrainMode {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Noise,
    Erode,
    /// A named shape ([`StampShape`]) instead of a raised or lowered patch.
    Stamp,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainStamp {
    center: Vec3,
    radius: f32,
    strength: f32,
    mode: TerrainMode,
    #[serde(default)]
    target_height: Option<f32>,
    #[serde(default)]
    stamp: Option<StampShape>,
    /// Turn of a stamp around the vertical axis, in degrees. Ignored by the radial brushes.
    #[serde(default)]
    rotation: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainPaintPayload {
    terrain: u64,
    stamps: Vec<TerrainStamp>,
    #[serde(default)]
    falloff: Option<f32>,
}

struct ChunkWork {
    entity: Option<u64>,
    chunk: HeightChunk,
    original: Vec<f32>,
}

fn terrain_paint(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: TerrainPaintPayload = parse("terrain.paint", payload)?;
    if request.stamps.is_empty() || request.stamps.len() > MAX_STAMPS {
        return Err(CommandError::InvalidPayload {
            command: "terrain.paint".to_string(),
            message: format!("envie de 1 a {MAX_STAMPS} carimbos"),
        });
    }
    let falloff = finite_between("falloff", request.falloff.unwrap_or(0.5), 0.0, 1.0)?;
    let terrain_handle = resolve(world, request.terrain)?;
    let cell_size = match world.get(terrain_handle, "Terrain", "cell_size") {
        Ok(Value::Float(value)) => *value as f32,
        _ => {
            return Err(CommandError::InvalidPayload {
                command: "terrain.paint".to_string(),
                message: "a entidade não é um terreno".to_string(),
            });
        }
    };
    if !cell_size.is_finite() || cell_size <= 0.0 {
        return Err(CommandError::InvalidPayload {
            command: "terrain.paint".to_string(),
            message: "o terreno tem `cell_size` inválido".to_string(),
        });
    }

    let mut work: BTreeMap<(i32, i32), ChunkWork> = BTreeMap::new();
    for handle in world
        .entities_with_table("TerrainChunk")
        .unwrap_or_default()
    {
        let owner = match world.get(handle, "TerrainChunk", "owner") {
            Ok(Value::Int(owner)) => *owner,
            _ => continue,
        };
        if owner != request.terrain as i64 {
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
            _ => HEIGHT_CHUNK_SIZE,
        };
        if !(2..=256).contains(&size) {
            continue;
        }
        let Ok(Value::Floats(heights)) = world.get(handle, "TerrainChunk", "heights") else {
            continue;
        };
        if heights.len() != size * size {
            return Err(CommandError::World(WorldError::InvalidValue {
                column: "heights".to_string(),
                reason: format!("bloco ({chunk_x}, {chunk_z}) com {} alturas", heights.len()),
            }));
        }
        let mut chunk = HeightChunk::new([chunk_x, chunk_z], size, cell_size);
        chunk.heights.copy_from_slice(heights);
        work.insert(
            (chunk_x, chunk_z),
            ChunkWork {
                entity: world.persistent_id(handle).ok().map(|id| id.0),
                original: heights.clone(),
                chunk,
            },
        );
    }

    let probe = Heightfield::new(cell_size);
    for stamp in &request.stamps {
        if !stamp.center.is_finite() {
            return Err(CommandError::InvalidPayload {
                command: "terrain.paint".to_string(),
                message: "o centro do carimbo não é finito".to_string(),
            });
        }
        let radius = finite_between("radius", stamp.radius, 0.01, 256.0)?;
        let strength = finite_between("strength", stamp.strength, 0.0, 64.0)?;
        if matches!(stamp.mode, TerrainMode::Stamp) && stamp.stamp.is_none() {
            return Err(CommandError::InvalidPayload {
                command: "terrain.paint".to_string(),
                message: "o modo `stamp` precisa de `stamp` (crater, mound, plateau ou ridge)"
                    .to_string(),
            });
        }
        if stamp.rotation.is_some_and(|value| !value.is_finite()) {
            return Err(CommandError::InvalidPayload {
                command: "terrain.paint".to_string(),
                message: "`rotation` precisa ser finito".to_string(),
            });
        }
        if matches!(stamp.mode, TerrainMode::Flatten)
            && stamp.target_height.is_some_and(|value| !value.is_finite())
        {
            return Err(CommandError::InvalidPayload {
                command: "terrain.paint".to_string(),
                message: "`target_height` precisa ser finito".to_string(),
            });
        }
        for position in probe.chunks_touching(stamp.center, radius) {
            let key = (position[0], position[1]);
            let entry = work.entry(key).or_insert_with(|| {
                let chunk = HeightChunk::new(position, HEIGHT_CHUNK_SIZE, cell_size);
                ChunkWork {
                    entity: None,
                    original: chunk.heights.clone(),
                    chunk,
                }
            });
            match stamp.mode {
                TerrainMode::Raise => entry.chunk.apply_brush_with_falloff(
                    stamp.center,
                    radius,
                    strength,
                    false,
                    falloff,
                ),
                TerrainMode::Lower => entry.chunk.apply_brush_with_falloff(
                    stamp.center,
                    radius,
                    strength,
                    true,
                    falloff,
                ),
                TerrainMode::Smooth => entry.chunk.smooth_region(stamp.center, radius, strength),
                TerrainMode::Flatten => entry.chunk.flatten_region(
                    stamp.center,
                    radius,
                    stamp.target_height.unwrap_or(stamp.center.y),
                    strength,
                ),
                TerrainMode::Noise => entry.chunk.roughen_region(stamp.center, radius, strength),
                TerrainMode::Erode => entry.chunk.erode_region(stamp.center, radius, strength),
                TerrainMode::Stamp => entry.chunk.stamp_region(
                    stamp.center,
                    radius,
                    strength,
                    stamp.stamp.unwrap_or(StampShape::Mound),
                    stamp.rotation.unwrap_or(0.0),
                ),
            }
        }
    }

    let mut inverse_records = Vec::new();
    let mut created = Vec::new();
    let mut changed = 0usize;
    for ((chunk_x, chunk_z), entry) in &work {
        if entry.chunk.heights == entry.original {
            continue;
        }
        changed += 1;
        match entry.entity {
            Some(entity) => {
                world.set(
                    resolve(world, entity)?,
                    "TerrainChunk",
                    "heights",
                    Value::Floats(entry.chunk.heights.clone()),
                )?;
                inverse_records.push(json!({
                    "command": "world.set",
                    "payload": {
                        "entity": entity,
                        "table": "TerrainChunk",
                        "column": "heights",
                        "value": Value::Floats(entry.original.clone()).to_json(),
                    },
                }));
            }
            None => {
                let chunk_id = spawn_named(
                    world,
                    "Bloco de terreno",
                    vec![(
                        "TerrainChunk",
                        map_from_json(json!({
                            "owner": request.terrain,
                            "chunk_x": *chunk_x,
                            "chunk_z": *chunk_z,
                            "size": entry.chunk.size as i64,
                            "cell_size": cell_size,
                            "heights": Value::Floats(entry.chunk.heights.clone()).to_json(),
                        })),
                    )],
                )?;
                created.push(chunk_id);
                inverse_records.push(json!({
                    "command": "world.remove",
                    "payload": { "entity": chunk_id },
                }));
            }
        }
    }
    // New chunks are removed first so undo never leaves orphan rows behind.
    inverse_records.sort_by_key(|record| {
        record
            .get("command")
            .and_then(Json::as_str)
            .map(|command| command == "world.set")
    });
    Ok(Outcome {
        result: json!({ "terrain": request.terrain, "chunks": changed }),
        inverse: batch_inverse(inverse_records),
    })
}

// ---- path.create / path.set ------------------------------------------------------------------

fn path_create_spec() -> CommandSpec {
    CommandSpec {
        id: "path.create".to_string(),
        summary: "Cria um rio ou uma estrada a partir de uma spline.".to_string(),
        params: vec![
            ParamSpec::required("kind", ParamKind::Text, "`river` ou `road`."),
            ParamSpec::required("spline", ParamKind::Any, "Spline {points, closed}."),
            ParamSpec::optional("name", ParamKind::Text, "Nome do caminho."),
            ParamSpec::optional("width", ParamKind::Float, "Largura da estrada, em metros."),
            ParamSpec::optional(
                "shoulder",
                ParamKind::Float,
                "Acostamento como fração da meia largura.",
            ),
            ParamSpec::optional("material", ParamKind::Text, "Material da estrada."),
            ParamSpec::optional("flow_speed", ParamKind::Float, "Velocidade da correnteza."),
            ParamSpec::optional("float_force", ParamKind::Float, "Força de flutuação."),
            ParamSpec::optional("swim_force", ParamKind::Float, "Força de nado."),
        ],
        returns: Some(ParamKind::Entity),
        failures: vec![FailureSpec::new(
            "COMMAND_INVALID_PAYLOAD",
            "Confira a spline e as medidas.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"kind": "river", "spline": {"points": [], "closed": false}}"#.to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PathKind {
    River,
    Road,
}

impl PathKind {
    fn table(self) -> &'static str {
        match self {
            Self::River => "River",
            Self::Road => "Road",
        }
    }

    fn default_name(self) -> &'static str {
        match self {
            Self::River => "Rio",
            Self::Road => "Estrada",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathCreatePayload {
    kind: PathKind,
    spline: CubicBezierSpline,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    shoulder: Option<f32>,
    #[serde(default)]
    material: Option<String>,
    #[serde(default)]
    guardrail: Option<bool>,
    #[serde(default)]
    flow_speed: Option<f32>,
    #[serde(default)]
    float_force: Option<f32>,
    #[serde(default)]
    swim_force: Option<f32>,
}

fn path_create(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: PathCreatePayload = parse("path.create", payload)?;
    validate_spline(&request.spline)?;
    let name = request
        .name
        .unwrap_or_else(|| request.kind.default_name().to_string());
    let row = match request.kind {
        PathKind::River => map_from_json(json!({
            "spline": Value::Spline(request.spline).to_json(),
            "flow_speed": finite_between("flow_speed", request.flow_speed.unwrap_or(1.0), 0.0, 1000.0)?,
            "float_force": finite_between("float_force", request.float_force.unwrap_or(9.8), 0.0, 1000.0)?,
            "swim_force": finite_between("swim_force", request.swim_force.unwrap_or(4.0), 0.0, 1000.0)?,
        })),
        PathKind::Road => map_from_json(json!({
            "spline": Value::Spline(request.spline).to_json(),
            "width": finite_between("width", request.width.unwrap_or(6.0), 0.1, 256.0)?,
            "shoulder": finite_between("shoulder", request.shoulder.unwrap_or(0.0), 0.0, 4.0)?,
            "material": request.material.unwrap_or_else(|| "gravel".to_string()),
            "guardrail": request.guardrail.unwrap_or(false),
        })),
    };
    let entity = spawn_named(world, &name, vec![(request.kind.table(), row)])?;
    Ok(Outcome {
        result: json!({ "entity": entity, "kind": match request.kind { PathKind::River => "river", PathKind::Road => "road" } }),
        inverse: Some(CommandRecord::new(
            "world.remove",
            json!({ "entity": entity }),
        )),
    })
}

fn path_set_spec() -> CommandSpec {
    CommandSpec {
        id: "path.set".to_string(),
        summary: "Muda a spline ou as medidas de um rio/estrada.".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Id do rio ou da estrada."),
            ParamSpec::optional("spline", ParamKind::Any, "Nova spline."),
            ParamSpec::optional("width", ParamKind::Float, "Nova largura da estrada."),
            ParamSpec::optional("shoulder", ParamKind::Float, "Novo acostamento."),
            ParamSpec::optional("material", ParamKind::Text, "Novo material."),
            ParamSpec::optional(
                "flow_speed",
                ParamKind::Float,
                "Nova velocidade da correnteza.",
            ),
        ],
        returns: None,
        failures: vec![FailureSpec::new(
            "COMMAND_INVALID_PAYLOAD",
            "Confira os valores.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 8, "spline": {"points": [], "closed": false}}"#.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathSetPayload {
    entity: u64,
    #[serde(default)]
    spline: Option<CubicBezierSpline>,
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    shoulder: Option<f32>,
    #[serde(default)]
    material: Option<String>,
    #[serde(default)]
    guardrail: Option<bool>,
    #[serde(default)]
    flow_speed: Option<f32>,
}

fn path_set(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: PathSetPayload = parse("path.set", payload)?;
    let handle = resolve(world, request.entity)?;
    let (table, updates): (&str, Vec<(&str, Json)>) = if world.has_row(handle, "River") {
        (
            "River",
            vec![
                (
                    "spline",
                    request
                        .spline
                        .map(|spline| Value::Spline(spline).to_json())
                        .unwrap_or(Json::Null),
                ),
                (
                    "flow_speed",
                    request
                        .flow_speed
                        .map(|value| json!(value))
                        .unwrap_or(Json::Null),
                ),
            ],
        )
    } else if world.has_row(handle, "Road") {
        (
            "Road",
            vec![
                (
                    "spline",
                    request
                        .spline
                        .map(|spline| Value::Spline(spline).to_json())
                        .unwrap_or(Json::Null),
                ),
                (
                    "width",
                    request
                        .width
                        .map(|value| json!(value))
                        .unwrap_or(Json::Null),
                ),
                (
                    "shoulder",
                    request
                        .shoulder
                        .map(|value| json!(value))
                        .unwrap_or(Json::Null),
                ),
                (
                    "material",
                    request
                        .material
                        .map(|value| json!(value))
                        .unwrap_or(Json::Null),
                ),
                (
                    "guardrail",
                    request
                        .guardrail
                        .map(|value| json!(value))
                        .unwrap_or(Json::Null),
                ),
            ],
        )
    } else {
        return Err(CommandError::InvalidPayload {
            command: "path.set".to_string(),
            message: "a entidade não é um rio nem uma estrada".to_string(),
        });
    };
    let mut old = Map::new();
    let mut pending = Vec::new();
    for (column, value) in updates {
        if value.is_null() {
            continue;
        }
        let kind = world.column_kind(table, column)?;
        let parsed =
            Value::from_json(kind, &value, world.limits().max_text_bytes).map_err(|reason| {
                WorldError::InvalidValue {
                    column: column.to_string(),
                    reason,
                }
            })?;
        if column == "spline"
            && let Value::Spline(spline) = &parsed
        {
            validate_spline(spline)?;
        }
        pending.push((column, parsed));
    }
    for (column, value) in pending {
        let previous = world.set(handle, table, column, value)?;
        old.insert(column.to_string(), previous.to_json());
    }
    Ok(Outcome {
        result: Json::Null,
        inverse: if old.is_empty() {
            None
        } else {
            Some(CommandRecord::new(
                "row.set",
                json!({ "entity": request.entity, "table": table, "values": old }),
            ))
        },
    })
}

fn road_structures_spec() -> CommandSpec {
    CommandSpec {
        id: "road.detect_structures".to_string(),
        summary: "Detecta pontes e bueiros comparando a estrada com o terreno.".to_string(),
        params: vec![
            ParamSpec::required("road", ParamKind::Entity, "Id da estrada."),
            ParamSpec::optional(
                "minimum_height_difference",
                ParamKind::Float,
                "Diferença de altura que exige uma estrutura, em metros.",
            ),
            ParamSpec::optional(
                "spacing",
                ParamKind::Float,
                "Passo de amostragem, em metros.",
            ),
        ],
        returns: Some(ParamKind::Any),
        failures: vec![FailureSpec::new(
            "WORLD_UNKNOWN_ENTITY",
            "Confira o id da estrada.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"road": 8, "minimum_height_difference": 2.0}"#.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoadStructuresPayload {
    road: u64,
    #[serde(default)]
    minimum_height_difference: Option<f32>,
    #[serde(default)]
    spacing: Option<f32>,
}

fn road_detect_structures(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    use petunia_sculpt::{RoadStructureKind, detect_road_structures};

    let request: RoadStructuresPayload = parse("road.detect_structures", payload)?;
    let handle = resolve(world, request.road)?;
    let Ok(Value::Spline(spline)) = world.get(handle, "Road", "spline") else {
        return Err(CommandError::InvalidPayload {
            command: "road.detect_structures".to_string(),
            message: "a entidade não é uma estrada".to_string(),
        });
    };
    let width = match world.get(handle, "Road", "width") {
        Ok(Value::Float(value)) => *value as f32,
        _ => 6.0,
    };
    let threshold = finite_between(
        "minimum_height_difference",
        request.minimum_height_difference.unwrap_or(2.0),
        0.0,
        128.0,
    )?;
    let spacing = finite_between("spacing", request.spacing.unwrap_or(1.0), 0.1, 32.0)?;
    let mut heightfield = Heightfield::new(1.0);
    for chunk_handle in world
        .entities_with_table("TerrainChunk")
        .unwrap_or_default()
    {
        let chunk_x = match world.get(chunk_handle, "TerrainChunk", "chunk_x") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let chunk_z = match world.get(chunk_handle, "TerrainChunk", "chunk_z") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let size = match world.get(chunk_handle, "TerrainChunk", "size") {
            Ok(Value::Int(value)) => *value as usize,
            _ => continue,
        };
        let cell_size = match world.get(chunk_handle, "TerrainChunk", "cell_size") {
            Ok(Value::Float(value)) => *value as f32,
            _ => continue,
        };
        let Ok(Value::Floats(heights)) = world.get(chunk_handle, "TerrainChunk", "heights") else {
            continue;
        };
        if !(2..=256).contains(&size) || heights.len() != size * size {
            continue;
        }
        let mut chunk = HeightChunk::new([chunk_x, chunk_z], size, cell_size);
        chunk.heights.copy_from_slice(heights);
        heightfield.default_cell_size = cell_size;
        heightfield.chunks.push(chunk);
    }

    let found = detect_road_structures(&heightfield, spline, spacing, threshold);
    let mut inverse = Vec::new();
    for old in world.entities_with_table("Bridge").unwrap_or_default() {
        let is_ours = matches!(
            world.get(old, "Bridge", "road"),
            Ok(Value::Int(road)) if *road == request.road as i64
        );
        if !is_ours {
            continue;
        }
        if let (Ok(PersistentId(id)), Ok(snapshot)) =
            (world.persistent_id(old), world.snapshot(old))
        {
            let mut rows = Map::new();
            for (table, values) in &snapshot.rows {
                rows.insert(
                    table.clone(),
                    Json::Object(world.row_to_json(table, values)?),
                );
            }
            inverse.push(json!({
                "command": "world.create",
                "payload": { "persistent_id": id, "tags": snapshot.tags, "rows": rows },
            }));
        }
        let _ = world.despawn(old);
    }
    let mut bridges = 0;
    let mut culverts = 0;
    let mut spans = Vec::new();
    for structure in found {
        let kind = match structure.kind {
            RoadStructureKind::Bridge => {
                bridges += 1;
                "bridge"
            }
            RoadStructureKind::Culvert => {
                culverts += 1;
                "culvert"
            }
        };
        let bridge_id = spawn_named(
            world,
            if kind == "bridge" { "Ponte" } else { "Bueiro" },
            vec![(
                "Bridge",
                map_from_json(json!({
                    "road": request.road,
                    "kind": kind,
                    "start_parameter": structure.start_parameter,
                    "end_parameter": structure.end_parameter,
                    "width": width,
                })),
            )],
        )?;
        inverse.push(json!({ "command": "world.remove", "payload": { "entity": bridge_id } }));
        spans.push(json!({
            "kind": kind,
            "start_parameter": structure.start_parameter,
            "end_parameter": structure.end_parameter,
            "height_difference": structure.maximum_height_difference,
        }));
    }
    Ok(Outcome {
        result: json!({ "bridges": bridges, "culverts": culverts, "spans": spans }),
        inverse: batch_inverse(inverse),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_world::World;

    fn world() -> World {
        let mut world = World::new(1);
        petunia_project::register_standard_tables(&mut world).expect("tables");
        world
    }

    fn river_spline() -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(petunia_math::SplinePoint {
            position: Vec3::new(0.0, 4.0, 0.0),
            ..petunia_math::SplinePoint::default()
        });
        spline.points.push(petunia_math::SplinePoint {
            position: Vec3::new(10.0, 1.0, 0.0),
            ..petunia_math::SplinePoint::default()
        });
        spline
    }

    #[test]
    fn terrain_create_and_paint_are_undoable() {
        let mut world = world();
        let created = terrain_create(
            &mut world,
            &json!({ "cell_size": 1.0, "size_meters": 64.0 }),
        )
        .expect("create");
        let terrain = created.result["entity"].as_u64().unwrap_or(0);
        assert!(terrain > 0);
        let before = world
            .entities_with_table("TerrainChunk")
            .unwrap_or_default()
            .len();
        let painted = terrain_paint(
            &mut world,
            &json!({
                "terrain": terrain,
                "stamps": [{
                    "center": {"x": 10.0, "y": 0.0, "z": 10.0},
                    "radius": 3.0,
                    "strength": 2.0,
                    "mode": "raise"
                }]
            }),
        )
        .expect("paint");
        assert!(painted.result["chunks"].as_u64().unwrap_or(0) > 0);
        let heights = world
            .entities_with_table("TerrainChunk")
            .unwrap_or_default()
            .into_iter()
            .filter_map(
                |handle| match world.get(handle, "TerrainChunk", "heights") {
                    Ok(Value::Floats(heights)) => Some(heights.iter().any(|value| *value > 0.0)),
                    _ => None,
                },
            )
            .any(|raised| raised);
        assert!(heights, "a chunk foi elevada");
        assert!(
            world
                .entities_with_table("TerrainChunk")
                .unwrap_or_default()
                .len()
                >= before
        );
    }

    #[test]
    fn river_create_requires_a_valid_spline() {
        let mut world = world();
        let rejected = path_create(
            &mut world,
            &json!({
                "kind": "river",
                "spline": { "points": [], "closed": false }
            }),
        );
        assert!(rejected.is_err());
        let created = path_create(
            &mut world,
            &json!({ "kind": "river", "spline": Value::Spline(river_spline()).to_json() }),
        )
        .expect("river");
        let entity = created.result["entity"].as_u64().unwrap_or(0);
        let handle = world.handle_of(PersistentId(entity)).expect("handle");
        assert!(world.has_row(handle, "River"));
    }

    #[test]
    fn path_set_changes_width_and_keeps_an_inverse() {
        let mut world = world();
        let created = path_create(
            &mut world,
            &json!({
                "kind": "road",
                "spline": Value::Spline(river_spline()).to_json(),
                "width": 6.0
            }),
        )
        .expect("road");
        let entity = created.result["entity"].as_u64().unwrap_or(0);
        let changed =
            path_set(&mut world, &json!({ "entity": entity, "width": 9.0 })).expect("set");
        assert!(changed.inverse.is_some());
        let handle = world.handle_of(PersistentId(entity)).expect("handle");
        assert_eq!(world.get(handle, "Road", "width"), Ok(&Value::Float(9.0)));
    }
}
