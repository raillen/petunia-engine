//! Editable meshes in the editor (DR-22, option A): the mesh itself is saved in a `Mesh` row, and
//! every modeling operation replaces it in **one** command, so one `Ctrl+Z` undoes it.
//!
//! | Op | What it does |
//! |---|---|
//! | `mesh.convert` | brush → editable mesh ("Tornar editável"), same entity, same look |
//! | `mesh.boolean` | union / subtract / intersect with another brush or mesh |
//! | `mesh.inset`, `mesh.extrude`, `mesh.subdivide`, `mesh.delete_faces`, `mesh.flip` | on faces |
//! | `mesh.loop_cut`, `mesh.bevel`, `mesh.dissolve` | on edges |
//! | `mesh.merge`, `mesh.merge_by_distance` | on vertices |
//! | `mesh.knife` | cuts one face between two points on its edges |
//! | `mesh.pick`, `mesh.select`, `mesh.info` | find and choose faces, edges and vertices |
//! | `mesh.preview`, `mesh.preview_end` | inset, extrude, bevel or loop cut live while dragging |
//!
//! Faces, edges and vertices are numbers into the mesh. An op takes them from the request or,
//! when the request has none, from the component selection (`mesh.select`). The selection
//! belongs to one mesh as it was when chosen: once the geometry changes (an op, undo, a script),
//! it is empty again instead of pointing at the wrong faces.

use std::collections::BTreeSet;

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_math::{Color, PolygonMesh, Transform, Vec3};
use petunia_mesh::{BooleanOperation, EdgePoint, Grouping, MeshEditError};
use petunia_render::{OverlayItem, Ray, Shape, build_mesh};
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::{inverse_rotate, rotate_vector, rotation_from_euler};
use crate::session::{EditorSession, display_name};
use crate::tools::Pointer;

/// The wireframe of the selected mesh is drawn up to this many edges (a huge mesh would flood
/// the overlay).
const MAX_WIREFRAME_EDGES: usize = 20_000;
const WIRE_COLOR: Color = Color::new(0.10, 0.12, 0.16, 0.55);
const FACE_SELECTION_COLOR: Color = Color::new(1.0, 0.62, 0.15, 1.0);
const EDGE_SELECTION_COLOR: Color = Color::new(1.0, 0.85, 0.20, 1.0);
const VERTEX_SELECTION_COLOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);
/// Default distances (meters) when the request does not say.
const DEFAULT_INSET: f32 = 0.2;
const DEFAULT_EXTRUDE: f32 = 0.5;
const DEFAULT_BEVEL: f32 = 0.1;
const DEFAULT_WELD: f32 = 0.001;

/// Faces, edges and vertices chosen on one mesh, valid while its geometry is the same.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ComponentSelection {
    entity: u64,
    fingerprint: u64,
    faces: BTreeSet<u32>,
    edges: BTreeSet<[u32; 2]>,
    vertices: BTreeSet<u32>,
    /// A drag in progress (`mesh.preview`). It lives here, with the parts it was started on.
    preview: Option<Box<MeshPreview>>,
}

/// An operation shown live while the mouse drags its value. Every step is computed again from
/// `base`, the mesh before the drag, so moving back and forth never accumulates; the session's
/// gesture keeps the change out of the history until the drag ends.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MeshPreview {
    op: String,
    base: PolygonMesh,
    /// The parts the drag started on (numbers into `base`).
    chosen: ComponentSelection,
    /// Fingerprint of the mesh the preview last put in the world. Anything else there means the
    /// gesture was ended elsewhere, and the preview is forgotten.
    shown: u64,
    /// What stays selected when the drag is kept (the inner faces of an inset, for example).
    keep_faces: Vec<u32>,
}

/// Operations that can be previewed: the ones with a value worth dragging.
const PREVIEW_OPERATIONS: [&str; 4] = ["inset", "extrude", "bevel", "loop_cut"];

fn edge_key(edge: [u32; 2]) -> [u32; 2] {
    [edge[0].min(edge[1]), edge[0].max(edge[1])]
}

impl From<MeshEditError> for Failure {
    fn from(error: MeshEditError) -> Self {
        Failure::new(error.code, &error.message, &error.suggestion)
    }
}

fn to_world(transform: &Transform, local: Vec3) -> Vec3 {
    let scale = transform.scale;
    let scaled = Vec3::new(local.x * scale.x, local.y * scale.y, local.z * scale.z);
    transform.position + rotate_vector(&rotation_from_euler(transform.rotation_degrees), scaled)
}

/// The inverse of [`to_world`]. `None` when a scale is zero (nothing can be undone).
fn to_local(transform: &Transform, point: Vec3) -> Option<Vec3> {
    let scale = transform.scale;
    if [scale.x, scale.y, scale.z]
        .iter()
        .any(|axis| axis.abs() < 1.0e-6)
    {
        return None;
    }
    let rotated = inverse_rotate(
        &rotation_from_euler(transform.rotation_degrees),
        point - transform.position,
    );
    Some(Vec3::new(
        rotated.x / scale.x,
        rotated.y / scale.y,
        rotated.z / scale.z,
    ))
}

/// Distance along the ray to the triangle (Möller–Trumbore), both sides.
fn ray_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge_one = b - a;
    let edge_two = c - a;
    let perpendicular = direction.cross(edge_two);
    let determinant = edge_one.dot(perpendicular);
    if determinant.abs() < 1.0e-12 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let offset = origin - a;
    let u = offset.dot(perpendicular) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let across = offset.cross(edge_one);
    let v = direction.dot(across) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge_two.dot(across) * inverse;
    (distance > 1.0e-6).then_some(distance)
}

fn distance_to_segment(point: Vec3, from: Vec3, to: Vec3) -> f32 {
    let along = to - from;
    let length_squared = along.length_squared();
    if length_squared <= f32::EPSILON {
        return (point - from).length();
    }
    let parameter = ((point - from).dot(along) / length_squared).clamp(0.0, 1.0);
    (point - (from + along * parameter)).length()
}

fn numbers(request: &Json, key: &str) -> Result<Option<Vec<u32>>, Failure> {
    let Some(list) = request.get(key) else {
        return Ok(None);
    };
    let items = list.as_array().ok_or_else(|| {
        Failure::bad_request(&format!("`{key}` precisa ser uma lista de números"))
    })?;
    items
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    Failure::bad_request(&format!("`{key}` só aceita números inteiros positivos"))
                })
        })
        .collect::<Result<Vec<u32>, Failure>>()
        .map(Some)
}

