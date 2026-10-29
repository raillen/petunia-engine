//! Terrain commands (DR-33): create, generate, sculpt, paint and patch a chunked heightmap.
//!
//! The terrain lives in one `Terrain` row of the scene. A command edits a copy, then writes the
//! whole value back, so the dispatcher keeps the undo history consistent. The inverse is a
//! `terrain.patch` that carries only the chunks the command touched, not the whole terrain.

use petunia_math::Vec3;
use petunia_schema::{
    CommandSpec, Cost, FailureSpec, ParamKind, ParamSpec, QuerySpec, Reversibility, Status,
};
use petunia_terrain::{
    Biome, Brush, BrushShape, ChunkCoord, Falloff, FractalSettings, Heightmap, MAX_BRUSH_SAMPLES,
    MAX_HEIGHTMAP_SIDE, Stamp, StampMode, Terrain, TerrainChunk, TerrainError, TerrainGeneration,
    TerrainHole, TerrainOperation,
};
use petunia_world::{PersistentId, Value, World};
use serde::Deserialize;
use serde_json::{Map, Value as Json, json};

use crate::dispatcher::{ApplyFn, BATCH_COMMAND, CommandRecord, Outcome};
use crate::error::CommandError;

const TERRAIN_TABLE: &str = "Terrain";
const TERRAIN_COLUMN: &str = "data";

pub(crate) fn queries() -> Vec<QuerySpec> {
    vec![
        QuerySpec {
            id: "terrain.height".to_string(),
            summary: "Altura e normal do terreno numa posição do mundo.".to_string(),
            params: vec![
                ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
                ParamSpec::required("x", ParamKind::Float, "Posição x no mundo (metros)."),
                ParamSpec::required("z", ParamKind::Float, "Posição z no mundo (metros)."),
            ],
            returns: ParamKind::Any,
            failures: vec![FailureSpec::new(
                "WORLD_UNKNOWN_ENTITY",
                "Confira o id do terreno.",
            )],
            permission: "world.read".to_string(),
            status: Status::Provisional,
            example: r#"{"entity": 4, "x": 10.5, "z": -3.0}"#.to_string(),
        },
        QuerySpec {
            id: "terrain.heights".to_string(),
            summary: "Uma grade de alturas do terreno, em linha, para exportar um heightmap."
                .to_string(),
            params: vec![
                ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
                ParamSpec::optional(
                    "x",
                    ParamKind::Float,
                    "x da primeira amostra em metros (padrão: início do terreno).",
                ),
                ParamSpec::optional(
                    "z",
                    ParamKind::Float,
                    "z da primeira amostra em metros (padrão: início do terreno).",
                ),
                ParamSpec::optional(
                    "width",
                    ParamKind::Int,
                    "Amostras em x (1 a 2048; padrão: a largura do terreno).",
                ),
                ParamSpec::optional(
                    "depth",
                    ParamKind::Int,
                    "Amostras em z (1 a 2048; padrão: a profundidade do terreno).",
                ),
            ],
            returns: ParamKind::Any,
            failures: vec![FailureSpec::new(
                "TERRAIN_INVALID",
                "Ajuste a área pedida (largura, profundidade, origem).",
            )],
            permission: "world.read".to_string(),
            status: Status::Provisional,
            example: r#"{"entity": 4, "x": 0, "z": 0, "width": 64, "depth": 64}"#.to_string(),
        },
        QuerySpec {
            id: "terrain.info".to_string(),
            summary: "Resumo do terreno: célula, blocos, amostras e limites.".to_string(),
            params: vec![ParamSpec::required("entity", ParamKind::Entity, "Terreno.")],
            returns: ParamKind::Any,
            failures: vec![FailureSpec::new(
                "WORLD_UNKNOWN_ENTITY",
                "Confira o id do terreno.",
            )],
            permission: "world.read".to_string(),
            status: Status::Provisional,
            example: r#"{"entity": 4}"#.to_string(),
        },
    ]
}

pub(crate) fn run_query(world: &World, query: &str, payload: &Json) -> Result<Json, CommandError> {
    match query {
        "terrain.height" => {
            let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
            let x = payload.get("x").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let z = payload.get("z").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let terrain = read_terrain(world, entity)?;
            let height = terrain.height_at(x, z);
            let normal = terrain.normal_at(x, z);
            Ok(json!({
                "height": height,
                "normal": normal.map(|normal| json!({
                    "x": normal.x, "y": normal.y, "z": normal.z
                })),
            }))
        }
        "terrain.heights" => {
            let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
            let terrain = read_terrain(world, entity)?;
            let (start_x, start_z, whole_width, whole_depth) = match terrain.bounds() {
                Some((minimum, maximum)) => (
                    minimum.x,
                    minimum.z,
                    ((maximum.x - minimum.x) / terrain.cell_size()).round() as i64 + 1,
                    ((maximum.z - minimum.z) / terrain.cell_size()).round() as i64 + 1,
                ),
                None => (0.0, 0.0, 1, 1),
            };
            let x = payload
                .get("x")
                .and_then(Json::as_f64)
                .map(|value| value as f32)
                .unwrap_or(start_x);
            let z = payload
                .get("z")
                .and_then(Json::as_f64)
                .map(|value| value as f32)
                .unwrap_or(start_z);
            let width = payload
                .get("width")
                .and_then(Json::as_u64)
                .map(|value| value as usize)
                .unwrap_or(usize::try_from(whole_width).unwrap_or(1))
                .clamp(1, MAX_HEIGHTMAP_SIDE);
            let depth = payload
                .get("depth")
                .and_then(Json::as_u64)
                .map(|value| value as usize)
                .unwrap_or(usize::try_from(whole_depth).unwrap_or(1))
                .clamp(1, MAX_HEIGHTMAP_SIDE);
            let map = Heightmap::from_terrain(&terrain, x, z, width, depth)
                .map_err(|error| terrain_error("terrain.heights", error))?;
            Ok(json!({
                "cell_size": map.cell_size,
                "origin_x": map.origin_x,
                "origin_z": map.origin_z,
                "width": map.width,
                "depth": map.depth,
                "heights": map.heights,
            }))
        }
        "terrain.info" => {
            let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
            let terrain = read_terrain(world, entity)?;
            let chunks: Vec<Json> = terrain
                .chunks()
                .map(|chunk| {
                    let coord = chunk.coord();
                    json!({ "x": coord.x, "z": coord.z })
                })
                .collect();
            Ok(json!({
                "cell_size": terrain.cell_size(),
                "chunk_cells": terrain.chunk_cells(),
                "chunk_count": terrain.chunk_count(),
                "sample_count": terrain.sample_count(),
                "chunks": chunks,
            }))
        }
        other => Err(CommandError::UnknownCommand {
            name: other.to_string(),
            suggestions: Vec::new(),
        }),
    }
}

pub(crate) fn commands() -> Vec<(CommandSpec, ApplyFn)> {
    vec![
        (create_spec(), create as ApplyFn),
        (generate_spec(), generate as ApplyFn),
        (brush_spec(), brush as ApplyFn),
        (paint_spec(), paint as ApplyFn),
        (patch_spec(), patch as ApplyFn),
        (holes_spec(), holes as ApplyFn),
        (follow_spec(), follow as ApplyFn),
        (surface_set_spec(), surface_set as ApplyFn),
        (erode_spec(), erode as ApplyFn),
        (holes_from_caves_spec(), holes_from_caves as ApplyFn),
        (stamp_spec(), stamp as ApplyFn),
        (hydraulic_spec(), hydraulic as ApplyFn),
        (stamp_mesh_spec(), stamp_mesh as ApplyFn),
    ]
}

// ---- payloads -------------------------------------------------------------------------------

fn default_cell_size() -> f32 {
    1.0
}

fn default_chunk_cells() -> usize {
    16
}

fn default_name() -> String {
    "Terreno".to_string()
}

fn default_falloff() -> String {
    "smooth".to_string()
}

fn default_strength() -> f32 {
    1.0
}

fn default_noise_feature_size() -> f32 {
    64.0
}

fn default_noise_amplitude() -> f32 {
    1.0
}

