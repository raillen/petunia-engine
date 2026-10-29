//! Caves ("cavernas") in the editor: create a block of rock, then shape it.
//!
//! - **Sculpt (B):** drag over the rock. The mode decides what a drag does: carve, add rock,
//!   inflate, flatten or smooth; Ctrl does the opposite (carve ↔ add, inflate ↔ deflate).
//! - **Path (Shift+B):** click points on the rock; Enter digs a smooth tunnel through them.
//! - **Generate:** rooms joined by winding tunnels from a seed; **consolidate:** merges the many
//!   small capsules of long drags into fewer ones.
//!
//! A drag is previewed live and saved as one batch of `SculptStroke` rows: one undo step. Strokes
//! live in the cave's own coordinates, so moving, turning or scaling the cave carries its tunnels
//! along. The baked field and the chunk meshes live in a [`SculptCache`] kept by the session.

use std::cell::RefCell;

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_math::{Color, PolygonMesh, Transform, Vec3};
use petunia_mesh::BooleanOperation;
use petunia_render::{
    OverlayItem, Ray, SculptCache, SculptItem, SculptVolume, SurfaceHit, extract_sculpt_volumes,
    hit_sculpt,
};
use petunia_sculpt::{
    GenerateParams, MAX_SMOOTH, MAX_STAMP_TRIANGLES, MAX_STROKES, StampOperation, Stroke,
    StrokeMode, consolidate, generate_cave, tunnel_strokes,
};
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::{Tool, inverse_rotate, rotate_vector, rotation_from_euler};
use crate::session::{EditorSession, vec3_of, vector_json};
use crate::tools::Pointer;

/// Brush radius limits (meters) and the default.
const MIN_SCULPT_RADIUS: f32 = 0.25;
const MAX_SCULPT_RADIUS: f32 = 16.0;
pub(crate) const DEFAULT_SCULPT_RADIUS: f32 = 1.5;
const DEFAULT_FORCE: f32 = 0.5;
/// A new dab is added once the mouse has moved this fraction of the radius.
const DAB_SPACING: f32 = 0.4;
/// Carving digs this fraction of the radius into the surface, so dragging opens a tunnel.
const CARVE_DEPTH: f32 = 0.4;
/// Strokes blend over this fraction of their radius (up to the blend limit).
const SMOOTH_RATIO: f32 = 0.5;
/// Inflating moves the surface this fraction of the radius per dab at full force.
const INFLATE_RATIO: f32 = 0.5;
/// One drag keeps at most this many dabs (one batch).
const MAX_DABS_PER_DRAG: usize = 256;
/// Path points keep the tunnel this fraction of the radius inside the rock.
const PATH_DEPTH: f32 = 0.4;
/// A path has at most this many points.
const MAX_PATH_POINTS: usize = 64;
/// The cursor ring.
const RING_SEGMENTS: usize = 40;
const RING_COLOR: Color = Color::new(1.0, 0.82, 0.25, 1.0);
const PATH_COLOR: Color = Color::new(0.35, 0.85, 1.0, 1.0);
/// Consolidation keeps the path within this fraction of a lattice cell.
const CONSOLIDATE_TOLERANCE: f32 = 0.25;
/// Size of a generated cave's block (meters).
const GENERATED_SIZE: Vec3 = Vec3::new(32.0, 8.0, 32.0);

/// A drag of the sculpting tool: the dabs not yet saved.
pub(crate) struct SculptDrag {
    volume: u64,
    mode: StrokeMode,
    /// Ctrl was held at the start: the opposite of the mode.
    inverted: bool,
    strokes: Vec<Stroke>,
    /// Last dab, in the volume's coordinates.
    last: Option<Vec3>,
    /// Flatten keeps the plane of the first dab for the whole drag.
    plane_normal: Option<Vec3>,
}

/// A tunnel being drawn point by point.
pub(crate) struct PathState {
    volume: u64,
    points: Vec<Vec3>,
    inverted: bool,
}

/// Where the next dab lands: a point of the rock surface and its normal (world space).
#[derive(Clone, Copy, Debug)]
pub(crate) struct SculptCursor {
    point: Vec3,
    normal: Vec3,
}

/// Per-session state of the sculpting tools (kept out of `EditorSession` to keep it readable).
pub(crate) struct SculptState {
    pub drag: Option<SculptDrag>,
    pub path: Option<PathState>,
    pub cursor: Option<SculptCursor>,
    pub radius: f32,
    pub mode: StrokeMode,
    /// 0..1: how hard flatten and smooth press, and how far inflate moves the surface.
    pub force: f32,
    /// Mirror every stroke across the cave's local x = 0.
    pub mirror_x: bool,
    /// Baked fields and meshes, kept between frames.
    pub cache: RefCell<SculptCache>,
}

impl SculptState {
    pub fn new() -> Self {
        Self {
            drag: None,
            path: None,
            cursor: None,
            radius: DEFAULT_SCULPT_RADIUS,
            mode: StrokeMode::Carve,
            force: DEFAULT_FORCE,
            mirror_x: false,
            cache: RefCell::new(SculptCache::new()),
        }
    }

    pub(crate) fn reset_transient(&mut self) {
        self.drag = None;
        self.path = None;
        self.cursor = None;
        self.cache = RefCell::new(SculptCache::new());
    }
}

fn to_local(transform: &Transform, point: Vec3) -> Vec3 {
    let rotated = inverse_rotate(
        &rotation_from_euler(transform.rotation_degrees),
        point - transform.position,
    );
    let scale = transform.scale;
    let safe = |value: f32| if value.abs() > 1.0e-6 { value } else { 1.0 };
    Vec3::new(
        rotated.x / safe(scale.x),
        rotated.y / safe(scale.y),
        rotated.z / safe(scale.z),
    )
}

fn cave_mesh(item: &petunia_render::SculptItem) -> Result<PolygonMesh, Failure> {
    let mut mesh = PolygonMesh::default();
    for chunk in &item.chunks {
        for triangle in chunk.mesh.indices.as_chunks::<3>().0 {
            let mut corners = Vec::with_capacity(3);
            for index in triangle {
                let Some(position) = chunk.mesh.positions.get(*index as usize) else {
                    continue;
                };
                mesh.positions.push(to_world(
                    &item.transform,
                    Vec3::new(position[0], position[1], position[2]),
                ));
                corners.push((mesh.positions.len() - 1) as u32);
            }
            if corners.len() == 3 {
                mesh.faces.push(corners);
            }
        }
    }
    petunia_mesh::merge_by_distance(&mesh, 0.0005)
        .map(|(welded, _)| welded)
        .map_err(|error| {
            Failure::new(
                "SCULPT_MESH",
                &error.to_string(),
                "A caverna não forma um sólido fechado; gere a malha de novo.",
            )
        })
}

fn to_world(transform: &Transform, local: Vec3) -> Vec3 {
    let scale = transform.scale;
    let scaled = Vec3::new(local.x * scale.x, local.y * scale.y, local.z * scale.z);
    transform.position + rotate_vector(&rotation_from_euler(transform.rotation_degrees), scaled)
}