fn edge_of(json: &Json, key: &str) -> Result<[u32; 2], Failure> {
    let pair = json
        .as_array()
        .filter(|pair| pair.len() == 2)
        .and_then(|pair| {
            let first = u32::try_from(pair.first()?.as_u64()?).ok()?;
            let second = u32::try_from(pair.get(1)?.as_u64()?).ok()?;
            Some([first, second])
        });
    pair.ok_or_else(|| {
        Failure::bad_request(&format!(
            "`{key}`: cada aresta é um par de vértices, [a, b]"
        ))
    })
}

fn edges(request: &Json, key: &str) -> Result<Option<Vec<[u32; 2]>>, Failure> {
    let Some(list) = request.get(key) else {
        return Ok(None);
    };
    let items = list
        .as_array()
        .ok_or_else(|| Failure::bad_request(&format!("`{key}` precisa ser uma lista de pares")))?;
    items
        .iter()
        .map(|item| edge_of(item, key))
        .collect::<Result<Vec<_>, Failure>>()
        .map(Some)
}

fn float(request: &Json, key: &str, default: f32) -> Result<f32, Failure> {
    match request.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .map(|number| number as f32)
            .filter(|number| number.is_finite())
            .ok_or_else(|| Failure::bad_request(&format!("`{key}` precisa ser um número"))),
    }
}

fn count(request: &Json, key: &str, default: u32) -> Result<u32, Failure> {
    match request.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .ok_or_else(|| {
                Failure::bad_request(&format!("`{key}` precisa ser um inteiro positivo"))
            }),
    }
}

fn grouping(request: &Json) -> Result<Grouping, Failure> {
    match request.get("grouping").and_then(Json::as_str) {
        None => Ok(Grouping::Region),
        Some(name) => Grouping::parse(name).ok_or_else(|| {
            Failure::bad_request("`grouping` é `region` (juntas) ou `individual` (cada face)")
        }),
    }
}

fn edge_point(request: &Json, key: &str) -> Result<EdgePoint, Failure> {
    let point = request.get(key).ok_or_else(|| {
        Failure::bad_request(&format!(
            "falta `{key}`: {{\"edge\": [a, b], \"fraction\": 0.5}}"
        ))
    })?;
    Ok(EdgePoint {
        edge: edge_of(
            point
                .get("edge")
                .ok_or_else(|| Failure::bad_request(&format!("`{key}` precisa de `edge`")))?,
            key,
        )?,
        fraction: float(point, "fraction", 0.5)?,
    })
}

fn needs_mesh(entity: u64) -> Failure {
    Failure::new(
        "EDITOR_NOT_A_MESH",
        &format!("a entidade {entity} não é uma malha editável"),
        "Selecione o objeto e use Malha > Tornar editável (mesh.convert) antes de editar faces e arestas.",
    )
}

/// The selection after an edit: the faces the op says stay chosen, on the new mesh.
fn selection_after(entity: u64, result: &PolygonMesh, keep_faces: Vec<u32>) -> ComponentSelection {
    ComponentSelection {
        entity,
        fingerprint: result.fingerprint(),
        faces: keep_faces
            .into_iter()
            .filter(|face| (*face as usize) < result.faces.len())
            .collect(),
        ..ComponentSelection::default()
    }
}

/// Runs one component op on `mesh`. Parts come from the request, or else from `chosen`. Returns
/// the new mesh and the faces that stay chosen.
fn compute_edit(
    op: &str,
    mesh: &PolygonMesh,
    chosen: &ComponentSelection,
    request: &Json,
) -> Result<(PolygonMesh, Vec<u32>), Failure> {
    let faces = |key: &str| -> Result<Vec<u32>, Failure> {
        Ok(numbers(request, key)?.unwrap_or_else(|| chosen.faces.iter().copied().collect()))
    };
    let chosen_edges = || -> Result<Vec<[u32; 2]>, Failure> {
        Ok(edges(request, "edges")?.unwrap_or_else(|| chosen.edges.iter().copied().collect()))
    };
    let mut keep_faces: Vec<u32> = Vec::new();
    let result = match op {
        "mesh.inset" => {
            let (result, inner) = petunia_mesh::inset(
                mesh,
                &faces("faces")?,
                float(request, "distance", DEFAULT_INSET)?,
                grouping(request)?,
            )?;
            keep_faces = inner;
            result
        }
        "mesh.extrude" => {
            let picked = faces("faces")?;
            let result = petunia_mesh::extrude(
                mesh,
                &picked,
                float(request, "distance", DEFAULT_EXTRUDE)?,
                grouping(request)?,
            )?;
            keep_faces = picked;
            result
        }
        "mesh.subdivide" => {
            petunia_mesh::subdivide(mesh, &faces("faces")?, count(request, "cuts", 1)?)?
        }
        "mesh.delete_faces" => petunia_mesh::delete_faces(mesh, &faces("faces")?)?,
        "mesh.flip" => petunia_mesh::flip_faces(mesh, &faces("faces")?)?,
        "mesh.loop_cut" => {
            let edge = match request.get("edge") {
                Some(edge) => edge_of(edge, "edge")?,
                None => chosen.edges.iter().next().copied().ok_or_else(|| {
                    Failure::new(
                        "MESH_NO_EDGES",
                        "o corte em laço precisa de uma aresta",
                        "Clique em uma aresta do anel que quer cortar.",
                    )
                })?,
            };
            petunia_mesh::loop_cut(
                mesh,
                edge,
                count(request, "cuts", 1)?,
                float(request, "slide", 0.0)?,
            )?
        }
        "mesh.bevel" => petunia_mesh::bevel(
            mesh,
            &chosen_edges()?,
            float(request, "width", DEFAULT_BEVEL)?,
            count(request, "segments", 1)?,
        )?,
        "mesh.dissolve" => petunia_mesh::dissolve_edges(mesh, &chosen_edges()?)?,
        "mesh.merge" => petunia_mesh::merge_vertices(
            mesh,
            &numbers(request, "vertices")?
                .unwrap_or_else(|| chosen.vertices.iter().copied().collect()),
        )?,
        "mesh.merge_by_distance" => {
            petunia_mesh::merge_by_distance(mesh, float(request, "distance", DEFAULT_WELD)?)?.0
        }
        "mesh.knife" => petunia_mesh::knife(
            mesh,
            edge_point(request, "start")?,
            edge_point(request, "end")?,
        )?,
        other => {
            return Err(Failure::new(
                "EDITOR_UNKNOWN_OP",
                &format!("operação `{other}` desconhecida"),
                "Veja a lista de operações de malha em docs/02-arquitetura/11-editor.md.",
            ));
        }
    };
    Ok((result, keep_faces))
}

impl EditorSession {
    fn target_entity(&self, request: &Json) -> Result<u64, Failure> {
        match request.get("entity") {
            Some(value) => value
                .as_u64()
                .ok_or_else(|| Failure::bad_request("`entity` precisa ser um id inteiro")),
            None => self.selection.ok_or_else(|| {
                Failure::new(
                    "EDITOR_NO_SELECTION",
                    "nenhuma entidade está selecionada",
                    "Clique em um objeto no viewport ou escolha um item na Hierarquia.",
                )
            }),
        }
    }