fn default_terrace_step() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePayload {
    #[serde(default = "default_cell_size")]
    cell_size: f32,
    #[serde(default = "default_chunk_cells")]
    chunk_cells: usize,
    #[serde(default = "default_name")]
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratePayload {
    entity: u64,
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    #[serde(default)]
    seed: u64,
    /// Every relief parameter is optional: a biome supplies its own, and a value sent here wins.
    #[serde(default)]
    feature_size: Option<f32>,
    #[serde(default)]
    amplitude: Option<f32>,
    #[serde(default)]
    octaves: Option<u32>,
    #[serde(default)]
    lacunarity: Option<f32>,
    #[serde(default)]
    persistence: Option<f32>,
    #[serde(default)]
    base_height: Option<f32>,
    /// `plains`, `forest`, `desert`, `snow`, `volcano` or `none`. Missing: the seed picks.
    #[serde(default)]
    biome: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrushPayload {
    entity: u64,
    shape: String,
    center: Vec3,
    #[serde(default)]
    radius: Option<f32>,
    #[serde(default)]
    half_extents: Option<Vec3>,
    #[serde(default)]
    half_height: Option<f32>,
    #[serde(default = "default_falloff")]
    falloff: String,
    #[serde(default = "default_strength")]
    strength: f32,
    operation: String,
    #[serde(default)]
    target_height: Option<f32>,
    #[serde(default)]
    noise_seed: u64,
    #[serde(default = "default_noise_feature_size")]
    noise_feature_size: f32,
    #[serde(default = "default_noise_amplitude")]
    noise_amplitude: f32,
    #[serde(default = "default_terrace_step")]
    terrace_step: f32,
    #[serde(default)]
    mirror_x: bool,
    #[serde(default)]
    mirror_z: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaintPayload {
    entity: u64,
    shape: String,
    center: Vec3,
    #[serde(default)]
    radius: Option<f32>,
    #[serde(default)]
    half_extents: Option<Vec3>,
    #[serde(default)]
    half_height: Option<f32>,
    #[serde(default = "default_falloff")]
    falloff: String,
    material: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StampMeshPayload {
    entity: u64,
    mesh: u64,
    #[serde(default = "default_stamp_mode")]
    mode: String,
    #[serde(default = "default_strength")]
    strength: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HydraulicPayload {
    entity: u64,
    #[serde(default = "default_droplets")]
    droplets: u32,
    #[serde(default = "default_erosion_strength")]
    strength: f32,
    #[serde(default = "default_deposition")]
    deposition: f32,
    #[serde(default = "default_evaporation")]
    evaporation: f32,
    #[serde(default)]
    seed: u64,
}

fn default_droplets() -> u32 {
    2000
}

fn default_deposition() -> f32 {
    0.3
}

fn default_evaporation() -> f32 {
    0.02
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StampPayload {
    entity: u64,
    x: f32,
    z: f32,
    width: u32,
    depth: u32,
    heights: Vec<f32>,
    #[serde(default = "default_stamp_mode")]
    mode: String,
    #[serde(default = "default_strength")]
    strength: f32,
}

fn default_stamp_mode() -> String {
    "replace".to_string()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HolesFromCavesPayload {
    entity: u64,
    /// Keep the holes that already exist (default: replace them).
    #[serde(default)]
    keep_existing: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceSetPayload {
    values: Vec<SurfaceSetValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceSetValue {
    entity: u64,
    #[serde(default)]
    position: Option<Vec3>,
    #[serde(default)]
    rotation_degrees: Option<Vec3>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErodePayload {
    entity: u64,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_talus")]
    talus: f32,
    #[serde(default = "default_erosion_strength")]
    strength: f32,
}

fn default_iterations() -> u32 {
    8
}

fn default_talus() -> f32 {
    0.5
}

fn default_erosion_strength() -> f32 {
    0.5
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowPayload {
    terrain: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HolesPayload {
    entity: u64,
    holes: Vec<HoleEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HoleEntry {
    x: f32,
    z: f32,
    radius: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchPayload {
    entity: u64,
    chunks: Vec<PatchChunk>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchChunk {
    x: i32,
    z: i32,
    #[serde(default)]
    remove: bool,
    #[serde(default)]
    heights: Option<Vec<f32>>,
    #[serde(default)]
    materials: Option<Vec<u8>>,
}

// ---- helpers --------------------------------------------------------------------------------

fn invalid(command: &str, message: impl Into<String>) -> CommandError {
    CommandError::InvalidPayload {
        command: command.to_string(),
        message: message.into(),
    }
}

fn terrain_error(command: &str, error: TerrainError) -> CommandError {
    invalid(command, error.to_string())
}

fn read_terrain(world: &World, entity: u64) -> Result<Terrain, CommandError> {
    let handle = world.handle_of(PersistentId(entity))?;
    match world.get(handle, TERRAIN_TABLE, TERRAIN_COLUMN)? {
        Value::Terrain(terrain) => Ok(terrain.clone()),
        other => Err(invalid(
            "terrain",
            format!(
                "a entidade {entity} não tem um terreno (a coluna é do tipo `{}`)",
                other.kind().as_str()
            ),
        )),
    }
}

fn write_terrain(world: &mut World, entity: u64, terrain: &Terrain) -> Result<(), CommandError> {
    let handle = world.handle_of(PersistentId(entity))?;
    world.set(
        handle,
        TERRAIN_TABLE,
        TERRAIN_COLUMN,
        Value::Terrain(terrain.clone()),
    )?;
    Ok(())
}

fn chunk_json(chunk: &TerrainChunk) -> Json {
    json!({
        "x": chunk.coord().x,
        "z": chunk.coord().z,
        "heights": chunk.heights(),
        "materials": chunk.materials(),
    })
}

fn removed_chunk_json(coord: ChunkCoord) -> Json {
    json!({ "x": coord.x, "z": coord.z, "remove": true })
}

fn terrain_inverse(
    entity: u64,
    previous: Vec<Json>,
    follow: Option<CommandRecord>,
) -> CommandRecord {
    let patch = patch_json(entity, previous);
    match follow {
        Some(follow) => CommandRecord::new(BATCH_COMMAND, json!({ "records": [patch, follow] })),
        None => patch,
    }
}

/// Applies the surface-follow targets for a new terrain. Returns how many objects moved and the
/// inverse (`surface.set` with the old values), so the caller can combine it with the terrain
/// patch in one undo step.
fn apply_surface_updates(
    world: &mut World,
    terrain: &Terrain,
) -> Result<(u64, Option<CommandRecord>), CommandError> {
    let entities = world
        .entities_with_table("SurfaceFollow")
        .unwrap_or_default();
    let mut old_values = Vec::new();
    let mut moved = 0u64;
    for handle in entities {
        let entity = world.persistent_id(handle)?.0;
        let offset = match world.get(handle, "SurfaceFollow", "offset") {
            Ok(Value::Float(value)) if value.is_finite() => *value as f32,
            _ => 0.0,
        };
        let align = matches!(
            world.get(handle, "SurfaceFollow", "align_normal"),
            Ok(Value::Bool(true))
        );
        let Ok(Value::Vec3(position)) = world.get(handle, "Transform", "position") else {
            continue;
        };
        let position = *position;
        let Some(height) = terrain.height_at(position.x, position.z) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("entity".to_string(), json!(entity));
        let target = Vec3::new(position.x, height + offset, position.z);
        if target != position {
            let old = world.set(handle, "Transform", "position", Value::Vec3(target))?;
            entry.insert("position".to_string(), old.to_json());
            moved += 1;
        }
        if align {
            let normal = terrain
                .normal_at(position.x, position.z)
                .unwrap_or(Vec3::UP);
            let pitch = (-normal.z).atan2(normal.y).to_degrees();
            let roll = normal.x.atan2(normal.y).to_degrees();
            let old_rotation = match world.get(handle, "Transform", "rotation_degrees") {
                Ok(Value::Vec3(rotation)) => *rotation,
                _ => Vec3::ZERO,
            };
            let target_rotation = Vec3::new(pitch, old_rotation.y, roll);
            if target_rotation != old_rotation {
                let old = world.set(
                    handle,
                    "Transform",
                    "rotation_degrees",
                    Value::Vec3(target_rotation),
                )?;
                entry.insert("rotation_degrees".to_string(), old.to_json());
            }
        }
        if entry.len() > 1 {
            old_values.push(Json::Object(entry));
        }
    }
    let inverse = if old_values.is_empty() {
        None
    } else {
        Some(CommandRecord::new(
            "surface.set",
            json!({ "values": old_values }),
        ))
    };
    Ok((moved, inverse))
}

fn patch_json(entity: u64, chunks: Vec<Json>) -> CommandRecord {
    CommandRecord::new(
        "terrain.patch",
        json!({ "entity": entity, "chunks": chunks }),
    )
}

struct BrushArguments<'a> {
    command: &'a str,
    shape: &'a str,
    center: Vec3,
    radius: Option<f32>,
    half_extents: Option<Vec3>,
    half_height: Option<f32>,
    falloff: &'a str,
    strength: f32,
}

fn brush_from_payload(arguments: BrushArguments<'_>) -> Result<Brush, CommandError> {
    let BrushArguments {
        command,
        shape,
        center,
        radius,
        half_extents,
        half_height,
        falloff,
        strength,
    } = arguments;
    let missing = |what: &str| invalid(command, format!("a forma `{shape}` precisa de `{what}`"));
    let shape = match shape {
        "sphere" => BrushShape::Sphere {
            radius: radius.ok_or_else(|| missing("radius"))?,
        },
        "box" => BrushShape::Box {
            half_extents: half_extents.ok_or_else(|| missing("half_extents"))?,
        },
        "cylinder" => BrushShape::Cylinder {
            radius: radius.ok_or_else(|| missing("radius"))?,
            half_height: half_height.ok_or_else(|| missing("half_height"))?,
        },
        "capsule" => BrushShape::Capsule {
            radius: radius.ok_or_else(|| missing("radius"))?,
            half_height: half_height.ok_or_else(|| missing("half_height"))?,
        },
        other => {
            return Err(invalid(
                command,
                format!(
                    "forma de pincel desconhecida `{other}`; use sphere, box, cylinder ou capsule"
                ),
            ));
        }
    };
    let falloff = match falloff {
        "constant" => Falloff::Constant,
        "linear" => Falloff::Linear,
        "smooth" => Falloff::Smooth,
        "gaussian" => Falloff::Gaussian,
        other => {
            return Err(invalid(
                command,
                format!("falloff desconhecido `{other}`; use constant, linear, smooth ou gaussian"),
            ));
        }
    };
    Ok(Brush::new(shape, center, falloff, strength))
}

fn operation_from_payload(
    command: &str,
    operation: &str,
    target_height: Option<f32>,
    noise_seed: u64,
    noise_feature_size: f32,
    noise_amplitude: f32,
    terrace_step: f32,
) -> Result<TerrainOperation, CommandError> {
    Ok(match operation {
        "raise" => TerrainOperation::Raise,
        "lower" => TerrainOperation::Lower,
        "smooth" => TerrainOperation::Smooth,
        "flatten" => TerrainOperation::Flatten { target_height },
        "noise" => TerrainOperation::Noise {
            seed: noise_seed,
            feature_size: noise_feature_size,
            amplitude: noise_amplitude,
        },
        "terrace" => TerrainOperation::Terrace { step: terrace_step },
        other => {
            return Err(invalid(
                command,
                format!(
                    "operação desconhecida `{other}`; use raise, lower, smooth, flatten, noise ou terrace"
                ),
            ));
        }
    })
}

fn region_of(payload: &GeneratePayload) -> Result<(ChunkCoord, ChunkCoord), CommandError> {
    let minimum = ChunkCoord::new(payload.min_x, payload.min_z);
    let maximum = ChunkCoord::new(payload.max_x, payload.max_z);
    if maximum.x < minimum.x || maximum.z < minimum.z {
        return Err(invalid(
            "terrain.generate",
            "o retângulo de geração está invertido",
        ));
    }
    Ok((minimum, maximum))
}

fn region_chunks(terrain: &Terrain, minimum: ChunkCoord, maximum: ChunkCoord) -> Vec<Json> {
    let mut patch = Vec::new();
    for x in minimum.x..=maximum.x {
        for z in minimum.z..=maximum.z {
            let coord = ChunkCoord::new(x, z);
            patch.push(match terrain.capture_chunk(coord) {
                Some(chunk) => chunk_json(&chunk),
                None => removed_chunk_json(coord),
            });
        }
    }
    patch
}

// ---- terrain.create --------------------------------------------------------------------------

fn create_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.create".to_string(),
        summary: "Cria uma entidade com um terreno (heightmap em chunks) vazio.".to_string(),
        params: vec![
            ParamSpec::optional(
                "cell_size",
                ParamKind::Float,
                "Tamanho da célula em metros (0,05 a 64; padrão 1).",
            ),
            ParamSpec::optional(
                "chunk_cells",
                ParamKind::Int,
                "Células por lado do bloco (1 a 256; padrão 16).",
            ),
            ParamSpec::optional(
                "name",
                ParamKind::Text,
                "Nome do objeto (padrão `Terreno`).",
            ),
        ],
        returns: Some(ParamKind::Entity),
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste `cell_size` ou `chunk_cells`."),
            FailureSpec::new(
                "WORLD_UNKNOWN_TABLE",
                "Abra um projeto com as tabelas padrão.",
            ),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"cell_size": 1.0, "chunk_cells": 16, "name": "Ilha"}"#.to_string(),
    }
}

fn create(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: CreatePayload = parse("terrain.create", payload)?;
    let terrain = Terrain::new(request.cell_size, request.chunk_cells)
        .map_err(|error| terrain_error("terrain.create", error))?;
    let handle = world.spawn()?;
    let filled = (|| -> Result<(), CommandError> {
        world.insert_row_values(handle, "Name", vec![Value::Text(request.name.clone())])?;
        world.insert_row_values(handle, TERRAIN_TABLE, vec![Value::Terrain(terrain)])?;
        Ok(())
    })();
    if let Err(error) = filled {
        let _ = world.despawn(handle);
        return Err(error);
    }
    let id = world.persistent_id(handle)?.0;
    Ok(Outcome {
        result: json!({ "entity": id }),
        inverse: Some(CommandRecord::new("world.remove", json!({ "entity": id }))),
    })
}

// ---- terrain.generate ------------------------------------------------------------------------

fn generate_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.generate".to_string(),
        summary: "Gera (ou regenera) blocos de terreno com ruído determinístico.".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required("min_x", ParamKind::Int, "Primeiro bloco em x."),
            ParamSpec::required("min_z", ParamKind::Int, "Primeiro bloco em z."),
            ParamSpec::required("max_x", ParamKind::Int, "Último bloco em x (inclusive)."),
            ParamSpec::required("max_z", ParamKind::Int, "Último bloco em z (inclusive)."),
            ParamSpec::optional("seed", ParamKind::Int, "Semente (padrão 0)."),
            ParamSpec::optional("feature_size", ParamKind::Float, "Tamanho da feição em metros (padrão 64)."),
            ParamSpec::optional("amplitude", ParamKind::Float, "Altura máxima em metros (padrão 12)."),
            ParamSpec::optional("octaves", ParamKind::Int, "Oitavas de ruído (1 a 16; padrão 5)."),
            ParamSpec::optional("lacunarity", ParamKind::Float, "Frequência entre oitavas (padrão 2)."),
            ParamSpec::optional("persistence", ParamKind::Float, "Amplitude entre oitavas (padrão 0,5)."),
            ParamSpec::optional("base_height", ParamKind::Float, "Altura base em metros (padrão 0)."),
            ParamSpec::optional(
                "biome",
                ParamKind::Text,
                "Bioma: plains, forest, desert, snow, volcano ou none. Sem ele, a semente escolhe.",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste os parâmetros do gerador."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "min_x": 0, "min_z": 0, "max_x": 1, "max_z": 1, "seed": 7, "amplitude": 10}"#.to_string(),
    }
}

fn generate(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: GeneratePayload = parse("terrain.generate", payload)?;
    let (minimum, maximum) = region_of(&request)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let previous = region_chunks(&terrain, minimum, maximum);
    // Without a biome the seed picks one, so the same seed always paints the same world; `none`
    // keeps a plain heightfield (every sample on the first layer). The biome brings the relief
    // (amplitude, feature size, octaves) and the layers; a parameter sent in the payload wins over
    // it, which is how a script can ask for a flatter desert.
    let biome = match request.biome.as_deref() {
        Some("none") => None,
        Some(name) => Some(Biome::parse(name).ok_or_else(|| {
            invalid(
                "terrain.generate",
                format!(
                    "bioma desconhecido `{name}`; use plains, forest, desert, snow, volcano ou none"
                ),
            )
        })?),
        None => Some(Biome::from_seed(request.seed)),
    };
    let mut settings = match biome {
        Some(biome) => biome.settings(request.seed),
        None => TerrainGeneration {
            fractal: FractalSettings {
                seed: request.seed,
                ..FractalSettings::default()
            },
            base_height: 0.0,
            biome: None,
        },
    };
    settings.fractal.seed = request.seed;
    if let Some(value) = request.feature_size {
        settings.fractal.feature_size = value;
    }
    if let Some(value) = request.amplitude {
        settings.fractal.amplitude = value;
    }
    if let Some(value) = request.octaves {
        settings.fractal.octaves = value;
    }
    if let Some(value) = request.lacunarity {
        settings.fractal.lacunarity = value;
    }
    if let Some(value) = request.persistence {
        settings.fractal.persistence = value;
    }
    if let Some(value) = request.base_height {
        settings.base_height = value;
    }
    terrain
        .generate_region(minimum, maximum, &settings)
        .map_err(|error| terrain_error("terrain.generate", error))?;
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    let count = (i64::from(maximum.x) - i64::from(minimum.x) + 1)
        * (i64::from(maximum.z) - i64::from(minimum.z) + 1);
    Ok(Outcome {
        result: json!({
            "chunks": count,
            "biome": biome.map(|biome| biome.name()),
        }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.brush ---------------------------------------------------------------------------

fn brush_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.brush".to_string(),
        summary: "Esculpe o terreno com um pincel 3D (subir, baixar, suavizar, aplainar, ruído, terraço).".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required("shape", ParamKind::Text, "sphere, box, cylinder ou capsule."),
            ParamSpec::required("center", ParamKind::Vec3, "Centro do pincel no mundo."),
            ParamSpec::optional("radius", ParamKind::Float, "Raio (esfera, cilindro, cápsula)."),
            ParamSpec::optional("half_extents", ParamKind::Vec3, "Meia-extensão da caixa."),
            ParamSpec::optional("half_height", ParamKind::Float, "Meia-altura (cilindro, cápsula)."),
            ParamSpec::optional("falloff", ParamKind::Text, "constant, linear, smooth (padrão) ou gaussian."),
            ParamSpec::optional("strength", ParamKind::Float, "Metros (subir/baixar) ou mistura 0..1."),
            ParamSpec::required("operation", ParamKind::Text, "raise, lower, smooth, flatten, noise ou terrace."),
            ParamSpec::optional("target_height", ParamKind::Float, "Plano do `flatten`; sem ele, a altura do centro."),
            ParamSpec::optional("noise_seed", ParamKind::Int, "Semente do ruído."),
            ParamSpec::optional("noise_feature_size", ParamKind::Float, "Tamanho da feição do ruído."),
            ParamSpec::optional("noise_amplitude", ParamKind::Float, "Amplitude do ruído em metros."),
            ParamSpec::optional("terrace_step", ParamKind::Float, "Altura do degrau do terraço."),
            ParamSpec::optional("mirror_x", ParamKind::Bool, "Aplica também espelhado em x = 0."),
            ParamSpec::optional("mirror_z", ParamKind::Bool, "Aplica também espelhado em z = 0."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste o pincel ou a operação."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "shape": "sphere", "center": {"x": 0, "y": 0, "z": 0}, "radius": 3, "falloff": "smooth", "strength": 1.0, "operation": "raise"}"#.to_string(),
    }
}

fn brush(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: BrushPayload = parse("terrain.brush", payload)?;
    let brush = brush_from_payload(BrushArguments {
        command: "terrain.brush",
        shape: &request.shape,
        center: request.center,
        radius: request.radius,
        half_extents: request.half_extents,
        half_height: request.half_height,
        falloff: &request.falloff,
        strength: request.strength,
    })?;
    let operation = operation_from_payload(
        "terrain.brush",
        &request.operation,
        request.target_height,
        request.noise_seed,
        request.noise_feature_size,
        request.noise_amplitude,
        request.terrace_step,
    )?;
    let mut centers = vec![brush.center];
    if request.mirror_x {
        centers.push(Vec3::new(-brush.center.x, brush.center.y, brush.center.z));
    }
    if request.mirror_z {
        centers.push(Vec3::new(brush.center.x, brush.center.y, -brush.center.z));
    }
    if request.mirror_x && request.mirror_z {
        centers.push(Vec3::new(-brush.center.x, brush.center.y, -brush.center.z));
    }
    let mut terrain = read_terrain(world, request.entity)?;
    let mut minimum = ChunkCoord::new(i32::MAX, i32::MAX);
    let mut maximum = ChunkCoord::new(i32::MIN, i32::MIN);
    for center in &centers {
        let mirrored = Brush {
            center: *center,
            ..brush
        };
        let (chunk_min, chunk_max) = terrain.brush_chunk_region(&mirrored);
        minimum = ChunkCoord::new(minimum.x.min(chunk_min.x), minimum.z.min(chunk_min.z));
        maximum = ChunkCoord::new(maximum.x.max(chunk_max.x), maximum.z.max(chunk_max.z));
    }
    let previous = region_chunks(&terrain, minimum, maximum);
    let mut changed = 0;
    for center in centers {
        let mirrored = Brush { center, ..brush };
        changed += terrain
            .apply(&mirrored, operation)
            .map_err(|error| terrain_error("terrain.brush", error))?;
    }
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.paint ---------------------------------------------------------------------------

fn paint_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.paint".to_string(),
        summary: "Pinta uma camada de material no terreno (splatmap).".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required("shape", ParamKind::Text, "sphere, box, cylinder ou capsule."),
            ParamSpec::required("center", ParamKind::Vec3, "Centro do pincel no mundo."),
            ParamSpec::optional("radius", ParamKind::Float, "Raio (esfera, cilindro, cápsula)."),
            ParamSpec::optional("half_extents", ParamKind::Vec3, "Meia-extensão da caixa."),
            ParamSpec::optional("half_height", ParamKind::Float, "Meia-altura (cilindro, cápsula)."),
            ParamSpec::optional("falloff", ParamKind::Text, "constant, linear, smooth (padrão) ou gaussian."),
            ParamSpec::required("material", ParamKind::Int, "Índice da camada (0 a 255)."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste o pincel ou o material."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "shape": "sphere", "center": {"x": 0, "y": 0, "z": 0}, "radius": 4, "material": 1}"#.to_string(),
    }
}

fn paint(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: PaintPayload = parse("terrain.paint", payload)?;
    let brush = brush_from_payload(BrushArguments {
        command: "terrain.paint",
        shape: &request.shape,
        center: request.center,
        radius: request.radius,
        half_extents: request.half_extents,
        half_height: request.half_height,
        falloff: &request.falloff,
        strength: 1.0,
    })?;
    let mut terrain = read_terrain(world, request.entity)?;
    let (minimum, maximum) = terrain.brush_chunk_region(&brush);
    let previous = region_chunks(&terrain, minimum, maximum);
    let changed = terrain
        .paint(&brush, request.material)
        .map_err(|error| terrain_error("terrain.paint", error))?;
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.patch (also the undo primitive) -------------------------------------------------

fn patch_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.patch".to_string(),
        summary: "Repõe ou remove blocos de terreno (usado pelo desfazer).".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required(
                "chunks",
                ParamKind::Any,
                "Lista de blocos: {x, z, heights, materials} ou {x, z, remove: true}.",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Corrija a lista de blocos."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "chunks": [{"x": 0, "z": 0, "heights": [0.0, 1.0], "materials": [0, 1]}]}"#.to_string(),
    }
}

fn patch(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: PatchPayload = parse("terrain.patch", payload)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let mut inverse = Vec::with_capacity(request.chunks.len());
    for entry in &request.chunks {
        let coord = ChunkCoord::new(entry.x, entry.z);
        inverse.push(match terrain.capture_chunk(coord) {
            Some(chunk) => chunk_json(&chunk),
            None => removed_chunk_json(coord),
        });
        if entry.remove {
            if entry.heights.is_some() || entry.materials.is_some() {
                return Err(invalid(
                    "terrain.patch",
                    "`remove` não pode vir junto de `heights` ou `materials`",
                ));
            }
            terrain.remove_chunk(coord);
        } else {
            let heights = entry.heights.clone().ok_or_else(|| {
                invalid(
                    "terrain.patch",
                    "cada bloco precisa de `heights` ou `remove`",
                )
            })?;
            let chunk = TerrainChunk::from_parts(
                coord,
                terrain.chunk_cells(),
                heights,
                entry.materials.clone().unwrap_or_default(),
            )
            .map_err(|error| terrain_error("terrain.patch", error))?;
            terrain
                .insert_chunk(chunk)
                .map_err(|error| terrain_error("terrain.patch", error))?;
        }
    }
    write_terrain(world, request.entity, &terrain)?;
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(patch_json(request.entity, inverse)),
    })
}

// ---- terrain.holes ---------------------------------------------------------------------------

fn holes_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.holes".to_string(),
        summary: "Define os furos circulares do terreno (entradas de caverna).".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required(
                "holes",
                ParamKind::Any,
                "Lista de {x, z, radius}; vazia remove todos os furos.",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new(
                "TERRAIN_INVALID",
                "Corrija os furos (números finitos, raio positivo).",
            ),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "holes": [{"x": 2.0, "z": 3.0, "radius": 1.5}]}"#.to_string(),
    }
}

fn holes(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: HolesPayload = parse("terrain.holes", payload)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let previous: Vec<Json> = terrain
        .holes()
        .iter()
        .map(|hole| json!({ "x": hole.x, "z": hole.z, "radius": hole.radius }))
        .collect();
    let holes: Vec<TerrainHole> = request
        .holes
        .into_iter()
        .map(|hole| TerrainHole {
            x: hole.x,
            z: hole.z,
            radius: hole.radius,
        })
        .collect();
    terrain
        .set_holes(holes)
        .map_err(|error| terrain_error("terrain.holes", error))?;
    write_terrain(world, request.entity, &terrain)?;
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(CommandRecord::new(
            "terrain.holes",
            json!({ "entity": request.entity, "holes": previous }),
        )),
    })
}

// ---- surface.follow --------------------------------------------------------------------------

fn follow_spec() -> CommandSpec {
    CommandSpec {
        id: "surface.follow".to_string(),
        summary: "Re-projeta na superfície os objetos com a tabela SurfaceFollow.".to_string(),
        params: vec![ParamSpec::required(
            "terrain",
            ParamKind::Entity,
            "Id do terreno que os objetos devem seguir.",
        )],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "O terreno não pôde ser lido."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"terrain": 4}"#.to_string(),
    }
}

fn follow(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: FollowPayload = parse("surface.follow", payload)?;
    let terrain = read_terrain(world, request.terrain)?;
    let (moved, inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "moved": moved }),
        inverse,
    })
}

fn surface_set_spec() -> CommandSpec {
    CommandSpec {
        id: "surface.set".to_string(),
        summary: "Define posição e rotação de objetos (usado pelo inverso de `surface.follow`)."
            .to_string(),
        params: vec![ParamSpec::required(
            "values",
            ParamKind::Any,
            "Lista de {entity, position?, rotation_degrees?}.",
        )],
        returns: None,
        failures: vec![
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira os ids das entidades."),
            FailureSpec::new(
                "WORLD_INVALID_VALUE",
                "Corrija os valores de posição/rotação.",
            ),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"values": [{"entity": 5, "position": {"x": 0, "y": 1, "z": 0}}]}"#.to_string(),
    }
}

fn surface_set(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: SurfaceSetPayload = parse("surface.set", payload)?;
    let mut old_values = Vec::new();
    for value in request.values {
        let handle = world.handle_of(PersistentId(value.entity))?;
        let mut entry = Map::new();
        entry.insert("entity".to_string(), json!(value.entity));
        if let Some(position) = value.position {
            if !position.is_finite() {
                return Err(invalid("surface.set", "a posição precisa ser finita"));
            }
            let old = world.set(handle, "Transform", "position", Value::Vec3(position))?;
            entry.insert("position".to_string(), old.to_json());
        }
        if let Some(rotation) = value.rotation_degrees {
            if !rotation.is_finite() {
                return Err(invalid("surface.set", "a rotação precisa ser finita"));
            }
            let old = world.set(
                handle,
                "Transform",
                "rotation_degrees",
                Value::Vec3(rotation),
            )?;
            entry.insert("rotation_degrees".to_string(), old.to_json());
        }
        if entry.len() > 1 {
            old_values.push(Json::Object(entry));
        }
    }
    let inverse = if old_values.is_empty() {
        None
    } else {
        Some(CommandRecord::new(
            "surface.set",
            json!({ "values": old_values }),
        ))
    };
    Ok(Outcome {
        result: Json::Null,
        inverse,
    })
}

// ---- terrain.erode ---------------------------------------------------------------------------

fn erode_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.erode".to_string(),
        summary:
            "Erosão térmica: o material das encostas íngremes escorrega para o vizinho mais baixo."
                .to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::optional(
                "iterations",
                ParamKind::Int,
                "Iterações (1 a 64; padrão 8).",
            ),
            ParamSpec::optional(
                "talus",
                ParamKind::Float,
                "Inclinação estável em metros por célula (padrão 0,5).",
            ),
            ParamSpec::optional(
                "strength",
                ParamKind::Float,
                "Fração movida por iteração (0 a 1; padrão 0,5).",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new(
                "TERRAIN_INVALID",
                "Ajuste `iterations`, `talus` ou `strength`.",
            ),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "iterations": 8, "talus": 0.5, "strength": 0.5}"#.to_string(),
    }
}

fn erode(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: ErodePayload = parse("terrain.erode", payload)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let mut minimum: Option<ChunkCoord> = None;
    let mut maximum: Option<ChunkCoord> = None;
    for coord in terrain.chunk_coords() {
        minimum = Some(minimum.map_or(coord, |current| {
            ChunkCoord::new(current.x.min(coord.x), current.z.min(coord.z))
        }));
        maximum = Some(maximum.map_or(coord, |current| {
            ChunkCoord::new(current.x.max(coord.x), current.z.max(coord.z))
        }));
    }
    let previous = match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => region_chunks(&terrain, minimum, maximum),
        _ => Vec::new(),
    };
    let changed = terrain
        .erode(request.iterations, request.talus, request.strength)
        .map_err(|error| terrain_error("terrain.erode", error))?;
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.holes_from_caves -----------------------------------------------------------------

fn holes_from_caves_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.holes_from_caves".to_string(),
        summary: "Abre no terreno um furo em cada túnel de caverna que chega à superfície."
            .to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::optional(
                "keep_existing",
                ParamKind::Bool,
                "Mantém os furos que já existem (padrão: substitui).",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "O terreno não pôde ser lido."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4}"#.to_string(),
    }
}