/// A world-space normal as a unit direction in the volume's space (the inverse transpose of the
/// volume's own transform).
fn normal_to_local(transform: &Transform, normal: Vec3) -> Vec3 {
    let rotated = inverse_rotate(&rotation_from_euler(transform.rotation_degrees), normal);
    let scale = transform.scale;
    Vec3::new(
        rotated.x * scale.x,
        rotated.y * scale.y,
        rotated.z * scale.z,
    )
    .normalized_or_zero()
}

/// Average scale, to turn a radius in meters into the volume's units.
fn mean_scale(transform: &Transform) -> f32 {
    let scale = transform.scale;
    let mean = (scale.x.abs() + scale.y.abs() + scale.z.abs()) / 3.0;
    if mean > 1.0e-6 { mean } else { 1.0 }
}

fn stroke_record(volume: Json, stroke: &Stroke) -> CommandRecord {
    CommandRecord::new(
        "world.create",
        json!({
            "rows": {
                "SculptStroke": {
                    "volume": volume,
                    "mode": stroke.mode.name(),
                    "start": vector_json(stroke.start),
                    "end": vector_json(stroke.end),
                    "radius": stroke.radius,
                    "smooth": stroke.smooth,
                    "strength": stroke.strength,
                }
            }
        }),
    )
}

impl EditorSession {
    // ---- the volumes, as drawn --------------------------------------------------------------

