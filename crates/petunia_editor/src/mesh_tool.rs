//! Boolean cuts between brushes and stored meshes (DR-51).
//!
//! A brush stays a brush until a boolean needs a mesh: the target becomes a `Mesh` row in one
//! undo step. There is no component editing (no inset, knife, bevel or edit mode).

use std::collections::BTreeSet;

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_math::{PolygonMesh, Transform, Vec3};
use petunia_mesh::{BooleanOperation, MeshEditError};
use petunia_render::{Shape, build_mesh};
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::{inverse_rotate, rotate_vector, rotation_from_euler};
use crate::session::{EditorSession, display_name};

impl From<MeshEditError> for Failure {
    fn from(error: MeshEditError) -> Self {
        Failure::new(error.code, &error.message, &error.suggestion)
    }
}

fn edge_key(edge: [u32; 2]) -> [u32; 2] {
    [edge[0].min(edge[1]), edge[0].max(edge[1])]
}

fn to_world(transform: &Transform, local: Vec3) -> Vec3 {
    let scale = transform.scale;
    let scaled = Vec3::new(local.x * scale.x, local.y * scale.y, local.z * scale.z);
    transform.position + rotate_vector(&rotation_from_euler(transform.rotation_degrees), scaled)
}

/// The inverse of [`to_world`]. `None` when a scale is zero.
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

fn needs_mesh(entity: u64) -> Failure {
    Failure::new(
        "EDITOR_NOT_A_MESH",
        &format!("a entidade {entity} não é um brush nem uma malha"),
        "Escolha uma caixa, um cilindro ou uma malha já gravada.",
    )
}

impl EditorSession {
    /// The stored mesh of an entity and its transform.
    pub(crate) fn mesh_of(&self, entity: u64) -> Option<(PolygonMesh, Transform)> {
        let world = &self.project.as_ref()?.world;
        let handle = world.handle_of(PersistentId(entity)).ok()?;
        let Ok(Value::Mesh(mesh)) = world.get(handle, "Mesh", "geometry") else {
            return None;
        };
        Some((mesh.clone(), self.transform_of_entity(entity)))
    }

    pub(crate) fn transform_of_entity(&self, entity: u64) -> Transform {
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

    /// A brush's shape as a mesh (in its own coordinates) and its color.
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

    /// `mesh.convert`, `mesh.boolean`, `mesh.info` and `mesh.loop_cut` (DR-53).
    pub(crate) fn mesh_request(&mut self, op: &str, request: &Json) -> Result<Json, Failure> {
        match op {
            "mesh.convert" => self.mesh_convert(request),
            "mesh.boolean" => self.mesh_boolean(request),
            "mesh.info" => self.mesh_info(request),
            "mesh.loop_cut" => self.mesh_loop_cut(request),
            _ => Err(Failure::bad_request(
                "edição de malha por faces, arestas e vértices foi removida; use blocagem ou Booleano",
            )),
        }
    }

    fn mesh_loop_cut(&mut self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        let (mesh, _) = self.mesh_of(entity).ok_or_else(|| {
            Failure::new(
                "EDITOR_NOT_A_MESH",
                "o corte em laço só funciona em malha gravada (após converter ou booleano)",
                "Converta o brush para malha antes de aplicar o corte em laço.",
            )
        })?;
        let edge = match request.get("edge") {
            Some(value) => {
                let pair = value
                    .as_array()
                    .ok_or_else(|| Failure::bad_request("`edge` precisa ser um par [v1, v2]"))?;
                if let [first_val, second_val] = pair.as_slice() {
                    let a = first_val
                        .as_u64()
                        .ok_or_else(|| Failure::bad_request("vértice inválido"))?
                        as u32;
                    let b = second_val
                        .as_u64()
                        .ok_or_else(|| Failure::bad_request("vértice inválido"))?
                        as u32;
                    [a, b]
                } else {
                    return Err(Failure::bad_request("`edge` precisa ter 2 vértices"));
                }
            }
            None => {
                if self.component_kind == crate::component::ComponentKind::Edge {
                    if let Some(&first_component) = self.components.first() {
                        let edge_idx = first_component as usize;
                        let edges = crate::component::mesh_edges(&mesh);
                        let [v0, v1] = edges.get(edge_idx).copied().ok_or_else(|| {
                            Failure::new(
                                "MESH_NO_EDGES",
                                "aresta selecionada inválida",
                                "Selecione uma aresta da malha.",
                            )
                        })?;
                        [v0 as u32, v1 as u32]
                    } else if let Some(&[v0, v1]) = crate::component::mesh_edges(&mesh).first() {
                        [v0 as u32, v1 as u32]
                    } else {
                        return Err(Failure::new(
                            "MESH_NO_EDGES",
                            "o corte em laço precisa de uma malha com arestas",
                            "A malha está vazia.",
                        ));
                    }
                } else if let Some(&[v0, v1]) = crate::component::mesh_edges(&mesh).first() {
                    [v0 as u32, v1 as u32]
                } else {
                    return Err(Failure::new(
                        "MESH_NO_EDGES",
                        "o corte em laço precisa de uma malha com arestas",
                        "A malha está vazia.",
                    ));
                }
            }
        };
        let cuts = request
            .get("cuts")
            .and_then(Json::as_u64)
            .unwrap_or(1)
            .clamp(1, 32) as u32;
        let slide = request
            .get("slide")
            .and_then(Json::as_f64)
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0) as f32;
        let result = petunia_mesh::loop_cut(&mesh, edge, cuts, slide).map_err(Failure::from)?;
        let record = Self::set_geometry(entity, &result);
        self.execute(&record)?;
        Ok(json!({
            "entity": entity,
            "cuts": cuts,
            "slide": slide,
            "vertices": result.positions.len(),
            "faces": result.faces.len(),
        }))
    }