fn rotate_euler(vector: Vec3, degrees: Vec3) -> Vec3 {
    let (sine_x, cosine_x) = degrees.x.to_radians().sin_cos();
    let (sine_y, cosine_y) = degrees.y.to_radians().sin_cos();
    let (sine_z, cosine_z) = degrees.z.to_radians().sin_cos();
    let x1 = vector.x * cosine_z - vector.y * sine_z;
    let y1 = vector.x * sine_z + vector.y * cosine_z;
    let z1 = vector.z;
    let y2 = y1 * cosine_x - z1 * sine_x;
    let z2 = y1 * sine_x + z1 * cosine_x;
    let x3 = x1 * cosine_y + z2 * sine_y;
    let z3 = -x1 * sine_y + z2 * cosine_y;
    Vec3::new(x3, y2, z3)
}

fn holes_from_caves(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: HolesFromCavesPayload = parse("terrain.holes_from_caves", payload)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let previous: Vec<Json> = terrain
        .holes()
        .iter()
        .map(|hole| json!({ "x": hole.x, "z": hole.z, "radius": hole.radius }))
        .collect();
    let mut holes: Vec<TerrainHole> = if request.keep_existing {
        terrain.holes().to_vec()
    } else {
        Vec::new()
    };
    let strokes = world
        .entities_with_table("SculptStroke")
        .unwrap_or_default();
    for handle in strokes {
        let mode = match world.get(handle, "SculptStroke", "mode") {
            Ok(Value::Text(mode)) => mode.clone(),
            _ => continue,
        };
        if mode != "carve" {
            continue;
        }
        let volume = match world.get(handle, "SculptStroke", "volume") {
            Ok(Value::Int(volume)) => *volume,
            _ => continue,
        };
        let Ok(cave) = world.handle_of(PersistentId(volume as u64)) else {
            continue;
        };
        let transform = match world.get(cave, "Transform", "position") {
            Ok(Value::Vec3(position)) => {
                let rotation = match world.get(cave, "Transform", "rotation_degrees") {
                    Ok(Value::Vec3(rotation)) => *rotation,
                    _ => Vec3::ZERO,
                };
                let scale = match world.get(cave, "Transform", "scale") {
                    Ok(Value::Vec3(scale)) => *scale,
                    _ => Vec3::ONE,
                };
                (*position, rotation, scale)
            }
            _ => continue,
        };
        let start = match world.get(handle, "SculptStroke", "start") {
            Ok(Value::Vec3(point)) => *point,
            _ => continue,
        };
        let end = match world.get(handle, "SculptStroke", "end") {
            Ok(Value::Vec3(point)) => *point,
            _ => continue,
        };
        let radius = match world.get(handle, "SculptStroke", "radius") {
            Ok(Value::Float(radius)) if radius.is_finite() => *radius as f32,
            _ => continue,
        };
        let local = Vec3::new(
            (start.x + end.x) * 0.5 * transform.2.x,
            (start.y + end.y) * 0.5 * transform.2.y,
            (start.z + end.z) * 0.5 * transform.2.z,
        );
        let world_center = transform.0 + rotate_euler(local, transform.1);
        let world_radius = radius
            * transform
                .2
                .x
                .abs()
                .max(transform.2.y.abs())
                .max(transform.2.z.abs());
        if world_radius <= 0.0 {
            continue;
        }
        let Some(height) = terrain.height_at(world_center.x, world_center.z) else {
            continue;
        };
        if (height - world_center.y).abs() > world_radius {
            continue;
        }
        let candidate = TerrainHole {
            x: world_center.x,
            z: world_center.z,
            radius: world_radius * 0.8,
        };
        if holes.iter().any(|hole| {
            let offset_x = hole.x - candidate.x;
            let offset_z = hole.z - candidate.z;
            offset_x * offset_x + offset_z * offset_z
                < (hole.radius.max(candidate.radius) * 0.5).powi(2)
        }) {
            continue;
        }
        holes.push(candidate);
    }
    terrain
        .set_holes(holes.clone())
        .map_err(|error| terrain_error("terrain.holes_from_caves", error))?;
    write_terrain(world, request.entity, &terrain)?;
    Ok(Outcome {
        result: json!({ "holes": holes.len() }),
        inverse: Some(CommandRecord::new(
            "terrain.holes",
            json!({ "entity": request.entity, "holes": previous }),
        )),
    })
}