    /// The editable mesh of an entity and its transform.
    pub(crate) fn mesh_of(&self, entity: u64) -> Option<(PolygonMesh, Transform)> {
        let world = &self.project.as_ref()?.world;
        let handle = world.handle_of(PersistentId(entity)).ok()?;
        let Ok(Value::Mesh(mesh)) = world.get(handle, "Mesh", "geometry") else {
            return None;
        };
        Some((mesh.clone(), self.transform_of_entity(entity)))
    }

    fn transform_of_entity(&self, entity: u64) -> Transform {
        let vector = |column: &str, default: Vec3| {
            self.world()
                .ok()
                .and_then(|world| crate::session::vec3_of(world, entity, "Transform", column))
                .unwrap_or(default)
        };
        Transform {
            position: vector("position", Vec3::ZERO),
            rotation_degrees: vector("rotation_degrees", Vec3::ZERO),
            scale: vector("scale", Vec3::ONE),
        }
    }

    /// A brush's shape as an editable mesh (in its own coordinates) and its color.
    fn brush_as_mesh(&self, entity: u64) -> Result<(PolygonMesh, Json), Failure> {
        let item = self
            .item_of(entity)
            .filter(|item| item.shape != Shape::Mesh)
            .ok_or_else(|| needs_mesh(entity))?;
        let data = build_mesh(item.shape, item.size, item.detail).map_err(|error| {
            Failure::new(
                "EDITOR_BRUSH_INVALID",
                &error.to_string(),
                "Corrija o tamanho do brush no Inspetor.",
            )
        })?;
        let positions: Vec<Vec3> = data
            .positions
            .iter()
            .map(|[x, y, z]| Vec3::new(*x, *y, *z))
            .collect();
        let triangles: Vec<[u32; 3]> = data.indices.as_chunks::<3>().0.to_vec();
        let mesh = petunia_mesh::from_triangles(&positions, &triangles)?;
        let color = self
            .world()?
            .handle_of(PersistentId(entity))
            .ok()
            .and_then(|handle| {
                self.world()
                    .ok()?
                    .get(handle, "Brush", "color")
                    .ok()
                    .cloned()
            })
            .map_or(Json::Null, |color| color.to_json());
        Ok((mesh, color))
    }

    /// Records that turn a brush into a mesh in place (same entity, name, transform, scripts).
    fn convert_records(entity: u64, mesh: &PolygonMesh, color: &Json) -> Vec<CommandRecord> {
        let mut values = serde_json::Map::new();
        values.insert("geometry".to_string(), mesh.to_text_json());
        if !color.is_null() {
            values.insert("color".to_string(), color.clone());
        }
        vec![
            CommandRecord::new("row.remove", json!({ "entity": entity, "table": "Brush" })),
            CommandRecord::new(
                "row.create",
                json!({ "entity": entity, "table": "Mesh", "values": values }),
            ),
        ]
    }

    fn set_geometry(entity: u64, mesh: &PolygonMesh) -> CommandRecord {
        CommandRecord::new(
            "row.set",
            json!({ "entity": entity, "table": "Mesh", "values": { "geometry": mesh.to_text_json() } }),
        )
    }

    /// The current component selection of `entity`, empty when the mesh changed since. Never
    /// carries the preview (that would copy a whole mesh on every frame).
    fn components(&self, entity: u64, mesh: &PolygonMesh) -> ComponentSelection {
        match &self.components {
            Some(selection)
                if selection.entity == entity && selection.fingerprint == mesh.fingerprint() =>
            {
                ComponentSelection {
                    entity: selection.entity,
                    fingerprint: selection.fingerprint,
                    faces: selection.faces.clone(),
                    edges: selection.edges.clone(),
                    vertices: selection.vertices.clone(),
                    preview: None,
                }
            }
            _ => ComponentSelection {
                entity,
                fingerprint: mesh.fingerprint(),
                ..ComponentSelection::default()
            },
        }
    }

    /// Runs the op named `op` (`mesh.*`). See the module notes.
    pub(crate) fn mesh_request(&mut self, op: &str, request: &Json) -> Result<Json, Failure> {
        match op {
            "mesh.preview" => return self.mesh_preview(request),
            "mesh.preview_end" => {
                let commit = request
                    .get("commit")
                    .and_then(Json::as_bool)
                    .unwrap_or(true);
                return self.finish_preview(commit);
            }
            // Looking does not end a drag; anything that edits or chooses does (keeping it).
            "mesh.pick" | "mesh.info" => {}
            _ => {
                self.finish_preview(true)?;
            }
        }
        match op {
            "mesh.convert" => self.mesh_convert(request),
            "mesh.boolean" => self.mesh_boolean(request),
            "mesh.pick" => self.mesh_pick(request),
            "mesh.select" => self.mesh_select(request),
            "mesh.info" => self.mesh_info(request),
            _ => self.mesh_edit(op, request),
        }
    }