    fn mesh_convert(&mut self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        if self.mesh_of(entity).is_some() {
            return Err(Failure::new(
                "EDITOR_ALREADY_A_MESH",
                "o objeto já é uma malha",
                "Use Booleano para cortar esta malha com outro objeto.",
            ));
        }
        let (mesh, color) = self.brush_as_mesh(entity)?;
        let records = Self::convert_records(entity, &mesh, &color);
        self.execute(&CommandRecord::new(
            BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        Ok(json!({ "entity": entity, "vertices": mesh.positions.len(), "faces": mesh.faces.len() }))
    }

    /// A solid in its own coordinates: (mesh, transform, brush color when it is still a brush).
    fn solid(&self, entity: u64) -> Result<(PolygonMesh, Transform, Option<Json>), Failure> {
        if let Some((mesh, transform)) = self.mesh_of(entity) {
            return Ok((mesh, transform, None));
        }
        let (mesh, color) = self.brush_as_mesh(entity).map_err(|_| {
            Failure::new(
                "EDITOR_BOOLEAN_NEEDS_SOLID",
                &format!("a entidade {entity} não é um brush nem uma malha"),
                "Booleanos funcionam entre brushes e malhas.",
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
                Failure::bad_request(
                    "`operation` é `union` (fundir), `subtract` (cortar) ou `intersect` (interseção)",
                )
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
        Ok(json!({
            "entity": target,
            "vertices": result.positions.len(),
            "faces": result.faces.len(),
            "tool_removed": !keep_tool,
        }))
    }

    fn mesh_info(&self, request: &Json) -> Result<Json, Failure> {
        let entity = self.target_entity(request)?;
        let (mesh, _) = self.mesh_of(entity).ok_or_else(|| needs_mesh(entity))?;
        let mut unique = BTreeSet::new();
        for face in &mesh.faces {
            for corner in 0..face.len() {
                unique.insert(edge_key([
                    face.get(corner).copied().unwrap_or(0),
                    face.get((corner + 1) % face.len()).copied().unwrap_or(0),
                ]));
            }
        }
        let mut info = json!({
            "entity": entity,
            "name": self.world().map(|world| display_name(world, entity)).unwrap_or_default(),
            "vertices": mesh.positions.len(),
            "faces": mesh.faces.len(),
            "edges": unique.len(),
            "triangles": mesh.triangles().len(),
            "solid": petunia_mesh::is_solid(&mesh),
        });
        if request.get("geometry").and_then(Json::as_bool) == Some(true)
            && let Some(object) = info.as_object_mut()
        {
            object.insert("geometry".to_string(), mesh.to_text_json());
        }
        Ok(info)
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

    /// A ray against the triangles the physics uses. The solid is in the object's own coordinates.
    fn ray_hits(solid: &petunia_math::SolidShape, origin: Vec3, direction: Vec3) -> bool {
        solid.triangles.iter().any(|triangle| {
            let [first, second, third] = *triangle;
            let (Some(a), Some(b), Some(c)) = (
                solid.points.get(first as usize),
                solid.points.get(second as usize),
                solid.points.get(third as usize),
            ) else {
                return false;
            };
            let edge_a = *b - *a;
            let edge_b = *c - *a;
            let p = direction.cross(edge_b);
            let determinant = edge_a.dot(p);
            if determinant.abs() < 1.0e-8 {
                return false;
            }
            let inverse = 1.0 / determinant;
            let offset = origin - *a;
            let u = inverse * offset.dot(p);
            if !(0.0..=1.0).contains(&u) {
                return false;
            }
            let q = offset.cross(edge_a);
            let v = inverse * direction.dot(q);
            if v < 0.0 || u + v > 1.0 {
                return false;
            }
            let t = inverse * edge_b.dot(q);
            t > 1.0e-4
        })
    }

    fn opening_solid(session: &EditorSession, wall: u64) -> petunia_math::SolidShape {
        petunia_render::solid_shapes(session.world().expect("world"))
            .remove(&wall)
            .expect("wall solid")
    }

    #[test]
    fn booleans_cut_a_door_and_one_undo_restores_both_brushes() {
        let (mut session, root) = opened("mesh_boolean");
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
        assert!(
            (volume(&mesh) - (4.0 - 0.75)).abs() < 1.0e-3,
            "{}",
            volume(&mesh)
        );
        let solid = opening_solid(&session, wall);
        assert!(
            !solid.is_box,
            "a cut wall is a triangle mesh, not a box collider"
        );
        assert!(
            !ray_hits(&solid, Vec3::new(0.0, -0.5, -1.0), Vec3::new(0.0, 0.0, 1.0)),
            "the doorway must not block a ray"
        );
        assert!(
            ray_hits(&solid, Vec3::new(1.5, 0.75, -1.0), Vec3::new(0.0, 0.0, 1.0)),
            "the rest of the wall still blocks"
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"],
            before
        );
        assert!(session.mesh_of(wall).is_none() && session.item_of(door).is_some());
        let refused = session.handle(&json!({ "op": "mesh.inset" }));
        assert_eq!(refused["ok"], false);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_rotated_cutter_still_opens_a_hole_and_a_miss_changes_nothing() {
        let (mut session, root) = opened("mesh_boolean_rotated");
        let wall = brush_at(&mut session, "box", Vec3::new(10.0, 1.0, 10.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": wall, "table": "Brush", "values": { "size": { "x": 4.0, "y": 2.0, "z": 0.5 } }
            }}),
        );
        let door = brush_at(&mut session, "box", Vec3::new(10.0, 1.0, 10.0));
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": door, "table": "Brush",
                "values": {
                    "size": { "x": 1.0, "y": 1.2, "z": 2.0 },
                    "detail": 1
                }
            }}),
        );
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": door, "table": "Transform",
                "values": {
                    "position": { "x": 10.0, "y": 1.0, "z": 10.0 },
                    "rotation_degrees": { "x": 0.0, "y": 35.0, "z": 0.0 }
                }
            }}),
        );
        let cut = call(
            &mut session,
            json!({
                "op": "mesh.boolean",
                "entity": wall,
                "tool": door,
                "operation": "subtract",
                "keep_tool": true
            }),
        );
        assert_eq!(cut["tool_removed"], false);
        assert!(session.item_of(door).is_some(), "the cutter stays");
        let solid = opening_solid(&session, wall);
        assert!(!solid.is_box);
        assert!(
            !ray_hits(&solid, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)),
            "a rotated cutter still opens the middle"
        );
        let full = volume(&geometry(&session, wall));
        assert!(full < 4.0 - 0.2, "volume {full} should drop");

        let away = brush_at(&mut session, "box", Vec3::new(30.0, 1.0, 10.0));
        let before = geometry(&session, wall).positions.len();
        let missed = session.handle(&json!({
            "op": "mesh.boolean",
            "entity": wall,
            "tool": away,
            "operation": "subtract"
        }));
        assert_eq!(missed["ok"], false, "{missed}");
        assert_eq!(geometry(&session, wall).positions.len(), before);
        assert!(session.item_of(away).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_converted_box_supports_loop_cut_remains_solid_and_one_undo_restores_it() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_loop_cut_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "loop_cut" }),
        );
        let box_entity = brush_at(&mut session, "box", Vec3::new(0.0, 1.0, 0.0));
        call(
            &mut session,
            json!({ "op": "mesh.convert", "entity": box_entity }),
        );

        let before = geometry(&session, box_entity);
        assert_eq!(before.faces.len(), 6);
        assert!(petunia_mesh::is_solid(&before));

        let cut = call(
            &mut session,
            json!({
                "op": "mesh.loop_cut",
                "entity": box_entity,
                "edge": [0, 1],
                "cuts": 2,
                "slide": 0.0
            }),
        );
        assert_eq!(cut["cuts"], 2);
        let after = geometry(&session, box_entity);
        assert!(after.faces.len() > before.faces.len());
        assert!(
            petunia_mesh::is_solid(&after),
            "mesh remains solid after loop cut"
        );

        session.undo().expect("undo");
        let restored = geometry(&session, box_entity);
        assert_eq!(restored.faces.len(), 6);
        assert_eq!(restored.positions.len(), before.positions.len());

        let _ = std::fs::remove_dir_all(&root);
    }
}