// ---- terrain.stamp ---------------------------------------------------------------------------

fn stamp_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.stamp".to_string(),
        summary: "Carimba um retângulo de alturas no terreno (importar uma montanha, uma cratera)."
            .to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required("x", ParamKind::Float, "Centro x no mundo."),
            ParamSpec::required("z", ParamKind::Float, "Centro z no mundo."),
            ParamSpec::required("width", ParamKind::Int, "Células na largura (1 a 4096)."),
            ParamSpec::required("depth", ParamKind::Int, "Células na profundidade (1 a 4096)."),
            ParamSpec::required(
                "heights",
                ParamKind::Any,
                "Alturas em linha, uma por célula (`width * depth`).",
            ),
            ParamSpec::optional(
                "mode",
                ParamKind::Text,
                "replace (padrão), add, subtract, min ou max.",
            ),
            ParamSpec::optional("strength", ParamKind::Float, "Fator de add/subtract (padrão 1)."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste o carimbo (tamanho, alturas, modo)."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "x": 0, "z": 0, "width": 2, "depth": 2, "heights": [1, 2, 3, 4], "mode": "replace"}"#.to_string(),
    }
}

fn stamp_mode(name: &str) -> Result<StampMode, CommandError> {
    match name {
        "replace" => Ok(StampMode::Replace),
        "add" => Ok(StampMode::Add),
        "subtract" => Ok(StampMode::Subtract),
        "min" => Ok(StampMode::Min),
        "max" => Ok(StampMode::Max),
        other => Err(invalid(
            "terrain.stamp",
            format!("modo desconhecido `{other}`; use replace, add, subtract, min ou max"),
        )),
    }
}