    /// Sculpted volumes as drawn this frame: what the world has, plus the dabs of the drag and
    /// the tunnel being drawn, so the preview is the real thing.
    pub(crate) fn sculpt_volumes(&self) -> Vec<SculptVolume> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut volumes = extract_sculpt_volumes(&project.world);
        if let Some(drag) = &self.sculpt.drag
            && let Some(item) = volumes.iter_mut().find(|item| item.entity == drag.volume)
        {
            item.volume.strokes.extend(drag.strokes.iter().copied());
        }
        if let Some(path) = &self.sculpt.path
            && let Some(item) = volumes.iter_mut().find(|item| item.entity == path.volume)
        {
            let strokes = self.path_strokes(path, &item.transform);
            item.volume.strokes.extend(strokes);
        }
        volumes
    }

    pub(crate) fn sculpt_items(&self) -> Vec<SculptItem> {
        let volumes = self.sculpt_volumes();
        self.sculpt.cache.borrow_mut().items(&volumes)
    }

    /// The nearest sculpted surface along the ray, skipping `ignore`.
    pub(crate) fn sculpt_hit(&self, ray: &Ray, ignore: &[u64]) -> Option<SurfaceHit> {
        let mut best: Option<SurfaceHit> = None;
        for item in self
            .sculpt_items()
            .iter()
            .filter(|item| !ignore.contains(&item.entity))
        {
            if let Some((distance, normal)) = hit_sculpt(item, ray)
                && best.is_none_or(|hit| distance < hit.distance)
            {
                best = Some(SurfaceHit {
                    entity: item.entity,
                    distance,
                    normal,
                });
            }
        }
        best
    }

    fn transform_of(&self, entity: u64) -> Transform {
        let world = self.world().ok();
        let read = |column: &str, default: Vec3| {
            world
                .and_then(|world| vec3_of(world, entity, "Transform", column))
                .unwrap_or(default)
        };
        Transform {
            position: read("position", Vec3::ZERO),
            rotation_degrees: read("rotation_degrees", Vec3::ZERO),
            scale: read("scale", Vec3::ONE),
        }
    }

    fn is_sculpt(&self, entity: u64) -> bool {
        self.world()
            .ok()
            .and_then(|world| {
                let handle = world.handle_of(PersistentId(entity)).ok()?;
                world.snapshot(handle).ok()
            })
            .is_some_and(|snapshot| snapshot.rows.contains_key("Sculpt"))
    }

    fn selected_cave(&self) -> Result<u64, Failure> {
        self.selection
            .filter(|id| self.is_sculpt(*id))
            .ok_or_else(|| {
                Failure::new(
                    "SCULPT_NO_VOLUME",
                    "selecione uma caverna",
                    "Clique numa caverna, ou crie uma em Criar > Caverna.",
                )
            })
    }

    /// Persistent ids of the strokes of a volume, in order.
    pub(crate) fn strokes_of(&self, volume: u64) -> Vec<u64> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        let mut found: Vec<u64> = world
            .entities_with_table("SculptStroke")
            .unwrap_or_default()
            .into_iter()
            .filter(|handle| {
                matches!(world.get(*handle, "SculptStroke", "volume"),
                    Ok(Value::Int(owner)) if u64::try_from(*owner).ok() == Some(volume))
            })
            .filter_map(|handle| world.persistent_id(handle).ok().map(|id| id.0))
            .collect();
        found.sort_unstable();
        found
    }

    /// The `world.create` records that copy a volume's strokes onto the entity created by command
    /// `owner` of the same batch.
    pub(crate) fn copy_strokes_records(&self, volume: u64, owner: usize) -> Vec<CommandRecord> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        let Some(item) = extract_sculpt_volumes(world)
            .into_iter()
            .find(|item| item.entity == volume)
        else {
            return Vec::new();
        };
        item.volume
            .strokes
            .iter()
            .map(|stroke| stroke_record(json!({ "$entity_of": owner }), stroke))
            .collect()
    }

    /// Runs the records as one undo step (a single record runs as itself).
    fn execute_step(&mut self, mut records: Vec<CommandRecord>) -> Result<Json, Failure> {
        match records.len() {
            0 => Ok(Json::Null),
            1 => self.execute(&records.remove(0)),
            _ => self.execute(&CommandRecord::new(
                BATCH_COMMAND,
                json!({ "records": records }),
            )),
        }
    }

    // ---- creating ---------------------------------------------------------------------------

    /// A block of rock in front of the camera, sitting on the current floor. With no `seed` it
    /// gets one tunnel, so the idea is visible at once; with a `seed` it gets a generated cave
    /// (rooms and winding tunnels). One undo step either way.
    pub fn create_cave(&mut self, seed: Option<u64>) -> Result<Json, Failure> {
        let size = if seed.is_some() {
            GENERATED_SIZE
        } else {
            Vec3::new(16.0, 6.0, 16.0)
        };
        let camera = self.camera.camera;
        let forward = camera.forward();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalized_or_zero();
        let flat = if flat.length_squared() == 0.0 {
            Vec3::new(0.0, 0.0, -1.0)
        } else {
            flat
        };
        let ahead = camera.position + flat * (size.z * 0.5 + 4.0);
        let position = Vec3::new(
            ahead.x.round(),
            self.floor_elevation() + size.y * 0.5,
            ahead.z.round(),
        );
        let strokes = match seed {
            Some(seed) => generate_cave(&GenerateParams::for_size(seed, size)),
            None => vec![Stroke::capsule(
                StrokeMode::Carve,
                Vec3::new(0.0, -1.0, size.z * 0.5 + 1.0),
                Vec3::new(2.0, -0.5, -2.0),
                1.6,
                0.8,
            )],
        };
        let mut records = vec![CommandRecord::new(
            "world.create",
            json!({
                "rows": {
                    "Name": { "text": "Caverna" },
                    "Transform": { "position": vector_json(position) },
                    "Sculpt": { "base": "solid", "size": vector_json(size), "cell_size": 0.5 }
                }
            }),
        )];
        records.extend(
            strokes
                .iter()
                .map(|stroke| stroke_record(json!({ "$entity_of": 0 }), stroke)),
        );
        let result = self.execute_step(records)?;
        let entity = result
            .get(0)
            .and_then(|first| first.get("entity"))
            .and_then(Json::as_u64);
        self.selection = entity;
        Ok(json!({ "entity": entity, "strokes": strokes.len() }))
    }

    /// Adds a generated set of rooms and tunnels to the selected cave.
    pub fn sculpt_generate(
        &mut self,
        seed: u64,
        rooms: Option<u32>,
        worms: Option<u32>,
        radius: Option<f32>,
    ) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let size = self
            .world()
            .ok()
            .and_then(|world| vec3_of(world, volume, "Sculpt", "size"))
            .unwrap_or(GENERATED_SIZE);
        let mut params = GenerateParams::for_size(seed, size);
        params.rooms = rooms.unwrap_or(params.rooms);
        params.worms = worms.unwrap_or(params.worms);
        params.tunnel_radius = radius.unwrap_or(params.tunnel_radius);
        let room_left = MAX_STROKES.saturating_sub(self.strokes_of(volume).len());
        let strokes: Vec<Stroke> = generate_cave(&params).into_iter().take(room_left).collect();
        if strokes.is_empty() {
            return Err(Failure::new(
                "SCULPT_GENERATE_EMPTY",
                "não coube nenhum túnel gerado",
                "A caverna é pequena demais, ou já está no limite de traços. Aumente o tamanho ou funda os traços.",
            ));
        }
        let records = strokes
            .iter()
            .map(|stroke| stroke_record(json!(volume), stroke))
            .collect();
        self.execute_step(records)?;
        Ok(json!({ "strokes": strokes.len() }))
    }

    /// Merges chains of small capsules of the selected cave into fewer, longer ones (one undo
    /// step). The surface stays within a quarter of a lattice cell of what it was.
    pub fn sculpt_consolidate(&mut self) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let (old_ids, old_strokes, cell) = {
            let world = self.world()?;
            let item = extract_sculpt_volumes(world)
                .into_iter()
                .find(|item| item.entity == volume)
                .ok_or_else(|| Failure::bad_request("a caverna não existe mais"))?;
            (
                self.strokes_of(volume),
                item.volume.strokes.clone(),
                item.volume.cell_size,
            )
        };
        let merged = consolidate(&old_strokes, cell * CONSOLIDATE_TOLERANCE);
        let (before, after) = (old_strokes.len(), merged.len());
        if after >= before {
            return Ok(json!({ "before": before, "after": after, "changed": false }));
        }
        let mut records: Vec<CommandRecord> = old_ids
            .into_iter()
            .map(|stroke| CommandRecord::new("world.remove", json!({ "entity": stroke })))
            .collect();
        records.extend(
            merged
                .iter()
                .map(|stroke| stroke_record(json!(volume), stroke)),
        );
        self.execute_step(records)?;
        Ok(json!({ "before": before, "after": after, "changed": true }))
    }

    /// Turns mirroring across the cave's local x = 0 on or off.
    pub fn set_sculpt_mirror(&mut self, enabled: bool) -> Json {
        self.sculpt.mirror_x = enabled;
        json!({ "mirror_x": enabled })
    }

    /// Fills the selected cave with a big add stroke (one undo step).
    pub fn sculpt_fill(&mut self) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let size = self
            .world()
            .ok()
            .and_then(|world| vec3_of(world, volume, "Sculpt", "size"))
            .unwrap_or(Vec3::new(16.0, 6.0, 16.0));
        let radius = (size.x.max(size.y).max(size.z) * 0.5).max(0.1);
        let stroke = Stroke::capsule(StrokeMode::Add, Vec3::ZERO, Vec3::ZERO, radius, 0.0);
        self.execute_step(vec![stroke_record(json!(volume), &stroke)])?;
        Ok(json!({ "strokes": 1 }))
    }

    /// Leaves a shell: carves a sphere inside the selected cave (one undo step).
    pub fn sculpt_hollow(&mut self, thickness: Option<f32>) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let size = self
            .world()
            .ok()
            .and_then(|world| vec3_of(world, volume, "Sculpt", "size"))
            .unwrap_or(Vec3::new(16.0, 6.0, 16.0));
        let smallest = size.x.min(size.y).min(size.z) * 0.5;
        let thickness = thickness.unwrap_or(1.0).clamp(0.1, smallest.max(0.1));
        let radius = (smallest - thickness).max(0.1);
        let stroke = Stroke::capsule(StrokeMode::Carve, Vec3::ZERO, Vec3::ZERO, radius, 0.0);
        self.execute_step(vec![stroke_record(json!(volume), &stroke)])?;
        Ok(json!({ "strokes": 1, "radius": radius }))
    }

    /// Adds deterministic bumps and pits inside the selected cave (one undo step).
    pub fn sculpt_noise(
        &mut self,
        count: Option<u32>,
        seed: Option<u64>,
        radius: Option<f32>,
    ) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let size = self
            .world()
            .ok()
            .and_then(|world| vec3_of(world, volume, "Sculpt", "size"))
            .unwrap_or(Vec3::new(16.0, 6.0, 16.0));
        let count = count.unwrap_or(12).clamp(1, 64);
        let room = MAX_STROKES.saturating_sub(self.strokes_of(volume).len());
        if room == 0 {
            return Err(Failure::new(
                "SCULPT_FULL",
                "a caverna já está no limite de traços",
                "Use Fundir traços ou apague alguns.",
            ));
        }
        let count = count.min(room as u32);
        let radius = radius
            .unwrap_or(size.x.min(size.y).min(size.z) * 0.08)
            .clamp(0.1, 16.0);
        let half = size * 0.5;
        let mut state = seed.unwrap_or(1) | 1;
        let mut strokes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let mut coordinate = || {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (state >> 33) as f32 / (1u64 << 31) as f32 - 1.0
            };
            let point = Vec3::new(
                coordinate() * half.x,
                coordinate() * half.y,
                coordinate() * half.z,
            );
            let mode = if state & 1 == 0 {
                StrokeMode::Add
            } else {
                StrokeMode::Carve
            };
            strokes.push(Stroke::capsule(mode, point, point, radius, 0.0));
        }
        let saved = strokes.len();
        let records: Vec<CommandRecord> = strokes
            .iter()
            .map(|stroke| stroke_record(json!(volume), stroke))
            .collect();
        self.execute_step(records)?;
        Ok(json!({ "strokes": saved }))
    }

    /// Bakes two caves into one mesh with a boolean and removes the caves (one undo step).
    ///
    /// The caves are SDF volumes; the result is a regular editable `Mesh` in world coordinates.
    /// Subtracting or intersecting loses the ability to keep sculpting the result, which is the
    /// price of a true boolean between two independently sculpted volumes.
    pub fn sculpt_boolean(
        &mut self,
        target: u64,
        tool: u64,
        operation: &str,
        keep_target: bool,
        keep_tool: bool,
    ) -> Result<Json, Failure> {
        if target == tool {
            return Err(Failure::bad_request(
                "o alvo e a ferramenta precisam ser cavernas diferentes",
            ));
        }
        let operation = match operation {
            "union" => BooleanOperation::Union,
            "subtract" => BooleanOperation::Subtract,
            "intersect" => BooleanOperation::Intersect,
            other => {
                return Err(Failure::bad_request(&format!(
                    "operação `{other}` desconhecida (use union, subtract ou intersect)"
                )));
            }
        };
        let (target_mesh, tool_mesh) = {
            let items = self.sculpt_items();
            let target_item = items
                .iter()
                .find(|item| item.entity == target)
                .ok_or_else(|| {
                    Failure::new(
                        "SCULPT_MISSING",
                        "a caverna alvo não existe",
                        "Confira o id.",
                    )
                })?;
            let tool_item = items
                .iter()
                .find(|item| item.entity == tool)
                .ok_or_else(|| {
                    Failure::new(
                        "SCULPT_MISSING",
                        "a caverna ferramenta não existe",
                        "Confira o id.",
                    )
                })?;
            (cave_mesh(target_item)?, cave_mesh(tool_item)?)
        };
        if target_mesh.faces.is_empty() || tool_mesh.faces.is_empty() {
            return Err(Failure::new(
                "SCULPT_EMPTY",
                "uma das cavernas não tem malha",
                "Gere ou esculpa a caverna antes do booleano.",
            ));
        }
        let baked =
            petunia_mesh::boolean(&target_mesh, &tool_mesh, operation).map_err(|error| {
                Failure::new(
                    "SCULPT_BOOLEAN",
                    &error.to_string(),
                    "Ajuste as cavernas ou tente outra operação.",
                )
            })?;
        let vertices = baked.positions.len();
        let faces = baked.faces.len();
        let mut records = vec![CommandRecord::new(
            "world.create",
            json!({
                "rows": {
                    "Name": { "text": "Booleano" },
                    "Mesh": { "geometry": baked.to_text_json() },
                }
            }),
        )];
        if !keep_target {
            for stroke in self.strokes_of(target) {
                records.push(CommandRecord::new(
                    "world.remove",
                    json!({ "entity": stroke }),
                ));
            }
            records.push(CommandRecord::new(
                "world.remove",
                json!({ "entity": target }),
            ));
        }
        if !keep_tool {
            for stroke in self.strokes_of(tool) {
                records.push(CommandRecord::new(
                    "world.remove",
                    json!({ "entity": stroke }),
                ));
            }
            records.push(CommandRecord::new(
                "world.remove",
                json!({ "entity": tool }),
            ));
        }
        let outcome = self.execute_step(records)?;
        let entity = outcome
            .get(0)
            .and_then(|entry| entry.get("entity"))
            .and_then(Json::as_u64);
        if let Some(entity) = entity {
            self.selection = Some(entity);
        }
        Ok(json!({ "entity": entity, "vertices": vertices, "faces": faces }))
    }

    /// Imprints the surface of a mesh entity into the selected cave (one undo step).
    ///
    /// The mesh is taken with its own transform, so moving, turning or scaling the object decides
    /// where the stamp lands. The triangles are baked in the cave's own space: later edits to the
    /// source object do not move the stamp, and deleting it leaves the cave as it is.
    pub fn sculpt_stamp_mesh(
        &mut self,
        mesh_entity: u64,
        operation: &str,
        smooth: Option<f32>,
    ) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let operation = StampOperation::parse(operation).ok_or_else(|| {
            Failure::bad_request(&format!(
                "operação `{operation}` desconhecida (use add ou carve)"
            ))
        })?;
        let geometry = {
            let world = self.world()?;
            let handle = world.handle_of(PersistentId(mesh_entity)).map_err(|_| {
                Failure::new(
                    "SCULPT_NO_MESH",
                    "a malha não existe",
                    "Escolha um objeto com malha na cena.",
                )
            })?;
            match world.get(handle, "Mesh", "geometry") {
                Ok(Value::Mesh(mesh)) => mesh.clone(),
                _ => {
                    return Err(Failure::new(
                        "SCULPT_NO_MESH",
                        "o objeto escolhido não tem malha",
                        "Escolha um objeto importado ou criado por booleano.",
                    ));
                }
            }
        };
        if geometry.faces.is_empty() {
            return Err(Failure::new(
                "SCULPT_EMPTY",
                "a malha não tem faces",
                "Escolha um objeto com geometria.",
            ));
        }
        let triangles = geometry.triangles().len();
        if triangles > MAX_STAMP_TRIANGLES {
            return Err(Failure::bad_request(&format!(
                "a malha tem {triangles} triângulos; o carimbo aceita até {MAX_STAMP_TRIANGLES}"
            )));
        }
        let smooth = smooth.unwrap_or(0.5).clamp(0.0, MAX_SMOOTH);
        let source = self.transform_of(mesh_entity);
        let cave = self.transform_of(volume);
        let local = PolygonMesh {
            positions: geometry
                .positions
                .iter()
                .map(|point| to_local(&cave, to_world(&source, *point)))
                .collect(),
            faces: geometry.faces.clone(),
        };
        let record = CommandRecord::new(
            "world.create",
            json!({
                "rows": {
                    "SculptStamp": {
                        "volume": volume,
                        "operation": operation.name(),
                        "geometry": local.to_text_json(),
                        "smooth": smooth,
                    }
                }
            }),
        );
        self.execute(&record)?;
        Ok(json!({ "triangles": triangles, "faces": local.faces.len() }))
    }

    /// Switches flat shading (low poly) of the selected cave.
    pub fn sculpt_faceted(&mut self, faceted: bool) -> Result<Json, Failure> {
        let volume = self.selected_cave()?;
        let record = CommandRecord::new(
            "world.set",
            json!({ "entity": volume, "table": "Sculpt", "column": "faceted", "value": faceted }),
        );
        self.execute(&record)?;
        Ok(json!({ "faceted": faceted }))
    }

    // ---- sculpting drag ---------------------------------------------------------------------

    /// Starts a sculpting drag at the mouse. `invert` does the opposite of the mode.
    pub fn sculpt_begin(&mut self, pointer: Pointer, invert: bool) -> Result<Json, Failure> {
        let view = self.view_for(pointer);
        let ray = view
            .ray(pointer.mouse())
            .ok_or_else(|| Failure::bad_request("o ponteiro está fora do viewport"))?;
        let volume = self
            .sculpt_hit(&ray, &[])
            .map(|hit| hit.entity)
            .or(self.selection.filter(|id| self.is_sculpt(*id)))
            .ok_or_else(|| {
                Failure::new(
                    "SCULPT_NO_VOLUME",
                    "não há caverna sob o mouse",
                    "Crie uma em Criar > Caverna, ou selecione uma e clique sobre ela.",
                )
            })?;
        if self.strokes_of(volume).len() >= MAX_STROKES {
            return Err(Failure::new(
                "SCULPT_FULL",
                &format!("a caverna já tem {MAX_STROKES} traços"),
                "Use Fundir traços, ou crie outra caverna ao lado.",
            ));
        }
        self.selection = Some(volume);
        self.sculpt.drag = Some(SculptDrag {
            volume,
            mode: self.sculpt.mode,
            inverted: invert,
            strokes: Vec::new(),
            last: None,
            plane_normal: None,
        });
        self.sculpt_drag(pointer)
    }

    /// Adds a dab where the mouse meets the cave, once it has moved far enough.
    pub fn sculpt_drag(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let Some(volume) = self.sculpt.drag.as_ref().map(|drag| drag.volume) else {
            return Ok(Json::Null);
        };
        let view = self.view_for(pointer);
        let Some(ray) = view.ray(pointer.mouse()) else {
            return Ok(Json::Null);
        };
        let Some(hit) = self
            .sculpt_hit(&ray, &[])
            .filter(|hit| hit.entity == volume)
        else {
            self.sculpt.cursor = None;
            return Ok(self.sculpt_json());
        };
        let transform = self.transform_of(volume);
        let point = ray.origin + ray.direction * hit.distance;
        self.sculpt.cursor = Some(SculptCursor {
            point,
            normal: hit.normal,
        });
        let radius_world = self.sculpt.radius;
        let force = self.sculpt.force;
        let Some(drag) = self.sculpt.drag.as_mut() else {
            return Ok(Json::Null);
        };
        let radius = radius_world / mean_scale(&transform);
        let smooth = (radius * SMOOTH_RATIO).min(MAX_SMOOTH);
        let mode = match (drag.mode, drag.inverted) {
            (StrokeMode::Carve, true) => StrokeMode::Add,
            (StrokeMode::Add, true) => StrokeMode::Carve,
            (mode, _) => mode,
        };
        let center_world = if mode == StrokeMode::Carve {
            point - hit.normal * (radius_world * CARVE_DEPTH)
        } else {
            point
        };
        let center = to_local(&transform, center_world);
        let spaced = drag
            .last
            .is_none_or(|last| (center - last).length() >= radius * DAB_SPACING);
        if !spaced || drag.strokes.len() >= MAX_DABS_PER_DRAG {
            return Ok(self.sculpt_json());
        }
        let stroke = match mode {
            StrokeMode::Carve | StrokeMode::Add => {
                Stroke::capsule(mode, drag.last.unwrap_or(center), center, radius, smooth)
            }
            StrokeMode::Inflate => Stroke {
                strength: if drag.inverted { -1.0 } else { 1.0 } * force * radius * INFLATE_RATIO,
                ..Stroke::capsule(mode, drag.last.unwrap_or(center), center, radius, 0.0)
            },
            StrokeMode::Flatten => {
                let normal = *drag
                    .plane_normal
                    .get_or_insert_with(|| normal_to_local(&transform, hit.normal));
                Stroke {
                    strength: force.max(0.1),
                    ..Stroke::capsule(mode, center, center + normal, radius, 0.0)
                }
            }
            StrokeMode::Smooth => Stroke {
                strength: force.max(0.1),
                ..Stroke::capsule(mode, center, center, radius, 0.0)
            },
        };
        drag.strokes.push(stroke);
        if self.sculpt.mirror_x {
            let mirrored = Stroke {
                start: Vec3::new(-stroke.start.x, stroke.start.y, stroke.start.z),
                end: Vec3::new(-stroke.end.x, stroke.end.y, stroke.end.z),
                ..stroke
            };
            drag.strokes.push(mirrored);
        }
        drag.last = Some(center);
        Ok(self.sculpt_json())
    }

    /// Saves the dabs of the drag as one undo step.
    pub fn sculpt_end(&mut self) -> Result<Json, Failure> {
        let Some(drag) = self.sculpt.drag.take() else {
            return Ok(Json::Null);
        };
        let records: Vec<CommandRecord> = drag
            .strokes
            .iter()
            .map(|stroke| stroke_record(json!(drag.volume), stroke))
            .collect();
        let count = records.len();
        self.execute_step(records)?;
        Ok(json!({ "strokes": count }))
    }

    pub fn sculpt_cancel(&mut self) -> Json {
        self.sculpt.drag = None;
        json!({ "strokes": 0 })
    }

    // ---- path tool --------------------------------------------------------------------------

    /// The strokes of the tunnel so far, ending at the mouse so the preview follows it.
    fn path_strokes(&self, path: &PathState, transform: &Transform) -> Vec<Stroke> {
        let mut points = path.points.clone();
        if let Some(cursor) = self.sculpt.cursor
            && self.tool == Tool::SculptPath
        {
            let inside = cursor.point - cursor.normal * (self.sculpt.radius * PATH_DEPTH);
            points.push(to_local(transform, inside));
        }
        self.path_stroke_list(&points, path.inverted, transform)
    }

    fn path_stroke_list(
        &self,
        points: &[Vec3],
        inverted: bool,
        transform: &Transform,
    ) -> Vec<Stroke> {
        let radius = self.sculpt.radius / mean_scale(transform);
        tunnel_strokes(
            points,
            if inverted {
                StrokeMode::Add
            } else {
                StrokeMode::Carve
            },
            radius,
            (radius * SMOOTH_RATIO).min(MAX_SMOOTH),
        )
    }

    /// Adds a point of the tunnel where the mouse meets the cave.
    pub fn path_point(&mut self, pointer: Pointer, invert: bool) -> Result<Json, Failure> {
        let view = self.view_for(pointer);
        let ray = view
            .ray(pointer.mouse())
            .ok_or_else(|| Failure::bad_request("o ponteiro está fora do viewport"))?;
        let hit = self.sculpt_hit(&ray, &[]).ok_or_else(|| {
            Failure::new(
                "SCULPT_NO_VOLUME",
                "não há caverna sob o mouse",
                "Clique sobre a rocha para pôr um ponto do túnel.",
            )
        })?;
        if let Some(path) = &self.sculpt.path
            && path.volume != hit.entity
        {
            return Err(Failure::new(
                "SCULPT_OTHER_VOLUME",
                "o túnel já começou em outra caverna",
                "Termine (Enter) ou cancele (Esc) o túnel antes.",
            ));
        }
        let transform = self.transform_of(hit.entity);
        let point = ray.origin + ray.direction * hit.distance;
        let inside = point - hit.normal * (self.sculpt.radius * PATH_DEPTH);
        let local = to_local(&transform, inside);
        let path = self.sculpt.path.get_or_insert_with(|| PathState {
            volume: hit.entity,
            points: Vec::new(),
            inverted: invert,
        });
        if path.points.len() >= MAX_PATH_POINTS {
            return Err(Failure::new(
                "SCULPT_PATH_FULL",
                &format!("o túnel já tem {MAX_PATH_POINTS} pontos"),
                "Termine o túnel com Enter e comece outro.",
            ));
        }
        path.points.push(local);
        self.selection = Some(hit.entity);
        Ok(self.sculpt_json())
    }

    pub fn path_pop(&mut self) -> Json {
        if let Some(path) = self.sculpt.path.as_mut() {
            path.points.pop();
            if path.points.is_empty() {
                self.sculpt.path = None;
            }
        }
        self.sculpt_json()
    }

    /// Digs the tunnel through the points (one undo step).
    pub fn path_commit(&mut self) -> Result<Json, Failure> {
        let Some(path) = self.sculpt.path.take() else {
            return Ok(Json::Null);
        };
        if path.points.len() < 2 {
            return Err(Failure::new(
                "SCULPT_PATH_SHORT",
                "um túnel precisa de pelo menos dois pontos",
                "Clique em mais um ponto da rocha e confirme com Enter.",
            ));
        }
        let transform = self.transform_of(path.volume);
        let strokes = self.path_stroke_list(&path.points, path.inverted, &transform);
        let room_left = MAX_STROKES.saturating_sub(self.strokes_of(path.volume).len());
        let records: Vec<CommandRecord> = strokes
            .iter()
            .take(room_left)
            .map(|stroke| stroke_record(json!(path.volume), stroke))
            .collect();
        let count = records.len();
        self.execute_step(records)?;
        Ok(json!({ "strokes": count }))
    }

    pub fn path_cancel(&mut self) -> Json {
        self.sculpt.path = None;
        self.sculpt_json()
    }

    // ---- cursor, options, overlay -----------------------------------------------------------

    /// Moves the cursor ring (a sculpting tool is active and the mouse moves without a button).
    pub(crate) fn sculpt_hover(&mut self, pointer: Pointer) -> Json {
        let view = self.view_for(pointer);
        let hit = view.ray(pointer.mouse()).and_then(|ray| {
            self.sculpt_hit(&ray, &[])
                .map(|hit| (hit, ray.origin + ray.direction * hit.distance))
        });
        self.sculpt.cursor = hit.map(|(hit, point)| SculptCursor {
            point,
            normal: hit.normal,
        });
        self.hovered = hit.map(|(hit, _)| hit.entity);
        json!({ "handle": null, "entity": self.hovered, "face": null })
    }

    pub fn set_sculpt_radius(&mut self, meters: f32) -> Result<Json, Failure> {
        if !meters.is_finite() {
            return Err(Failure::bad_request("o raio precisa ser um número"));
        }
        self.sculpt.radius = meters.clamp(MIN_SCULPT_RADIUS, MAX_SCULPT_RADIUS);
        Ok(self.sculpt_json())
    }

    /// The brush mode (`carve`, `add`, `inflate`, `flatten`, `smooth`) and its force (0..1).
    pub fn set_sculpt_mode(
        &mut self,
        mode: Option<&str>,
        force: Option<f32>,
    ) -> Result<Json, Failure> {
        if let Some(name) = mode {
            self.sculpt.mode = StrokeMode::parse(name).ok_or_else(|| {
                Failure::bad_request(&format!(
                    "modo `{name}` desconhecido (carve, add, inflate, flatten ou smooth)"
                ))
            })?;
        }
        if let Some(force) = force {
            if !force.is_finite() {
                return Err(Failure::bad_request("a força precisa ser um número"));
            }
            self.sculpt.force = force.clamp(0.0, 1.0);
        }
        Ok(self.sculpt_json())
    }

    pub(crate) fn sculpt_json(&self) -> Json {
        let selected = self.selection.filter(|id| self.is_sculpt(*id));
        let faceted = selected.is_some_and(|id| {
            self.world().ok().is_some_and(|world| {
                world
                    .handle_of(PersistentId(id))
                    .ok()
                    .is_some_and(|handle| {
                        matches!(
                            world.get(handle, "Sculpt", "faceted"),
                            Ok(Value::Bool(true))
                        )
                    })
            })
        });
        json!({
            "radius": self.sculpt.radius,
            "mode": self.sculpt.mode.name(),
            "force": self.sculpt.force,
            "mirror_x": self.sculpt.mirror_x,
            "dragging": self.sculpt.drag.is_some(),
            "dabs": self.sculpt.drag.as_ref().map_or(0, |drag| drag.strokes.len()),
            "path_points": self.sculpt.path.as_ref().map_or(0, |path| path.points.len()),
            "cave": selected,
            "strokes": selected.map_or(0, |id| self.strokes_of(id).len()),
            "max_strokes": MAX_STROKES,
            "faceted": faceted,
        })
    }

    /// The cursor ring on the rock, and the points and line of the tunnel being drawn.
    pub(crate) fn sculpt_overlay(&self, items: &mut Vec<OverlayItem>) {
        if !self.tool.is_sculpting() {
            return;
        }
        if let Some(path) = &self.sculpt.path {
            let transform = self.transform_of(path.volume);
            let mut points: Vec<Vec3> = path
                .points
                .iter()
                .map(|point| to_world(&transform, *point))
                .collect();
            for point in &points {
                items.push(OverlayItem::Square {
                    center: *point,
                    color: PATH_COLOR,
                    size_pixels: 9.0,
                });
            }
            if let Some(cursor) = self.sculpt.cursor {
                points.push(cursor.point - cursor.normal * (self.sculpt.radius * PATH_DEPTH));
            }
            for pair in points.windows(2) {
                if let (Some(from), Some(to)) = (pair.first(), pair.get(1)) {
                    items.push(OverlayItem::Line {
                        from: *from,
                        to: *to,
                        color: PATH_COLOR,
                        width_pixels: 2.0,
                    });
                }
            }
        }
        let Some(cursor) = self.sculpt.cursor else {
            return;
        };
        let normal = cursor.normal.normalized_or_zero();
        let helper = if normal.y.abs() < 0.9 {
            Vec3::UP
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let side = normal.cross(helper).normalized_or_zero();
        let other = normal.cross(side);
        let lift = cursor.point + normal * 0.02;
        let radius = self.sculpt.radius;
        let at = |step: usize| {
            let angle = step as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
            lift + (side * angle.cos() + other * angle.sin()) * radius
        };
        for step in 0..RING_SEGMENTS {
            items.push(OverlayItem::Line {
                from: at(step),
                to: at(step + 1),
                color: RING_COLOR,
                width_pixels: 2.0,
            });
        }
        items.push(OverlayItem::Line {
            from: lift,
            to: lift + normal * (radius * 0.5),
            color: RING_COLOR,
            width_pixels: 1.5,
        });
    }
}