    fn mesh_convert(&mut self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        if self.mesh_of(entity).is_some() {
            return Err(Failure::new(
                "EDITOR_ALREADY_A_MESH",
                "o objeto já é uma malha editável",
                "Edite as faces e arestas direto.",
            ));
        }
        let (mesh, color) = self.brush_as_mesh(entity)?;
        let records = Self::convert_records(entity, &mesh, &color);
        self.execute(&CommandRecord::new(
            BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        self.components = None;
        Ok(json!({ "entity": entity, "vertices": mesh.positions.len(), "faces": mesh.faces.len() }))
    }

    /// A solid in world coordinates for a boolean: (mesh in its own space, transform, is a brush).
    fn solid(&self, entity: u64) -> Result<(PolygonMesh, Transform, Option<Json>), Failure> {
        if let Some((mesh, transform)) = self.mesh_of(entity) {
            return Ok((mesh, transform, None));
        }
        let (mesh, color) = self.brush_as_mesh(entity).map_err(|_| {
            Failure::new(
                "EDITOR_BOOLEAN_NEEDS_SOLID",
                &format!("a entidade {entity} não é um brush nem uma malha"),
                "Booleanos funcionam entre brushes e malhas editáveis.",
            )
        })?;
        Ok((mesh, self.transform_of_entity(entity), Some(color)))
    }

    fn mesh_boolean(&mut self, request: &Json) -> Result<Json, Failure> {
        let operation = request
            .get("operation")
            .and_then(Json::as_str)
            .and_then(BooleanOperation::parse)
            .ok_or_else(|| {
                Failure::bad_request("`operation` é `union` (fundir), `subtract` (cortar) ou `intersect` (interseção)")
            })?;
        let target = self.target_entity(request)?;
        let tool = request.get("tool").and_then(Json::as_u64).ok_or_else(|| {
            Failure::bad_request("falta `tool`: o id do objeto que corta ou se junta")
        })?;
        if tool == target {
            return Err(Failure::bad_request(
                "o objeto e a ferramenta precisam ser diferentes",
            ));
        }
        let keep_tool = request
            .get("keep_tool")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        let (target_mesh, target_transform, target_brush_color) = self.solid(target)?;
        let (tool_mesh, tool_transform, _) = self.solid(tool)?;
        // The tool into the target's own coordinates.
        let mut moved = tool_mesh;
        for point in &mut moved.positions {
            *point = to_local(&target_transform, to_world(&tool_transform, *point)).ok_or_else(
                || {
                    Failure::new(
                        "EDITOR_SCALE_ZERO",
                        "o objeto tem escala zero em algum eixo",
                        "Use uma escala diferente de zero.",
                    )
                },
            )?;
        }
        if tool_transform.scale.x * tool_transform.scale.y * tool_transform.scale.z < 0.0 {
            // A mirrored tool turns inside out; flip it back so its faces point outward.
            for face in &mut moved.faces {
                face.reverse();
            }
        }
        let result = petunia_mesh::boolean(&target_mesh, &moved, operation)?;
        let mut records = match &target_brush_color {
            Some(color) => Self::convert_records(target, &result, color),
            None => vec![Self::set_geometry(target, &result)],
        };
        if !keep_tool {
            records.push(CommandRecord::new(
                "world.remove",
                json!({ "entity": tool }),
            ));
        }
        self.execute(&CommandRecord::new(
            BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        self.selection = Some(target);
        self.components = None;
        Ok(json!({
            "entity": target,
            "vertices": result.positions.len(),
            "faces": result.faces.len(),
            "tool_removed": !keep_tool,
        }))
    }

    /// The component ops: everything that edits one mesh by faces, edges or vertices.
    fn mesh_edit(&mut self, op: &str, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        let (mesh, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
        let chosen = self.components(entity, &mesh);
        let (result, keep_faces) = compute_edit(op, &mesh, &chosen, request)?;
        self.execute(&Self::set_geometry(entity, &result))?;
        self.components = Some(selection_after(entity, &result, keep_faces));
        Ok(json!({
            "entity": entity,
            "vertices": result.positions.len(),
            "faces": result.faces.len(),
            "selection": self.components_json(),
        }))
    }

    /// Shows `operation` live with the values of this request (see [`MeshPreview`]). The first
    /// call starts the drag on the chosen parts; later calls only change the values. A value that
    /// does not work yet (an inset of 0) shows the mesh as it was and says why, without failing.
    fn mesh_preview(&mut self, request: &Json) -> Result<Json, Failure> {
        let operation = request
            .get("operation")
            .and_then(Json::as_str)
            .filter(|name| PREVIEW_OPERATIONS.contains(name))
            .ok_or_else(|| {
                Failure::bad_request("`operation` é `inset`, `extrude`, `bevel` ou `loop_cut`")
            })?;
        let op = format!("mesh.{operation}");
        let entity = self.target_entity(request)?;
        let (current, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
        let mut preview = match self.live_preview() {
            Some(preview) if preview.chosen.entity == entity && preview.op == op => preview,
            other => {
                // A drag of another operation is kept first (put back so it can be committed).
                if let Some(previous) = other {
                    if let Some(selection) = self.components.as_mut() {
                        selection.preview = Some(previous);
                    }
                    self.finish_preview(true)?;
                }
                let (current, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
                let mut chosen = self.components(entity, &current);
                // Parts named in the request become the parts of this drag.
                if let Some(faces) = numbers(request, "faces")? {
                    chosen.faces = faces.into_iter().collect();
                }
                if let Some(picked) = edges(request, "edges")? {
                    chosen.edges = picked.into_iter().map(edge_key).collect();
                }
                if let Some(edge) = request.get("edge") {
                    chosen.edges = [edge_key(edge_of(edge, "edge")?)].into_iter().collect();
                }
                Box::new(MeshPreview {
                    op: op.clone(),
                    shown: current.fingerprint(),
                    base: current,
                    chosen,
                    keep_faces: Vec::new(),
                })
            }
        };
        // The parts come from the drag, never from this request (they are numbers into `base`).
        let mut values = request.clone();
        if let Some(object) = values.as_object_mut() {
            for key in ["faces", "edges", "edge", "vertices", "entity"] {
                object.remove(key);
            }
        }
        let outcome = compute_edit(&op, &preview.base, &preview.chosen, &values);
        let (shown, reply) = match outcome {
            Ok((result, keep)) => {
                preview.keep_faces = keep;
                let reply = json!({
                    "valid": true,
                    "vertices": result.positions.len(),
                    "faces": result.faces.len(),
                });
                (result, reply)
            }
            Err(failure) => (
                preview.base.clone(),
                json!({ "valid": false, "reason": failure.to_json() }),
            ),
        };
        if shown.fingerprint() != current.fingerprint() {
            self.gesture_preview(entity, "Mesh", "geometry", &shown.to_text_json())?;
        }
        preview.shown = shown.fingerprint();
        let chosen = preview.chosen.clone();
        self.components = Some(ComponentSelection {
            preview: Some(preview),
            ..chosen
        });
        Ok(reply)
    }

    /// The drag in progress, unless its gesture was ended elsewhere (then it is forgotten).
    fn live_preview(&mut self) -> Option<Box<MeshPreview>> {
        let preview = self.components.as_mut()?.preview.take()?;
        let showing = self
            .mesh_of(preview.chosen.entity)
            .is_some_and(|(mesh, _)| mesh.fingerprint() == preview.shown);
        showing.then_some(preview)
    }

    /// Ends the drag: `commit` keeps the last shown mesh as ONE undo step (nothing when it did not
    /// change), otherwise the mesh goes back to how it was. No drag: nothing happens.
    fn finish_preview(&mut self, commit: bool) -> Result<Json, Failure> {
        let Some(preview) = self.live_preview() else {
            return Ok(Json::Null);
        };
        let entity = preview.chosen.entity;
        let changed = preview.shown != preview.base.fingerprint();
        if commit && changed {
            self.gesture_commit()?;
            let (result, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
            self.components = Some(selection_after(entity, &result, preview.keep_faces));
            return Ok(json!({ "committed": true, "selection": self.components_json() }));
        }
        self.gesture_cancel()?;
        self.components = Some(preview.chosen);
        Ok(json!({ "committed": false }))
    }

    /// The face, edge and vertex under the mouse on an editable mesh (the selected one when the
    /// request does not name an entity; else the nearest mesh under the mouse).
    fn mesh_pick(&mut self, request: &Json) -> Result<Json, Failure> {
        let pointer = Pointer {
            x: float(request, "x", f32::NAN)?,
            y: float(request, "y", f32::NAN)?,
            width: float(request, "width", 0.0)?,
            height: float(request, "height", 0.0)?,
        };
        if !(pointer.x.is_finite() && pointer.y.is_finite())
            || pointer.width < 1.0
            || pointer.height < 1.0
        {
            return Err(Failure::bad_request(
                "`mesh.pick` precisa de `x`, `y`, `width` e `height` (pixels do viewport)",
            ));
        }
        let ray = self
            .view_for(pointer)
            .ray(pointer.mouse())
            .ok_or_else(|| Failure::bad_request("ponto fora do viewport"))?;
        let candidates: Vec<u64> = match request.get("entity").and_then(Json::as_u64) {
            Some(entity) => vec![entity],
            None => self
                .draw_items()
                .into_iter()
                .filter(|item| item.shape == Shape::Mesh)
                .map(|item| item.entity)
                .collect(),
        };
        let mut best: Option<(f32, Json)> = None;
        for entity in candidates {
            if let Some((distance, hit)) = self.pick_component(entity, &ray)
                && best.as_ref().is_none_or(|(nearest, _)| distance < *nearest)
            {
                best = Some((distance, hit));
            }
        }
        Ok(best.map_or(Json::Null, |(_, hit)| hit))
    }

    fn pick_component(&self, entity: u64, ray: &Ray) -> Option<(f32, Json)> {
        let (mesh, transform) = self.mesh_of(entity)?;
        let origin = to_local(&transform, ray.origin)?;
        // Not normalized on purpose: a distance along it equals the distance along the world ray.
        let direction = to_local(&transform, ray.origin + ray.direction)? - origin;
        let point = |vertex: u32| {
            mesh.positions
                .get(vertex as usize)
                .copied()
                .unwrap_or(Vec3::ZERO)
        };
        let mut best: Option<(f32, usize)> = None;
        let mut triangles = Vec::new();
        for (number, face) in mesh.faces.iter().enumerate() {
            triangles.clear();
            mesh.triangulate_face(face, &mut triangles);
            for [a, b, c] in &triangles {
                if let Some(distance) =
                    ray_triangle(origin, direction, point(*a), point(*b), point(*c))
                    && best.is_none_or(|(nearest, _)| distance < nearest)
                {
                    best = Some((distance, number));
                }
            }
        }
        let (distance, face_number) = best?;
        let face = mesh.faces.get(face_number)?;
        let hit = origin + direction * distance;
        let nearest_edge = (0..face.len())
            .map(|corner| {
                let edge = [
                    face.get(corner).copied().unwrap_or(0),
                    face.get((corner + 1) % face.len()).copied().unwrap_or(0),
                ];
                (
                    distance_to_segment(hit, point(edge[0]), point(edge[1])),
                    edge,
                )
            })
            .min_by(|first, second| first.0.total_cmp(&second.0))?;
        let nearest_vertex = face
            .iter()
            .map(|vertex| ((hit - point(*vertex)).length(), *vertex))
            .min_by(|first, second| first.0.total_cmp(&second.0))?;
        let along = point(nearest_edge.1[1]) - point(nearest_edge.1[0]);
        let fraction = if along.length_squared() > f32::EPSILON {
            ((hit - point(nearest_edge.1[0])).dot(along) / along.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let world_point = to_world(&transform, hit);
        Some((
            distance,
            json!({
                "entity": entity,
                "face": face_number,
                "edge": nearest_edge.1,
                "edge_fraction": fraction,
                "vertex": nearest_vertex.1,
                "point": { "x": world_point.x, "y": world_point.y, "z": world_point.z },
            }),
        ))
    }

    /// Chooses faces, edges or vertices of the selected mesh. `mode`: `replace` (default), `add`,
    /// `toggle` or `clear`.
    fn mesh_select(&mut self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        let (mesh, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
        let mode = request
            .get("mode")
            .and_then(Json::as_str)
            .unwrap_or("replace");
        let mut selection = self.components(entity, &mesh);
        let faces = numbers(request, "faces")?.unwrap_or_default();
        let picked_edges = edges(request, "edges")?.unwrap_or_default();
        let vertices = numbers(request, "vertices")?.unwrap_or_default();
        if let Some(face) = faces
            .iter()
            .find(|face| **face as usize >= mesh.faces.len())
        {
            return Err(Failure::new(
                "MESH_UNKNOWN_FACE",
                &format!("a face {face} não existe"),
                "Escolha de novo: a malha mudou.",
            ));
        }
        if let Some(vertex) = vertices
            .iter()
            .find(|vertex| **vertex as usize >= mesh.positions.len())
        {
            return Err(Failure::new(
                "MESH_UNKNOWN_VERTEX",
                &format!("o vértice {vertex} não existe"),
                "Escolha de novo: a malha mudou.",
            ));
        }
        let is_edge = |edge: &[u32; 2]| {
            mesh.faces.iter().any(|face| {
                (0..face.len()).any(|corner| {
                    let pair = [
                        face.get(corner).copied().unwrap_or(0),
                        face.get((corner + 1) % face.len()).copied().unwrap_or(0),
                    ];
                    edge_key(pair) == edge_key(*edge)
                })
            })
        };
        if let Some(edge) = picked_edges.iter().find(|edge| !is_edge(edge)) {
            return Err(Failure::new(
                "MESH_UNKNOWN_EDGE",
                &format!("{} – {} não é uma aresta da malha", edge[0], edge[1]),
                "Escolha de novo: a malha mudou.",
            ));
        }
        fn apply<T: Ord + Copy>(set: &mut BTreeSet<T>, items: &[T], mode: &str) {
            match mode {
                "add" => set.extend(items.iter().copied()),
                "toggle" => {
                    for item in items {
                        if !set.remove(item) {
                            set.insert(*item);
                        }
                    }
                }
                _ => {
                    set.clear();
                    set.extend(items.iter().copied());
                }
            }
        }
        match mode {
            "clear" => {
                selection.faces.clear();
                selection.edges.clear();
                selection.vertices.clear();
            }
            "replace" | "add" | "toggle" => {
                if mode == "replace" {
                    selection.faces.clear();
                    selection.edges.clear();
                    selection.vertices.clear();
                }
                apply(&mut selection.faces, &faces, mode);
                let normalized: Vec<[u32; 2]> =
                    picked_edges.iter().map(|edge| edge_key(*edge)).collect();
                apply(&mut selection.edges, &normalized, mode);
                apply(&mut selection.vertices, &vertices, mode);
            }
            other => {
                return Err(Failure::bad_request(&format!(
                    "modo `{other}` desconhecido (use replace, add, toggle ou clear)"
                )));
            }
        }
        self.selection = Some(entity);
        self.components = Some(selection);
        Ok(self.components_json())
    }

    pub(crate) fn components_json(&self) -> Json {
        let Some(selection) = &self.components else {
            return Json::Null;
        };
        let live = self
            .mesh_of(selection.entity)
            .is_some_and(|(mesh, _)| mesh.fingerprint() == selection.fingerprint);
        if !live {
            return Json::Null;
        }
        json!({
            "entity": selection.entity,
            "faces": selection.faces,
            "edges": selection.edges,
            "vertices": selection.vertices,
        })
    }

    /// Counts (and, with `"geometry": true`, the whole mesh in its text form) for the inspector
    /// and for agents.
    fn mesh_info(&self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        let (mesh, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
        let edge_count: usize = {
            let mut unique = BTreeSet::new();
            for face in &mesh.faces {
                for corner in 0..face.len() {
                    unique.insert(edge_key([
                        face.get(corner).copied().unwrap_or(0),
                        face.get((corner + 1) % face.len()).copied().unwrap_or(0),
                    ]));
                }
            }
            unique.len()
        };
        let mut info = json!({
            "entity": entity,
            "name": self.world().map(|world| display_name(world, entity)).unwrap_or_default(),
            "vertices": mesh.positions.len(),
            "faces": mesh.faces.len(),
            "edges": edge_count,
            "triangles": mesh.triangles().len(),
            "solid": petunia_mesh::is_solid(&mesh),
            "selection": self.components_json(),
        });
        if request.get("geometry").and_then(Json::as_bool) == Some(true)
            && let Some(object) = info.as_object_mut()
        {
            object.insert("geometry".to_string(), mesh.to_text_json());
        }
        Ok(info)
    }

    /// The selected mesh's wireframe and its chosen faces, edges and vertices, on top of the view.
    pub(crate) fn mesh_overlay(&self, items: &mut Vec<OverlayItem>) {
        let Some(entity) = self.selection else {
            return;
        };
        let Some((mesh, transform)) = self.mesh_of(entity) else {
            return;
        };
        let world_point = |vertex: u32| {
            to_world(
                &transform,
                mesh.positions
                    .get(vertex as usize)
                    .copied()
                    .unwrap_or(Vec3::ZERO),
            )
        };
        let mut drawn = BTreeSet::new();
        for face in &mesh.faces {
            for corner in 0..face.len() {
                if drawn.len() >= MAX_WIREFRAME_EDGES {
                    break;
                }
                let edge = edge_key([
                    face.get(corner).copied().unwrap_or(0),
                    face.get((corner + 1) % face.len()).copied().unwrap_or(0),
                ]);
                if drawn.insert(edge) {
                    items.push(OverlayItem::Line {
                        from: world_point(edge[0]),
                        to: world_point(edge[1]),
                        color: WIRE_COLOR,
                        width_pixels: 1.0,
                    });
                }
            }
        }
        let selection = self.components(entity, &mesh);
        for face in &selection.faces {
            let Some(corners) = mesh.faces.get(*face as usize) else {
                continue;
            };
            for corner in 0..corners.len() {
                items.push(OverlayItem::Line {
                    from: world_point(corners.get(corner).copied().unwrap_or(0)),
                    to: world_point(
                        corners
                            .get((corner + 1) % corners.len())
                            .copied()
                            .unwrap_or(0),
                    ),
                    color: FACE_SELECTION_COLOR,
                    width_pixels: 3.0,
                });
            }
        }
        for edge in &selection.edges {
            items.push(OverlayItem::Line {
                from: world_point(edge[0]),
                to: world_point(edge[1]),
                color: EDGE_SELECTION_COLOR,
                width_pixels: 4.0,
            });
        }
        for vertex in &selection.vertices {
            items.push(OverlayItem::Square {
                center: world_point(*vertex),
                color: VERTEX_SELECTION_COLOR,
                size_pixels: 8.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use petunia_math::{PolygonMesh, Vec3};
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn refused(session: &mut EditorSession, request: Json) -> String {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], false, "{request} -> {reply}");
        reply["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
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

    /// A brush of `shape` placed at `position` (away from the sample scene's own objects).
    fn brush_at(session: &mut EditorSession, shape: &str, position: Vec3) -> u64 {
        let entity = call(session, json!({ "op": "create.brush", "shape": shape }))["entity"]
            .as_u64()
            .expect("brush");
        call(
            session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": entity, "table": "Transform",
                "values": { "position": { "x": position.x, "y": position.y, "z": position.z } }
            }}),
        );
        entity
    }

    fn geometry(session: &EditorSession, entity: u64) -> PolygonMesh {
        session.mesh_of(entity).expect("mesh").0
    }

    fn volume(mesh: &PolygonMesh) -> f32 {
        mesh.triangles()
            .iter()
            .map(|[a, b, c]| {
                let point = |vertex: &u32| mesh.positions[*vertex as usize];
                point(a).dot(point(b).cross(point(c))) / 6.0
            })
            .sum()
    }

    /// Camera straight above `point`, so the middle of an 800 × 600 view looks down at it.
    fn look_down_at(session: &mut EditorSession, point: Vec3) {
        session.camera.camera.position = point + Vec3::new(0.0, 10.0, 0.0);
        session.camera.camera.yaw_degrees = 0.0;
        session.camera.camera.pitch_degrees = -89.9;
    }

    fn center_pick(session: &mut EditorSession) -> Json {
        call(
            session,
            json!({ "op": "mesh.pick", "x": 400.0, "y": 300.0, "width": 800.0, "height": 600.0 }),
        )
    }

    #[test]
    fn a_brush_becomes_an_editable_mesh_in_place_and_undo_brings_it_back() {
        let (mut session, root) = opened("mesh_convert");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        let converted = call(&mut session, json!({ "op": "mesh.convert" }));
        assert_eq!(converted["entity"], json!(entity));
        assert_eq!(converted["vertices"], 8);
        assert_eq!(converted["faces"], 6);
        let described = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": entity }),
        );
        let tables = described["tables"].to_string();
        assert!(
            tables.contains("\"Mesh\"") && !tables.contains("\"Brush\""),
            "{tables}"
        );
        assert!(tables.contains("\"Transform\"") && tables.contains("\"Name\""));
        let info = call(&mut session, json!({ "op": "mesh.info" }));
        assert_eq!(info["solid"], true);
        assert_eq!(info["edges"], 12);
        // Drawn as a mesh, with the same size as the brush it was.
        let item = session
            .draw_items()
            .into_iter()
            .find(|item| item.entity == entity)
            .expect("drawn");
        assert_eq!(item.shape, petunia_render::Shape::Mesh);
        assert_eq!(item.size, Vec3::splat(2.0));
        assert_eq!(
            refused(&mut session, json!({ "op": "mesh.convert" })),
            "EDITOR_ALREADY_A_MESH"
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        assert!(session.mesh_of(entity).is_none());
        assert_eq!(
            session.item_of(entity).map(|item| item.shape),
            Some(petunia_render::Shape::Box)
        );
        call(&mut session, json!({ "op": "edit.redo" }));
        assert!(session.mesh_of(entity).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pick_select_inset_extrude_loop_cut_and_bevel_each_undo_in_one_step() {
        let (mut session, root) = opened("mesh_model");
        let center = Vec3::new(40.0, 1.0, 40.0);
        let entity = brush_at(&mut session, "box", center);
        call(&mut session, json!({ "op": "mesh.convert" }));
        let original = geometry(&session, entity);

        // The mouse in the middle of the view, looking down: the top face.
        look_down_at(&mut session, center);
        let hit = center_pick(&mut session);
        assert_eq!(hit["entity"], json!(entity));
        let top = hit["face"].as_u64().expect("face") as usize;
        assert!(original.face_normal(&original.faces[top]).y > 0.99, "{hit}");
        assert!(
            (hit["point"]["y"].as_f64().unwrap_or(0.0) - 2.0).abs() < 1.0e-3,
            "{hit}"
        );

        // Inset the chosen face (from the selection), then dig the inner face down.
        call(&mut session, json!({ "op": "mesh.select", "faces": [top] }));
        let inset = call(
            &mut session,
            json!({ "op": "mesh.inset", "distance": 0.25 }),
        );
        assert_eq!(inset["faces"], 10);
        assert_eq!(
            inset["selection"]["faces"].as_array().map(Vec::len),
            Some(1)
        );
        let after_inset = geometry(&session, entity);
        call(
            &mut session,
            json!({ "op": "mesh.extrude", "distance": -0.5 }),
        );
        let dug = geometry(&session, entity);
        assert!(
            (volume(&dug) - (8.0 - 1.5 * 1.5 * 0.5)).abs() < 1.0e-3,
            "{}",
            volume(&dug)
        );
        assert!(petunia_mesh::is_solid(&dug));

        // A loop cut and a bevel on the original box edges (vertex numbers are stable here).
        let cut = call(
            &mut session,
            json!({ "op": "mesh.loop_cut", "edge": [0, 1], "cuts": 2 }),
        );
        assert!(cut["faces"].as_u64().unwrap_or(0) > 14, "{cut}");
        let info = call(&mut session, json!({ "op": "mesh.info" }));
        assert_eq!(info["solid"], true, "{info}");

        // Every step is exactly one undo.
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), dug);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), after_inset);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), original);
        // The component selection belonged to a mesh that no longer exists.
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["mesh_selection"],
            Json::Null
        );
        assert_eq!(
            refused(&mut session, json!({ "op": "mesh.inset" })),
            "MESH_NO_FACES"
        );

        let beveled = call(
            &mut session,
            json!({ "op": "mesh.bevel", "edges": [[0, 1]], "width": 0.2, "segments": 2 }),
        );
        assert_eq!(beveled["faces"], 8);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_saved_mesh_opens_again_exactly_and_diffs_line_by_line() {
        let (mut session, root) = opened("mesh_save");
        let entity = brush_at(&mut session, "cylinder", Vec3::new(40.0, 1.5, 40.0));
        call(&mut session, json!({ "op": "mesh.convert" }));
        call(&mut session, json!({ "op": "mesh.select", "faces": [0] }));
        call(&mut session, json!({ "op": "mesh.inset", "distance": 0.1 }));
        let saved = geometry(&session, entity);
        let path = call(&mut session, json!({ "op": "project.save" }))["path"]
            .as_str()
            .expect("path")
            .to_string();
        let text = std::fs::read_to_string(&path).expect("scene");
        assert!(text.contains("\"geometry\""));
        // One line per vertex: a moved vertex changes one line of the file.
        let first = &saved.positions[0];
        assert!(
            text.contains(&format!("\"{} {} {}\"", first.x, first.y, first.z)),
            "positions are written as \"x y z\" lines"
        );
        let mut reopened = EditorSession::new().expect("session");
        call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        assert_eq!(geometry(&reopened, entity), saved);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn booleans_cut_join_and_intersect_brushes_and_meshes_in_one_step() {
        let (mut session, root) = opened("mesh_boolean");
        // A 4 m wall cut by a doorway brush; both are brushes, the wall becomes a mesh.
        let wall = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": wall, "table": "Brush", "values": { "size": { "x": 4.0, "y": 2.0, "z": 0.5 } }
            }}),
        );
        let door = brush_at(&mut session, "box", Vec3::new(40.0, 0.5, 40.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": door, "table": "Brush", "values": { "size": { "x": 1.0, "y": 2.0, "z": 2.0 } }
            }}),
        );
        let before = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"].clone();
        let cut = call(
            &mut session,
            json!({ "op": "mesh.boolean", "entity": wall, "tool": door, "operation": "subtract" }),
        );
        assert_eq!(cut["tool_removed"], true);
        let mesh = geometry(&session, wall);
        assert!(petunia_mesh::is_solid(&mesh));
        // 4 × 2 × 0.5 minus the doorway 1 × 1.5 × 0.5 (the door reaches y = 1.5 of the wall).
        assert!(
            (volume(&mesh) - (4.0 - 0.75)).abs() < 1.0e-3,
            "{}",
            volume(&mesh)
        );
        // One undo restores both brushes.
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"],
            before
        );
        assert!(session.mesh_of(wall).is_none() && session.item_of(door).is_some());

        // A rotated, stretched tool is carried into the target's space: a 1 m cube turned 45°
        // and stretched 2× upward still removes exactly its own volume.
        call(&mut session, json!({ "op": "edit.redo" }));
        let block = brush_at(&mut session, "box", Vec3::new(50.0, 2.0, 50.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": block, "table": "Brush", "values": { "size": { "x": 4.0, "y": 4.0, "z": 4.0 } }
            }}),
        );
        let pillar = brush_at(&mut session, "box", Vec3::new(50.0, 2.0, 50.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": pillar, "table": "Transform", "values": {
                    "rotation_degrees": { "x": 0.0, "y": 45.0, "z": 0.0 },
                    "scale": { "x": 0.5, "y": 1.0, "z": 0.5 }
                }
            }}),
        );
        call(
            &mut session,
            json!({ "op": "mesh.boolean", "entity": block, "tool": pillar, "operation": "intersect", "keep_tool": true }),
        );
        // The pillar is 1 × 2 × 1 m inside the block, so the intersection is the pillar itself.
        assert!((volume(&geometry(&session, block)) - 2.0).abs() < 1.0e-3);
        assert!(session.item_of(pillar).is_some(), "keep_tool keeps it");
        let joined = call(
            &mut session,
            json!({ "op": "mesh.boolean", "entity": block, "tool": wall, "operation": "union" }),
        );
        assert!(joined["faces"].as_u64().unwrap_or(0) > 6);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dragging_an_inset_shows_it_live_and_keeps_one_undo_step() {
        let (mut session, root) = opened("mesh_preview");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        call(&mut session, json!({ "op": "mesh.convert" }));
        let original = geometry(&session, entity);
        let depth = session.dispatcher.undo_depth().0;
        let top = (0..original.faces.len())
            .find(|face| original.face_normal(&original.faces[*face]).y > 0.99)
            .expect("top");
        call(&mut session, json!({ "op": "mesh.select", "faces": [top] }));
        let depth = depth.max(session.dispatcher.undo_depth().0);

        // The start of a drag (distance 0) is not an inset yet: the mesh stays, with a reason.
        let start = call(
            &mut session,
            json!({ "op": "mesh.preview", "operation": "inset", "distance": 0.0 }),
        );
        assert_eq!(start["valid"], false, "{start}");
        assert_eq!(start["reason"]["code"], "MESH_BAD_PARAMETER");
        assert_eq!(geometry(&session, entity), original);
        // Each step is computed from the original mesh, never from the previous step.
        let mut shown = Vec::new();
        for distance in [0.1, 0.3, 0.6, 0.3] {
            let step = call(
                &mut session,
                json!({ "op": "mesh.preview", "operation": "inset", "distance": distance }),
            );
            assert_eq!(step["valid"], true, "{step}");
            assert_eq!(step["faces"], 10, "one ring, not a ring per step");
            shown.push(geometry(&session, entity));
        }
        assert_eq!(
            shown[1], shown[3],
            "back to 0.3 m is exactly the 0.3 m inset"
        );
        assert_eq!(
            session.dispatcher.undo_depth().0,
            depth,
            "nothing in the history yet"
        );
        // Picking does not end the drag.
        look_down_at(&mut session, Vec3::new(40.0, 1.0, 40.0));
        center_pick(&mut session);
        let kept = call(&mut session, json!({ "op": "mesh.preview_end" }));
        assert_eq!(kept["committed"], true, "{kept}");
        assert_eq!(kept["selection"]["faces"].as_array().map(Vec::len), Some(1));
        assert_eq!(geometry(&session, entity), shown[3]);
        assert_eq!(session.dispatcher.undo_depth().0, depth + 1);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), original);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cancelled_drag_leaves_no_trace_and_another_op_keeps_the_drag() {
        let (mut session, root) = opened("mesh_preview_cancel");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        call(&mut session, json!({ "op": "mesh.convert" }));
        let original = geometry(&session, entity);
        let depth = session.dispatcher.undo_depth().0;
        call(
            &mut session,
            json!({ "op": "mesh.preview", "operation": "extrude", "faces": [0], "distance": 0.8 }),
        );
        assert_ne!(geometry(&session, entity), original);
        let cancelled = call(
            &mut session,
            json!({ "op": "mesh.preview_end", "commit": false }),
        );
        assert_eq!(cancelled["committed"], false);
        assert_eq!(geometry(&session, entity), original);
        assert_eq!(session.dispatcher.undo_depth().0, depth);

        // Loop cut slide while dragging, then a different op: the drag is kept first.
        for slide in [-0.5, 0.0, 0.5] {
            let step = call(
                &mut session,
                json!({ "op": "mesh.preview", "operation": "loop_cut", "edge": [0, 1], "slide": slide }),
            );
            assert_eq!(step["valid"], true, "{step}");
        }
        let after_cut = geometry(&session, entity);
        // Switching to another previewed op keeps the first drag as its own step.
        call(
            &mut session,
            json!({ "op": "mesh.preview", "operation": "extrude", "faces": [0], "distance": 0.4 }),
        );
        call(&mut session, json!({ "op": "mesh.flip" }));
        assert_eq!(
            session.dispatcher.undo_depth().0,
            depth + 3,
            "cut, extrude, flip"
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), after_cut);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(geometry(&session, entity), original);
        assert_eq!(
            refused(
                &mut session,
                json!({ "op": "mesh.preview", "operation": "boolean" })
            ),
            "EDITOR_BAD_REQUEST"
        );
        // Ending when nothing is being dragged does nothing.
        assert_eq!(
            call(&mut session, json!({ "op": "mesh.preview_end" })),
            Json::Null
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_requests_are_refused_and_leave_the_world_and_history_alone() {
        let (mut session, root) = opened("mesh_hostile");
        let brush = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        assert_eq!(
            refused(&mut session, json!({ "op": "mesh.inset", "faces": [0] })),
            "EDITOR_NOT_A_MESH"
        );
        call(&mut session, json!({ "op": "mesh.convert" }));
        let mesh = geometry(&session, brush);
        let depth = session.dispatcher.undo_depth().0;
        let cases = [
            json!({ "op": "mesh.inset", "faces": [99] }),
            json!({ "op": "mesh.inset", "faces": [-1] }),
            json!({ "op": "mesh.inset", "faces": "0" }),
            json!({ "op": "mesh.inset", "faces": [0], "distance": "far" }),
            json!({ "op": "mesh.inset", "faces": [0], "distance": 1.0e12 }),
            json!({ "op": "mesh.extrude", "faces": [0], "grouping": "sideways" }),
            json!({ "op": "mesh.loop_cut", "edge": [0, 6] }),
            json!({ "op": "mesh.loop_cut", "edge": [0] }),
            json!({ "op": "mesh.loop_cut", "edge": [0, 1], "cuts": 1000 }),
            json!({ "op": "mesh.bevel", "edges": [[0, 1]], "segments": 0 }),
            json!({ "op": "mesh.merge", "vertices": [0] }),
            json!({ "op": "mesh.knife", "start": { "edge": [0, 1] } }),
            json!({ "op": "mesh.boolean", "tool": brush, "operation": "union" }),
            json!({ "op": "mesh.boolean", "tool": 999_999, "operation": "union" }),
            json!({ "op": "mesh.boolean", "tool": brush, "operation": "melt" }),
            json!({ "op": "mesh.select", "faces": [99] }),
            json!({ "op": "mesh.select", "edges": [[0, 6]] }),
            json!({ "op": "mesh.select", "faces": [0], "mode": "sometimes" }),
            json!({ "op": "mesh.pick", "x": 1.0 }),
            json!({ "op": "mesh.teleport" }),
            json!({ "op": "mesh.inset", "entity": 999_999, "faces": [0] }),
        ];
        for request in cases {
            refused(&mut session, request);
        }
        assert_eq!(geometry(&session, brush), mesh);
        assert_eq!(session.dispatcher.undo_depth().0, depth);
        let _ = std::fs::remove_dir_all(&root);
    }
}