fn stamp(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: StampPayload = parse("terrain.stamp", payload)?;
    let mode = stamp_mode(&request.mode)?;
    let width = usize::try_from(request.width).unwrap_or(0);
    let depth = usize::try_from(request.depth).unwrap_or(0);
    let mut terrain = read_terrain(world, request.entity)?;
    let cell = terrain.cell_size();
    let cells = terrain.chunk_cells() as f32 * cell;
    let span_x = width as f32 * cell;
    let span_z = depth as f32 * cell;
    let minimum = ChunkCoord::new(
        ((request.x - span_x * 0.5) / cells).floor() as i32,
        ((request.z - span_z * 0.5) / cells).floor() as i32,
    );
    let maximum = ChunkCoord::new(
        ((request.x + span_x * 0.5) / cells).floor() as i32,
        ((request.z + span_z * 0.5) / cells).floor() as i32,
    );
    let previous = region_chunks(&terrain, minimum, maximum);
    let changed = terrain
        .stamp(Stamp {
            center_x: request.x,
            center_z: request.z,
            width,
            depth,
            heights: &request.heights,
            mode,
            strength: request.strength,
        })
        .map_err(|error| terrain_error("terrain.stamp", error))?;
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.erode_hydraulic -----------------------------------------------------------------

fn hydraulic_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.erode_hydraulic".to_string(),
        summary: "Erosão hidráulica por gotas determinísticas: sulcos e depósitos de água."
            .to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::optional(
                "droplets",
                ParamKind::Int,
                "Gotas (1 a 100000; padrão 2000).",
            ),
            ParamSpec::optional(
                "strength",
                ParamKind::Float,
                "Força da escavação (0 a 1; padrão 0,5).",
            ),
            ParamSpec::optional(
                "deposition",
                ParamKind::Float,
                "Deposição (0 a 1; padrão 0,3).",
            ),
            ParamSpec::optional(
                "evaporation",
                ParamKind::Float,
                "Evaporação (0 a 1; padrão 0,02).",
            ),
            ParamSpec::optional("seed", ParamKind::Int, "Semente (padrão 0)."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("TERRAIN_INVALID", "Ajuste as gotas ou os fatores."),
            FailureSpec::new("WORLD_UNKNOWN_ENTITY", "Confira o id do terreno."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "droplets": 2000, "strength": 0.5, "deposition": 0.3}"#
            .to_string(),
    }
}