#[cfg(test)]
mod tests {
    use petunia_math::Vec3;
    use petunia_world::{PersistentId, Value};
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn pointer(op: &str, x: f32) -> Json {
        json!({ "op": op, "x": x, "y": 300.0, "width": 800.0, "height": 600.0 })
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        (session, root)
    }

    fn count(session: &mut EditorSession) -> u64 {
        call(session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .unwrap_or(0)
    }

    fn triangles(session: &EditorSession, cave: u64) -> usize {
        let frame = session.frame().expect("frame");
        let item = frame
            .sculpts
            .iter()
            .find(|item| item.entity == cave)
            .expect("drawn");
        item.chunks
            .iter()
            .map(|chunk| chunk.mesh.indices.len() / 3)
            .sum()
    }

    #[test]
    fn a_cave_boolean_bakes_a_mesh_and_undoes() {
        let (mut session, root) = opened("sculpt_boolean");
        let target = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("target");
        let tool = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("tool");
        call(
            &mut session,
            json!({
                "op": "command.execute", "command": "world.set",
                "payload": {
                    "entity": tool, "table": "Transform", "column": "position",
                    "value": { "x": 2.0, "y": 0.0, "z": 0.0 }
                }
            }),
        );
        let made = call(
            &mut session,
            json!({ "op": "sculpt.boolean", "target": target, "tool": tool, "operation": "union" }),
        );
        assert!(made["entity"].as_u64().is_some(), "{made}");
        assert!(made["faces"].as_u64().unwrap_or(0) > 0, "{made}");
        let world = session.world().expect("world");
        assert!(world.handle_of(PersistentId(target)).is_err());
        assert!(world.handle_of(PersistentId(tool)).is_err());
        assert!(
            world
                .entities_with_table("Mesh")
                .unwrap_or_default()
                .iter()
                .any(|handle| world.persistent_id(*handle).ok()
                    == made["entity"].as_u64().map(PersistentId))
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        let world = session.world().expect("world");
        assert!(world.handle_of(PersistentId(target)).is_ok());
        assert!(world.handle_of(PersistentId(tool)).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_stamped_mesh_imprints_the_cave_and_undoes_in_one_step() {
        let (mut session, root) = opened("sculpt_stamp");
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        // The cave is created in front of the camera; put it on the origin so the local checks
        // below read in world numbers too.
        call(
            &mut session,
            json!({
                "op": "command.execute", "command": "world.set",
                "payload": {
                    "entity": cave, "table": "Transform", "column": "position",
                    "value": { "x": 0.0, "y": 0.0, "z": 0.0 }
                }
            }),
        );
        // A 6 m plate at y = 0, its face looking up (+y).
        let plate = call(
            &mut session,
            json!({
                "op": "command.execute", "command": "world.create",
                "payload": {
                    "rows": {
                        "Name": { "text": "Placa" },
                        "Mesh": { "geometry": {
                            "positions": ["-3 0 -3", "-3 0 3", "3 0 3", "3 0 -3"],
                            "faces": ["0 1 2 3"]
                        }}
                    }
                }
            }),
        )["entity"]
            .as_u64()
            .expect("plate");
        let volume_of = |session: &EditorSession| {
            session
                .sculpt_volumes()
                .into_iter()
                .find(|volume| volume.entity == cave)
                .expect("volume")
                .volume
        };
        let before = triangles(&session, cave);
        let made = call(
            &mut session,
            json!({
                "op": "sculpt.stamp_mesh", "mesh": plate,
                "operation": "carve", "smooth": 0.0
            }),
        );
        assert_eq!(made["triangles"].as_u64(), Some(2), "{made}");
        let volume = volume_of(&session);
        assert_eq!(volume.stamps.len(), 1);
        assert_eq!(volume.stamps[0].triangles.len(), 2);
        assert!(
            volume.distance(Vec3::new(0.0, -0.5, 0.0)) > 0.0,
            "air below the plate"
        );
        assert!(
            volume.distance(Vec3::new(0.0, 0.5, 0.0)) < 0.0,
            "rock above the plate"
        );
        assert!(
            volume.distance(Vec3::new(6.0, -1.0, 0.0)) < 0.0,
            "rock outside the plate stays"
        );
        assert_ne!(triangles(&session, cave), before, "the surface changed");
        call(&mut session, json!({ "op": "edit.undo" }));
        let volume = volume_of(&session);
        assert!(volume.stamps.is_empty());
        assert!(
            volume.distance(Vec3::new(0.0, -0.5, 0.0)) < 0.0,
            "the rock is back"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fill_hollow_and_noise_add_strokes_in_one_undo_each() {
        let (mut session, root) = opened("sculpt_tools");
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        let initial = session.strokes_of(cave).len();
        call(&mut session, json!({ "op": "sculpt.fill" }));
        assert_eq!(session.strokes_of(cave).len(), initial + 1);
        call(
            &mut session,
            json!({ "op": "sculpt.hollow", "thickness": 1.0 }),
        );
        assert_eq!(session.strokes_of(cave).len(), initial + 2);
        call(
            &mut session,
            json!({ "op": "sculpt.noise", "count": 4, "seed": 9 }),
        );
        assert_eq!(session.strokes_of(cave).len(), initial + 6);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), initial + 2);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), initial + 1);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), initial);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mirror_x_saves_the_stroke_and_its_mirror_in_one_undo() {
        let (mut session, root) = opened("sculpt_mirror");
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        call(&mut session, json!({ "op": "tool.set", "tool": "sculpt" }));
        call(
            &mut session,
            json!({ "op": "sculpt.mirror", "enabled": true }),
        );
        call(&mut session, pointer("sculpt.begin", 400.0));
        for x in [430.0, 460.0, 490.0] {
            call(&mut session, pointer("sculpt.drag", x));
        }
        let saved = call(&mut session, json!({ "op": "sculpt.end" }));
        let dabs = saved["strokes"].as_u64().expect("strokes");
        assert!(dabs >= 2, "{saved}");
        assert_eq!(session.strokes_of(cave).len() as u64, 1 + dabs);
        let world = session.world().expect("world");
        let mut starts = Vec::new();
        for id in session.strokes_of(cave) {
            let handle = world.handle_of(PersistentId(id)).expect("stroke");
            if let Ok(Value::Vec3(start)) = world.get(handle, "SculptStroke", "start") {
                starts.push(start.x);
            }
        }
        assert!(starts.iter().any(|x| *x > 0.1), "{starts:?}");
        assert!(starts.iter().any(|x| *x < -0.1), "{starts:?}");
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), 1);
        call(
            &mut session,
            json!({ "op": "sculpt.mirror", "enabled": false }),
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn carve_a_cave_then_undo_duplicate_and_delete_it() {
        let (mut session, root) = opened("cave");
        let before = count(&mut session);

        // The block and its first tunnel: two entities, one undo step, only the cave is listed.
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        assert_eq!(count(&mut session), before + 2);
        let listed = call(&mut session, json!({ "op": "editor.snapshot" }))["entities"].clone();
        assert!(listed.to_string().contains("Caverna"));
        assert!(!listed.to_string().contains("SculptStroke"));
        assert!(triangles(&session, cave) > 1000);

        // Drag the tool across the front of the block: several dabs, saved as one step.
        call(&mut session, json!({ "op": "tool.set", "tool": "sculpt" }));
        call(&mut session, pointer("sculpt.begin", 400.0));
        for x in [430.0, 460.0, 490.0, 520.0, 550.0] {
            call(&mut session, pointer("sculpt.drag", x));
        }
        let saved = call(&mut session, json!({ "op": "sculpt.end" }));
        let dabs = saved["strokes"].as_u64().expect("strokes");
        assert!(dabs >= 2, "{saved}");
        assert_eq!(session.strokes_of(cave).len() as u64, 1 + dabs);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), 1);