fn hydraulic(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: HydraulicPayload = parse("terrain.erode_hydraulic", payload)?;
    let mut terrain = read_terrain(world, request.entity)?;
    let mut minimum: Option<ChunkCoord> = None;
    let mut maximum: Option<ChunkCoord> = None;
    for coord in terrain.chunk_coords() {
        minimum = Some(minimum.map_or(coord, |current| {
            ChunkCoord::new(current.x.min(coord.x), current.z.min(coord.z))
        }));
        maximum = Some(maximum.map_or(coord, |current| {
            ChunkCoord::new(current.x.max(coord.x), current.z.max(coord.z))
        }));
    }
    let previous = match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => region_chunks(&terrain, minimum, maximum),
        _ => Vec::new(),
    };
    let changed = terrain
        .hydraulic_erode(
            request.droplets,
            request.strength,
            request.deposition,
            request.evaporation,
            request.seed,
        )
        .map_err(|error| terrain_error("terrain.erode_hydraulic", error))?;
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

// ---- terrain.stamp_mesh ----------------------------------------------------------------------

fn stamp_mesh_spec() -> CommandSpec {
    CommandSpec {
        id: "terrain.stamp_mesh".to_string(),
        summary: "Carimba a superfície de uma malha 3D no terreno, na posição da malha."
            .to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Terreno."),
            ParamSpec::required("mesh", ParamKind::Entity, "Objeto com a tabela `Mesh`."),
            ParamSpec::optional(
                "mode",
                ParamKind::Text,
                "replace (padrão), add, subtract, min ou max.",
            ),
            ParamSpec::optional(
                "strength",
                ParamKind::Float,
                "Fator de add/subtract (padrão 1).",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new(
                "TERRAIN_INVALID",
                "A malha não tem triângulos ou o modo é inválido.",
            ),
            FailureSpec::new(
                "WORLD_UNKNOWN_ENTITY",
                "Confira os ids do terreno e da malha.",
            ),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Budgeted,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 4, "mesh": 9, "mode": "replace"}"#.to_string(),
    }
}

fn mesh_height_at(triangles: &[(Vec3, Vec3, Vec3)], x: f32, z: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for (a, b, c) in triangles {
        let determinant = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
        if determinant.abs() < 1.0e-9 {
            continue;
        }
        let first = ((b.z - c.z) * (x - c.x) + (c.x - b.x) * (z - c.z)) / determinant;
        let second = ((c.z - a.z) * (x - c.x) + (a.x - c.x) * (z - c.z)) / determinant;
        let third = 1.0 - first - second;
        if first >= -1.0e-5 && second >= -1.0e-5 && third >= -1.0e-5 {
            let height = first * a.y + second * b.y + third * c.y;
            best = Some(best.map_or(height, |current| current.max(height)));
        }
    }
    best
}

fn stamp_mesh(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: StampMeshPayload = parse("terrain.stamp_mesh", payload)?;
    let mode = stamp_mode(&request.mode)?;
    let mesh_handle = world.handle_of(PersistentId(request.mesh))?;
    let geometry = match world.get(mesh_handle, "Mesh", "geometry") {
        Ok(Value::Mesh(geometry)) => geometry.clone(),
        _ => {
            return Err(invalid(
                "terrain.stamp_mesh",
                "o objeto precisa de uma tabela `Mesh` com geometria",
            ));
        }
    };
    let transform = (
        match world.get(mesh_handle, "Transform", "position") {
            Ok(Value::Vec3(position)) => *position,
            _ => Vec3::ZERO,
        },
        match world.get(mesh_handle, "Transform", "rotation_degrees") {
            Ok(Value::Vec3(rotation)) => *rotation,
            _ => Vec3::ZERO,
        },
        match world.get(mesh_handle, "Transform", "scale") {
            Ok(Value::Vec3(scale)) => *scale,
            _ => Vec3::ONE,
        },
    );
    let to_world = |point: Vec3| {
        transform.0
            + rotate_euler(
                Vec3::new(
                    point.x * transform.2.x,
                    point.y * transform.2.y,
                    point.z * transform.2.z,
                ),
                transform.1,
            )
    };
    let triangles: Vec<(Vec3, Vec3, Vec3)> = geometry
        .triangles()
        .into_iter()
        .filter_map(|indices| {
            let a = geometry.positions.get(indices[0] as usize)?;
            let b = geometry.positions.get(indices[1] as usize)?;
            let c = geometry.positions.get(indices[2] as usize)?;
            Some((to_world(*a), to_world(*b), to_world(*c)))
        })
        .collect();
    if triangles.is_empty() {
        return Err(invalid("terrain.stamp_mesh", "a malha não tem triângulos"));
    }
    let mut minimum_x = f32::INFINITY;
    let mut maximum_x = f32::NEG_INFINITY;
    let mut minimum_z = f32::INFINITY;
    let mut maximum_z = f32::NEG_INFINITY;
    for (a, b, c) in &triangles {
        for point in [a, b, c] {
            minimum_x = minimum_x.min(point.x);
            maximum_x = maximum_x.max(point.x);
            minimum_z = minimum_z.min(point.z);
            maximum_z = maximum_z.max(point.z);
        }
    }
    let mut terrain = read_terrain(world, request.entity)?;
    let cell = terrain.cell_size();
    let chunk_side = terrain.chunk_cells() as f32 * cell;
    let previous = region_chunks(
        &terrain,
        ChunkCoord::new(
            (minimum_x / chunk_side).floor() as i32,
            (minimum_z / chunk_side).floor() as i32,
        ),
        ChunkCoord::new(
            (maximum_x / chunk_side).floor() as i32,
            (maximum_z / chunk_side).floor() as i32,
        ),
    );
    let first_x = (minimum_x / cell).floor() as i64;
    let last_x = (maximum_x / cell).ceil() as i64;
    let first_z = (minimum_z / cell).floor() as i64;
    let last_z = (maximum_z / cell).ceil() as i64;
    let width = last_x.saturating_sub(first_x).saturating_add(1);
    let depth = last_z.saturating_sub(first_z).saturating_add(1);
    if width.saturating_mul(depth) > MAX_BRUSH_SAMPLES {
        return Err(invalid(
            "terrain.stamp_mesh",
            "a malha cobre amostras demais; reduza o tamanho",
        ));
    }
    let mut changed = 0;
    for grid_z in first_z..=last_z {
        for grid_x in first_x..=last_x {
            let Some(current) = terrain.sample(grid_x, grid_z) else {
                continue;
            };
            let world_x = grid_x as f32 * cell;
            let world_z = grid_z as f32 * cell;
            let Some(height) = mesh_height_at(&triangles, world_x, world_z) else {
                continue;
            };
            let next = match mode {
                StampMode::Replace => height,
                StampMode::Add => current + height * request.strength,
                StampMode::Subtract => current - height * request.strength,
                StampMode::Min => current.min(height),
                StampMode::Max => current.max(height),
            };
            if (next - current).abs() > 1.0e-6 {
                terrain
                    .set_sample(grid_x, grid_z, next)
                    .map_err(|error| terrain_error("terrain.stamp_mesh", error))?;
                changed += 1;
            }
        }
    }
    write_terrain(world, request.entity, &terrain)?;
    let (_, follow_inverse) = apply_surface_updates(world, &terrain)?;
    Ok(Outcome {
        result: json!({ "changed": changed }),
        inverse: Some(terrain_inverse(request.entity, previous, follow_inverse)),
    })
}

fn parse<T: serde::de::DeserializeOwned>(command: &str, payload: &Json) -> Result<T, CommandError> {
    let source = if payload.is_null() {
        json!({})
    } else {
        payload.clone()
    };
    serde_json::from_value(source).map_err(|error| invalid(command, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dispatcher;
    use crate::query::{QueryRecord, run_query};
    use petunia_math::{Color, PolygonMesh};
    use petunia_terrain::{LAYER_ROCK, LAYER_SAND};
    use petunia_world::{TableSpec, ValueKind};

    fn world() -> World {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Name").column(
                "text",
                ValueKind::Text,
                Value::Text(String::new()),
            ))
            .expect("Name");
        world
            .define_table(TableSpec::new(TERRAIN_TABLE).column(
                TERRAIN_COLUMN,
                ValueKind::Terrain,
                Value::Terrain(Terrain::default()),
            ))
            .expect("Terrain");
        world
            .define_table(
                TableSpec::new("Transform")
                    .column("position", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("scale", ValueKind::Vec3, Value::Vec3(Vec3::ONE)),
            )
            .expect("Transform");
        world
            .define_table(
                TableSpec::new("SurfaceFollow")
                    .column("offset", ValueKind::Float, Value::Float(0.0))
                    .column("align_normal", ValueKind::Bool, Value::Bool(false)),
            )
            .expect("SurfaceFollow");
        world
            .define_table(
                TableSpec::new("Mesh")
                    .column(
                        "geometry",
                        ValueKind::Mesh,
                        Value::Mesh(PolygonMesh::default()),
                    )
                    .column("color", ValueKind::Color, Value::Color(Color::WHITE)),
            )
            .expect("Mesh");
        world
            .define_table(
                TableSpec::new("SculptStroke")
                    .column("volume", ValueKind::Int, Value::Int(0))
                    .column("mode", ValueKind::Text, Value::Text("carve".to_string()))
                    .column("start", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("end", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("radius", ValueKind::Float, Value::Float(1.0))
                    .column("smooth", ValueKind::Float, Value::Float(0.0))
                    .column("strength", ValueKind::Float, Value::Float(0.0)),
            )
            .expect("SculptStroke");
        world
    }

    fn dispatcher() -> Dispatcher {
        Dispatcher::with_builtin().expect("dispatcher")
    }

    fn run(world: &mut World, dispatcher: &mut Dispatcher, command: &str, payload: Json) -> Json {
        dispatcher
            .execute(world, &CommandRecord::new(command, payload))
            .expect("command")
    }

    fn terrain_of(world: &World, entity: u64) -> Terrain {
        let handle = world.handle_of(PersistentId(entity)).expect("handle");
        match world
            .get(handle, TERRAIN_TABLE, TERRAIN_COLUMN)
            .expect("row")
        {
            Value::Terrain(terrain) => terrain.clone(),
            other => panic!("expected terrain, got {:?}", other.kind()),
        }
    }

    #[test]
    fn create_generate_brush_and_undo_round_trip() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(
            &mut world,
            &mut dispatcher,
            "terrain.create",
            json!({ "cell_size": 1.0, "chunk_cells": 16 }),
        );
        let entity = made["entity"].as_u64().expect("entity");
        assert!(terrain_of(&world, entity).is_empty());

        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({
                "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                "seed": 3, "amplitude": 5.0
            }),
        );
        assert_eq!(terrain_of(&world, entity).chunk_count(), 1);

        let surface = terrain_of(&world, entity).sample(4, 4).expect("height");
        let raised = run(
            &mut world,
            &mut dispatcher,
            "terrain.brush",
            json!({
                "entity": entity, "shape": "sphere",
                "center": {"x": 4.0, "y": surface, "z": 4.0},
                "radius": 2.0, "falloff": "constant", "strength": 2.0,
                "operation": "raise"
            }),
        );
        assert!(raised["changed"].as_u64().unwrap_or(0) > 0);
        let before = terrain_of(&world, entity).sample(4, 4).expect("height");

        dispatcher.undo(&mut world).expect("undo brush");
        let after = terrain_of(&world, entity).sample(4, 4).expect("height");
        assert!((before - after - 2.0).abs() < 1.0e-4);

        dispatcher.redo(&mut world).expect("redo brush");
        let again = terrain_of(&world, entity).sample(4, 4).expect("height");
        assert!((again - before).abs() < 1.0e-4);
    }

    #[test]
    fn create_is_undoable() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        assert!(world.handle_of(PersistentId(entity)).is_ok());
        dispatcher.undo(&mut world).expect("undo");
        assert!(world.handle_of(PersistentId(entity)).is_err());
        dispatcher.redo(&mut world).expect("redo");
        assert!(world.handle_of(PersistentId(entity)).is_ok());
    }

    #[test]
    fn paint_changes_materials_and_undo_restores_them() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0 }),
        );
        let surface = terrain_of(&world, entity).sample(4, 4).expect("height");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.paint",
            json!({
                "entity": entity, "shape": "sphere",
                "center": {"x": 4.0, "y": surface, "z": 4.0},
                "radius": 2.0, "material": 4
            }),
        );
        assert_eq!(terrain_of(&world, entity).sample_material(4, 4), Some(4));
        dispatcher.undo(&mut world).expect("undo paint");
        assert_eq!(terrain_of(&world, entity).sample_material(4, 4), Some(0));
    }

    #[test]
    fn hostile_brushes_are_refused_without_touching_the_terrain() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        let before = terrain_of(&world, entity);

        let bad_shape = dispatcher.execute(
            &mut world,
            &CommandRecord::new(
                "terrain.brush",
                json!({
                    "entity": entity, "shape": "teapot",
                    "center": {"x": 0.0, "y": 0.0, "z": 0.0},
                    "operation": "raise"
                }),
            ),
        );
        assert!(bad_shape.is_err());

        let bad_falloff = dispatcher.execute(
            &mut world,
            &CommandRecord::new(
                "terrain.brush",
                json!({
                    "entity": entity, "shape": "sphere",
                    "center": {"x": 0.0, "y": 0.0, "z": 0.0}, "radius": 1.0,
                    "falloff": "cubic", "operation": "raise"
                }),
            ),
        );
        assert!(bad_falloff.is_err());
        assert_eq!(terrain_of(&world, entity), before);
    }

    #[test]
    fn erosion_lowers_a_spike_and_undo_restores_it() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        run(
            &mut world,
            &mut dispatcher,
            "terrain.brush",
            json!({
                "entity": entity, "shape": "sphere",
                "center": {"x": 4.0, "y": 0.0, "z": 4.0}, "radius": 1.0,
                "falloff": "constant", "strength": 5.0, "operation": "raise"
            }),
        );
        let raised = terrain_of(&world, entity).sample(4, 4).expect("height");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.erode",
            json!({ "entity": entity, "iterations": 4, "talus": 0.5, "strength": 0.5 }),
        );
        let eroded = terrain_of(&world, entity).sample(4, 4).expect("height");
        assert!(eroded < raised, "raised {raised}, eroded {eroded}");
        dispatcher.undo(&mut world).expect("undo erode");
        let restored = terrain_of(&world, entity).sample(4, 4).expect("height");
        assert!((restored - raised).abs() < 1.0e-5);
    }

    #[test]
    fn a_mirrored_brush_changes_both_sides_and_undoes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": -1, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        run(
            &mut world,
            &mut dispatcher,
            "terrain.brush",
            json!({
                "entity": entity, "shape": "sphere",
                "center": {"x": 3.0, "y": 0.0, "z": 3.0}, "radius": 1.0,
                "falloff": "constant", "strength": 1.0, "operation": "raise",
                "mirror_x": true
            }),
        );
        let terrain = terrain_of(&world, entity);
        assert!(terrain.sample(3, 3).unwrap_or(0.0) > 0.5);
        assert!(terrain.sample(-3, 3).unwrap_or(0.0) > 0.5);
        dispatcher.undo(&mut world).expect("undo mirror");
        let terrain = terrain_of(&world, entity);
        assert!(terrain.sample(3, 3).unwrap_or(0.0).abs() < 1.0e-5);
        assert!(terrain.sample(-3, 3).unwrap_or(0.0).abs() < 1.0e-5);
    }

    #[test]
    fn hydraulic_erosion_changes_a_spike_and_undoes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        run(
            &mut world,
            &mut dispatcher,
            "terrain.brush",
            json!({
                "entity": terrain, "shape": "sphere",
                "center": {"x": 4.0, "y": 0.0, "z": 4.0}, "radius": 1.0,
                "falloff": "constant", "strength": 5.0, "operation": "raise"
            }),
        );
        let raised = terrain_of(&world, terrain).sample(4, 4).unwrap_or(0.0);
        let made = run(
            &mut world,
            &mut dispatcher,
            "terrain.erode_hydraulic",
            json!({ "entity": terrain, "droplets": 500, "strength": 0.5, "deposition": 0.3, "seed": 7 }),
        );
        assert!(made["changed"].as_u64().unwrap_or(0) > 0, "{made}");
        let eroded = terrain_of(&world, terrain).sample(4, 4).unwrap_or(0.0);
        assert!(eroded < raised, "{raised} -> {eroded}");
        dispatcher.undo(&mut world).expect("undo hydraulic");
        assert!((terrain_of(&world, terrain).sample(4, 4).unwrap_or(0.0) - raised).abs() < 1.0e-5);
    }

    #[test]
    fn stamp_applies_a_height_patch_and_undoes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        let made = run(
            &mut world,
            &mut dispatcher,
            "terrain.stamp",
            json!({
                "entity": terrain, "x": 0.5, "z": 0.5, "width": 2, "depth": 2,
                "heights": [1.0, 2.0, 3.0, 4.0], "mode": "replace"
            }),
        );
        assert_eq!(made["changed"], 4);
        assert!((terrain_of(&world, terrain).sample(0, 0).unwrap() - 1.0).abs() < 1.0e-6);
        dispatcher.undo(&mut world).expect("undo stamp");
        assert!(terrain_of(&world, terrain).sample(0, 0).unwrap().abs() < 1.0e-6);
    }

    #[test]
    fn cave_tunnels_near_the_surface_become_terrain_holes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        let cave = world.spawn().expect("cave");
        world
            .insert_row_json(
                cave,
                "Transform",
                &json!({ "position": { "x": 2.0, "y": 0.0, "z": 2.0 } })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            )
            .expect("cave transform");
        let cave_id = world.persistent_id(cave).expect("id").0 as i64;
        let stroke = world.spawn().expect("stroke");
        world
            .insert_row_json(
                stroke,
                "SculptStroke",
                &json!({
                    "volume": cave_id,
                    "mode": "carve",
                    "start": { "x": 0.0, "y": 0.0, "z": 0.0 },
                    "end": { "x": 0.0, "y": 0.0, "z": 0.0 },
                    "radius": 2.0
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            )
            .expect("stroke row");

        let made = run(
            &mut world,
            &mut dispatcher,
            "terrain.holes_from_caves",
            json!({ "entity": terrain }),
        );
        assert_eq!(made["holes"], 1, "{made}");
        let holes = terrain_of(&world, terrain).holes().to_vec();
        assert_eq!(holes.len(), 1);
        assert!((holes[0].x - 2.0).abs() < 0.01);
        assert!((holes[0].z - 2.0).abs() < 0.01);
        dispatcher.undo(&mut world).expect("undo holes");
        assert!(terrain_of(&world, terrain).holes().is_empty());
    }

    #[test]
    fn a_terrain_brush_moves_following_objects_in_one_undo() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        let object = world.spawn().expect("object");
        world
            .insert_row_json(
                object,
                "Transform",
                &json!({ "position": { "x": 4.0, "y": 5.0, "z": 4.0 } })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            )
            .expect("transform");
        world
            .insert_row_values(
                object,
                "SurfaceFollow",
                vec![Value::Float(0.0), Value::Bool(false)],
            )
            .expect("follow");

        run(
            &mut world,
            &mut dispatcher,
            "terrain.brush",
            json!({
                "entity": terrain, "shape": "sphere",
                "center": {"x": 4.0, "y": 0.0, "z": 4.0}, "radius": 2.0,
                "falloff": "constant", "strength": 1.0, "operation": "raise"
            }),
        );
        let height = terrain_of(&world, terrain).sample(4, 4).unwrap_or(0.0);
        match world.get(object, "Transform", "position") {
            Ok(Value::Vec3(position)) => {
                assert!(
                    (position.y - height).abs() < 0.01,
                    "{position:?} vs {height}"
                );
            }
            other => panic!("expected position, got {other:?}"),
        }
        dispatcher.undo(&mut world).expect("one undo");
        assert!(
            terrain_of(&world, terrain)
                .sample(4, 4)
                .unwrap_or(0.0)
                .abs()
                < 1.0e-5
        );
        match world.get(object, "Transform", "position") {
            Ok(Value::Vec3(position)) => assert!((position.y - 5.0).abs() < 0.01),
            other => panic!("expected position, got {other:?}"),
        }
    }

    #[test]
    fn surface_follow_moves_objects_to_the_terrain_and_undoes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        let object = world.spawn().expect("object");
        world
            .insert_row_json(
                object,
                "Transform",
                &json!({ "position": { "x": 2.0, "y": 5.0, "z": 2.0 } })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            )
            .expect("transform");
        world
            .insert_row_values(
                object,
                "SurfaceFollow",
                vec![Value::Float(1.5), Value::Bool(false)],
            )
            .expect("follow");
        let id = world.persistent_id(object).expect("id").0;

        let result = run(
            &mut world,
            &mut dispatcher,
            "surface.follow",
            json!({ "terrain": terrain }),
        );
        assert_eq!(result["moved"], 1);
        match world.get(object, "Transform", "position") {
            Ok(Value::Vec3(position)) => assert!((position.y - 1.5).abs() < 0.01),
            other => panic!("expected position, got {other:?}"),
        }
        dispatcher.undo(&mut world).expect("undo follow");
        match world.get(object, "Transform", "position") {
            Ok(Value::Vec3(position)) => assert!((position.y - 5.0).abs() < 0.01),
            other => panic!("expected position, got {other:?}"),
        }
        let _ = id;
    }

    #[test]
    fn mesh_stamp_copies_the_top_surface_and_undoes() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let terrain = run(&mut world, &mut dispatcher, "terrain.create", json!({}))["entity"]
            .as_u64()
            .expect("terrain");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": terrain, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "amplitude": 0.0 }),
        );
        let mesh = world.spawn().expect("mesh");
        world
            .insert_row_values(
                mesh,
                "Mesh",
                vec![
                    Value::Mesh(PolygonMesh {
                        positions: vec![
                            Vec3::new(0.0, 2.0, 0.0),
                            Vec3::new(2.0, 2.0, 0.0),
                            Vec3::new(0.0, 2.0, 2.0),
                        ],
                        faces: vec![vec![0, 1, 2]],
                    }),
                    Value::Color(Color::WHITE),
                ],
            )
            .expect("mesh row");
        let mesh_id = world.persistent_id(mesh).expect("id").0;
        let made = run(
            &mut world,
            &mut dispatcher,
            "terrain.stamp_mesh",
            json!({ "entity": terrain, "mesh": mesh_id, "mode": "replace" }),
        );
        assert!(made["changed"].as_u64().unwrap_or(0) > 0, "{made}");
        let height = terrain_of(&world, terrain);
        assert!((height.sample(0, 0).unwrap_or(0.0) - 2.0).abs() < 0.01);
        assert!((height.sample(1, 1).unwrap_or(0.0) - 2.0).abs() < 0.01);
        assert!(height.sample(4, 4).unwrap_or(0.0).abs() < 0.01);
        dispatcher.undo(&mut world).expect("undo mesh stamp");
        assert!(
            terrain_of(&world, terrain)
                .sample(0, 0)
                .unwrap_or(0.0)
                .abs()
                < 1.0e-5
        );
    }

    #[test]
    fn holes_are_undoable() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.holes",
            json!({ "entity": entity, "holes": [{ "x": 1.0, "z": 2.0, "radius": 1.5 }] }),
        );
        assert_eq!(terrain_of(&world, entity).holes().len(), 1);
        dispatcher.undo(&mut world).expect("undo holes");
        assert!(terrain_of(&world, entity).holes().is_empty());
        dispatcher.redo(&mut world).expect("redo holes");
        assert_eq!(terrain_of(&world, entity).holes().len(), 1);
    }

    #[test]
    fn terrain_queries_return_height_and_info() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0 }),
        );
        let height = run_query(
            &world,
            &QueryRecord::new(
                "terrain.height",
                json!({ "entity": entity, "x": 4.0, "z": 4.0 }),
            ),
        )
        .expect("height");
        assert!(height["height"].is_number());
        assert!(height["normal"]["y"].as_f64().unwrap_or(0.0) > 0.0);
        let info = run_query(
            &world,
            &QueryRecord::new("terrain.info", json!({ "entity": entity })),
        )
        .expect("info");
        assert_eq!(info["chunk_count"], 1);
        assert_eq!(info["sample_count"], 256);
    }

    #[test]
    fn the_heights_query_reads_a_grid_that_can_be_stamped_back() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "seed": 7 }),
        );
        // The whole terrain by default, and a smaller patch when asked.
        let all = run_query(
            &world,
            &QueryRecord::new("terrain.heights", json!({ "entity": entity })),
        )
        .expect("heights");
        // A 16-cell chunk spans 17 samples (the border cells are shared with the neighbour).
        assert_eq!(all["width"], 17);
        assert_eq!(all["depth"], 17);
        assert_eq!(all["heights"].as_array().map(Vec::len), Some(289));
        let patch = run_query(
            &world,
            &QueryRecord::new(
                "terrain.heights",
                json!({ "entity": entity, "width": 2, "depth": 3 }),
            ),
        )
        .expect("patch");
        assert_eq!(patch["width"], 2);
        assert_eq!(patch["depth"], 3);
        assert_eq!(patch["heights"].as_array().map(Vec::len), Some(6));
        // A grid too wide for the format is clamped, never an error.
        let clamped = run_query(
            &world,
            &QueryRecord::new(
                "terrain.heights",
                json!({ "entity": entity, "width": 9000, "depth": 1 }),
            ),
        )
        .expect("clamped");
        assert_eq!(clamped["width"], MAX_HEIGHTMAP_SIDE);
    }

    #[test]
    fn patch_can_remove_and_restore_a_chunk() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({ "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0 }),
        );
        let chunk = terrain_of(&world, entity)
            .capture_chunk(ChunkCoord::new(0, 0))
            .expect("chunk");
        let patch = json!({
            "entity": entity,
            "chunks": [{ "x": 0, "z": 0, "remove": true }]
        });
        run(&mut world, &mut dispatcher, "terrain.patch", patch);
        assert_eq!(terrain_of(&world, entity).chunk_count(), 0);
        dispatcher.undo(&mut world).expect("undo patch");
        let restored = terrain_of(&world, entity)
            .capture_chunk(ChunkCoord::new(0, 0))
            .expect("chunk back");
        assert_eq!(restored, chunk);
    }

    #[test]
    fn generate_paints_the_biome_and_reports_it() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        let result = run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({
                "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                "seed": 4, "amplitude": 6.0, "biome": "desert"
            }),
        );
        assert_eq!(result["biome"], json!("desert"));
        let terrain = terrain_of(&world, entity);
        let chunk = terrain.capture_chunk(ChunkCoord::new(0, 0)).expect("chunk");
        assert!(
            chunk
                .materials()
                .iter()
                .all(|material| *material == LAYER_SAND || *material == LAYER_ROCK),
            "o deserto só pinta areia e rocha"
        );

        // Without a biome the seed chooses one, and the same seed always chooses the same.
        let first = run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({
                "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "seed": 9
            }),
        );
        let second = run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({
                "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0, "seed": 9
            }),
        );
        assert!(first["biome"].is_string());
        assert_eq!(first["biome"], second["biome"]);

        // `none` keeps a plain heightfield: every sample on the first layer.
        run(
            &mut world,
            &mut dispatcher,
            "terrain.generate",
            json!({
                "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                "seed": 9, "biome": "none"
            }),
        );
        let chunk = terrain_of(&world, entity)
            .capture_chunk(ChunkCoord::new(0, 0))
            .expect("chunk");
        assert!(chunk.materials().iter().all(|material| *material == 0));
    }

    #[test]
    fn an_unknown_biome_is_rejected_without_touching_the_terrain() {
        let mut world = world();
        let mut dispatcher = dispatcher();
        let made = run(&mut world, &mut dispatcher, "terrain.create", json!({}));
        let entity = made["entity"].as_u64().expect("entity");
        let before = terrain_of(&world, entity);
        let error = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "terrain.generate",
                    json!({
                        "entity": entity, "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                        "biome": "tundra"
                    }),
                ),
            )
            .expect_err("bioma desconhecido");
        let message = format!("{error:?}");
        assert!(message.contains("tundra"), "{message}");
        assert_eq!(terrain_of(&world, entity), before);
    }
}