        // A click on it selects the cave; the copy brings its strokes along.
        call(&mut session, json!({ "op": "tool.set", "tool": "select" }));
        call(
            &mut session,
            json!({ "op": "selection.pick", "x": 0.0, "y": 0.0, "aspect": 800.0 / 600.0 }),
        );
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["selection"],
            json!(cave)
        );
        let copy = call(&mut session, json!({ "op": "edit.duplicate" }))["entity"]
            .as_u64()
            .expect("copy");
        assert_eq!(session.strokes_of(copy).len(), 1);
        assert_eq!(count(&mut session), before + 4);

        // Deleting a cave removes its strokes too (one step each).
        call(&mut session, json!({ "op": "edit.delete" }));
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": cave }),
        );
        call(&mut session, json!({ "op": "edit.delete" }));
        assert_eq!(count(&mut session), before);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(count(&mut session), before + 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_brush_mode_saves_one_undo_step_and_changes_the_surface() {
        let (mut session, root) = opened("modes");
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        call(&mut session, json!({ "op": "tool.set", "tool": "sculpt" }));
        for mode in ["carve", "add", "inflate", "flatten", "smooth"] {
            call(
                &mut session,
                json!({ "op": "sculpt.mode", "mode": mode, "force": 0.8 }),
            );
            let before = (session.strokes_of(cave).len(), triangles(&session, cave));
            call(&mut session, pointer("sculpt.begin", 400.0));
            for x in [430.0, 460.0, 490.0] {
                call(&mut session, pointer("sculpt.drag", x));
            }
            let saved = call(&mut session, json!({ "op": "sculpt.end" }));
            assert!(saved["strokes"].as_u64() >= Some(1), "{mode}");
            assert!(
                session.strokes_of(cave).len() > before.0,
                "{mode} saved strokes"
            );
            assert_ne!(
                triangles(&session, cave),
                before.1,
                "{mode} changed the surface"
            );
            call(&mut session, json!({ "op": "edit.undo" }));
            assert_eq!(
                session.strokes_of(cave).len(),
                before.0,
                "{mode}: one undo step"
            );
        }
        let refused = session.handle(&json!({ "op": "sculpt.mode", "mode": "melt" }));
        assert_eq!(refused["ok"], false);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_digs_a_tunnel_through_its_points_in_one_step() {
        let (mut session, root) = opened("path");
        let cave = call(&mut session, json!({ "op": "create.cave" }))["entity"]
            .as_u64()
            .expect("cave");
        call(
            &mut session,
            json!({ "op": "tool.set", "tool": "sculpt_path" }),
        );
        let first = session.strokes_of(cave).len();
        let nothing = session.handle(&json!({ "op": "path.commit" }));
        assert_eq!(nothing["ok"], true, "nothing to commit is fine: {nothing}");
        call(&mut session, pointer("path.point", 360.0));
        let one = session.handle(&json!({ "op": "path.commit" }));
        assert_eq!(one["ok"], false, "{one}");
        call(&mut session, pointer("path.point", 360.0));
        call(&mut session, pointer("path.point", 440.0));
        call(&mut session, pointer("path.point", 500.0));
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["sculpt"]["path_points"],
            3
        );
        let saved = call(&mut session, json!({ "op": "path.commit" }))["strokes"]
            .as_u64()
            .expect("strokes");
        assert!(saved >= 2);
        assert_eq!(session.strokes_of(cave).len() as u64, first as u64 + saved);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(session.strokes_of(cave).len(), first);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn generate_consolidate_and_facet_work_on_the_selected_cave() {
        let (mut session, root) = opened("generate");
        let refused = session.handle(&json!({ "op": "sculpt.generate", "seed": 1 }));
        assert_eq!(refused["ok"], false, "no cave selected");
        let cave = call(&mut session, json!({ "op": "create.cave", "seed": 42 }))["entity"]
            .as_u64()
            .expect("cave");
        let generated = session.strokes_of(cave).len();
        assert!(generated > 10, "{generated}");
        assert!(triangles(&session, cave) > 3000);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(
            session.strokes_of(cave).len(),
            0,
            "the cave and its strokes are one step"
        );
        call(&mut session, json!({ "op": "edit.redo" }));
        // Undo does not restore the selection (a known limit): select the cave again.
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": cave }),
        );

        let extra = call(&mut session, json!({ "op": "sculpt.generate", "seed": 7 }))["strokes"]
            .as_u64()
            .expect("strokes");
        assert!(extra > 5);

        // A long drag becomes many strokes; consolidating merges its straight runs.
        call(&mut session, json!({ "op": "tool.set", "tool": "sculpt" }));
        call(&mut session, pointer("sculpt.begin", 300.0));
        for step in 0..40 {
            call(
                &mut session,
                pointer("sculpt.drag", 300.0 + step as f32 * 5.0),
            );
        }
        call(&mut session, json!({ "op": "sculpt.end" }));
        let before = session.strokes_of(cave).len();
        let merged = call(&mut session, json!({ "op": "sculpt.consolidate" }));
        assert_eq!(merged["before"], before);
        assert!(
            merged["after"].as_u64() < merged["before"].as_u64(),
            "{merged}"
        );
        assert_eq!(
            session.strokes_of(cave).len() as u64,
            merged["after"].as_u64().unwrap_or(0)
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(
            session.strokes_of(cave).len(),
            before,
            "consolidating is one undo step"
        );

        let faceted = call(
            &mut session,
            json!({ "op": "sculpt.faceted", "value": true }),
        );
        assert_eq!(faceted["faceted"], true);
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["sculpt"]["faceted"],
            true
        );
        let frame = session.frame().expect("frame");
        assert!(frame.sculpts.iter().any(|item| item.faceted));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sculpting_needs_a_cave_and_a_sane_radius() {
        let (mut session, root) = opened("refused");
        let refused = session.handle(&pointer("sculpt.begin", 400.0));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["error"]["code"], "SCULPT_NO_VOLUME", "{refused}");
        let radius = call(
            &mut session,
            json!({ "op": "sculpt.radius", "meters": 1000.0 }),
        );
        assert_eq!(radius["radius"], 16.0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
