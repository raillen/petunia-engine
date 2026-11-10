//! Brush operations for building maps, adapted from Petunia3D's modeling tools to parametric
//! brushes:
//!
//! | Petunia3D | Here |
//! |---|---|
//! | Push/Pull (P) | drag a face: the brush grows or shrinks on that side, the opposite face stays |
//! | Extrude (E) | press on a face: a new brush grows out of it while you drag |
//! | Boolean difference | **carve**: the selected box is cut out of every box it overlaps |
//! | Solidify / shell | **hollow**: a box becomes floor, ceiling and four walls |
//!
//! Booleans on general meshes need the mesh crate (DR-22, level 3). On brushes they are exact
//! and cheap: a box minus a box is at most six boxes. Every operation is ONE `world.batch`, so a
//! single undo restores everything.

use petunia_commands::CommandRecord;
use petunia_math::{Color, Vec3};
use petunia_render::{OverlayItem, Shape};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo;
use crate::session::{EditorSession, display_name, vector_json};
use crate::tools::Pointer;

/// Thinnest a brush may get when pushing or pulling a face, in meters.
const MINIMUM_THICKNESS: f32 = 0.05;
/// Corner and edge handles of a selected box. Amber, so they stay distinct from collider squares.
const BOX_HANDLE_COLOR: Color = Color::new(0.93, 0.55, 0.12, 1.0);
const BOX_CORNER_PIXELS: f32 = 11.0;
const BOX_EDGE_PIXELS: f32 = 7.0;
const BOX_HANDLE_HIT_PIXELS: f32 = 12.0;

/// Most items one arrangement may create, counting the original.
const MAXIMUM_ARRANGE_COUNT: usize = 64;

/// Arrangements need at least the original and one copy, and a sane ceiling.
fn validate_arrangement(count: usize) -> Result<(), Failure> {
    if (2..=MAXIMUM_ARRANGE_COUNT).contains(&count) {
        return Ok(());
    }
    let message =
        format!("o arranjo aceita de 2 a {MAXIMUM_ARRANGE_COUNT} itens, contando o original");
    Err(Failure::new(
        "EDITOR_ARRANGE_COUNT",
        &message,
        "Ajuste o número de cópias.",
    ))
}

/// An axis-aligned box in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub low: Vec3,
    pub high: Vec3,
}

impl Aabb {
    pub fn size(&self) -> Vec3 {
        self.high - self.low
    }

    pub fn center(&self) -> Vec3 {
        (self.low + self.high) * 0.5
    }

    #[cfg(test)]
    pub fn volume(&self) -> f32 {
        let size = self.size();
        size.x.max(0.0) * size.y.max(0.0) * size.z.max(0.0)
    }

    fn overlaps(&self, other: &Aabb) -> bool {
        self.low.x < other.high.x
            && other.low.x < self.high.x
            && self.low.y < other.high.y
            && other.low.y < self.high.y
            && self.low.z < other.high.z
            && other.low.z < self.high.z
    }
}

/// `target` minus `cutter` as at most six boxes (below, above, then the four sides of the slab in
/// between). `None` when they do not overlap.
pub fn carve(target: Aabb, cutter: Aabb) -> Option<Vec<Aabb>> {
    if !target.overlaps(&cutter) {
        return None;
    }
    let mut pieces = Vec::new();
    let mut push = |low: Vec3, high: Vec3| {
        let piece = Aabb { low, high };
        if piece.size().x > 1.0e-4 && piece.size().y > 1.0e-4 && piece.size().z > 1.0e-4 {
            pieces.push(piece);
        }
    };
    let (t, c) = (target, cutter);
    let y_low = t.low.y.max(c.low.y);
    let y_high = t.high.y.min(c.high.y);
    push(t.low, Vec3::new(t.high.x, y_low, t.high.z));
    push(Vec3::new(t.low.x, y_high, t.low.z), t.high);
    let x_low = t.low.x.max(c.low.x);
    let x_high = t.high.x.min(c.high.x);
    push(
        Vec3::new(t.low.x, y_low, t.low.z),
        Vec3::new(x_low, y_high, t.high.z),
    );
    push(
        Vec3::new(x_high, y_low, t.low.z),
        Vec3::new(t.high.x, y_high, t.high.z),
    );
    push(
        Vec3::new(x_low, y_low, t.low.z),
        Vec3::new(x_high, y_high, t.low.z.max(c.low.z)),
    );
    push(
        Vec3::new(x_low, y_low, t.high.z.min(c.high.z)),
        Vec3::new(x_high, y_high, t.high.z),
    );
    Some(pieces)
}

/// The six walls of a hollow box of `size`, as (center, size) in the box's own units.
pub fn hollow(size: Vec3, thickness: Vec3) -> Vec<(&'static str, Vec3, Vec3)> {
    let half = size * 0.5;
    let t = Vec3::new(
        thickness.x.min(size.x / 3.0),
        thickness.y.min(size.y / 3.0),
        thickness.z.min(size.z / 3.0),
    );
    let inner_y = size.y - 2.0 * t.y;
    vec![
        (
            "Piso",
            Vec3::new(0.0, -half.y + t.y * 0.5, 0.0),
            Vec3::new(size.x, t.y, size.z),
        ),
        (
            "Teto",
            Vec3::new(0.0, half.y - t.y * 0.5, 0.0),
            Vec3::new(size.x, t.y, size.z),
        ),
        (
            "Parede",
            Vec3::new(-half.x + t.x * 0.5, 0.0, 0.0),
            Vec3::new(t.x, inner_y, size.z),
        ),
        (
            "Parede",
            Vec3::new(half.x - t.x * 0.5, 0.0, 0.0),
            Vec3::new(t.x, inner_y, size.z),
        ),
        (
            "Parede",
            Vec3::new(0.0, 0.0, -half.z + t.z * 0.5),
            Vec3::new(size.x - 2.0 * t.x, inner_y, t.z),
        ),
        (
            "Parede",
            Vec3::new(0.0, 0.0, half.z - t.z * 0.5),
            Vec3::new(size.x - 2.0 * t.x, inner_y, t.z),
        ),
    ]
}

/// A face of a brush under the mouse: its local axis and side, and where it is in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceHit {
    pub entity: u64,
    pub axis: usize,
    pub sign: f32,
    pub normal: Vec3,
    pub center: Vec3,
    pub corners: [Vec3; 4],
}

/// A push/pull (or extrude) drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceDrag {
    /// The face as it is shown now (it follows the mouse).
    pub face: FaceHit,
    /// The face where the drag started: the fixed reference of the math.
    start_face: FaceHit,
    start_size: Vec3,
    start_position: Vec3,
    scale_along: f32,
    start_parameter: f32,
    is_extrude: bool,
    extruded_entity: Option<u64>,
    previous_selection: Option<u64>,
    has_dragged: bool,
    grown: f32,
}

/// One corner or edge of a box. A sign of `0` is the axis the edge runs along and does not resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxHandle {
    pub entity: u64,
    pub signs: [i8; 3],
    pub center: Vec3,
}

/// A corner or edge drag. The opposite corner or edge stays put.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxDrag {
    entity: u64,
    signs: [i8; 3],
    start_size: Vec3,
    start_position: Vec3,
    rotation_degrees: Vec3,
    scale: Vec3,
    start_point: Vec3,
}

pub(crate) fn component(vector: Vec3, index: usize) -> f32 {
    match index {
        0 => vector.x,
        1 => vector.y,
        _ => vector.z,
    }
}

pub(crate) fn with_component(mut vector: Vec3, index: usize, value: f32) -> Vec3 {
    match index {
        0 => vector.x = value,
        1 => vector.y = value,
        _ => vector.z = value,
    }
    vector
}

pub(crate) fn unit(index: usize) -> Vec3 {
    with_component(Vec3::ZERO, index, 1.0)
}

fn brush_row(
    name: &str,
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
    size: Vec3,
    color: Json,
) -> Json {
    json!({
        "rows": {
            "Name": { "text": name },
            "Transform": {
                "position": vector_json(position),
                "rotation_degrees": vector_json(rotation),
                "scale": vector_json(scale),
            },
            "Brush": { "shape": "box", "size": vector_json(size), "detail": 16, "color": color },
        }
    })
}

impl EditorSession {
    fn spawn_preview_brush(
        &mut self,
        name: &str,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
        size: Vec3,
        color: Json,
    ) -> Result<u64, Failure> {
        let payload = brush_row(name, position, rotation, scale, size, color);
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let handle = project.world.spawn()?;
        let entity = project.world.persistent_id(handle)?.0;
        if let Some(rows) = payload.get("rows").and_then(Json::as_object) {
            for (table, values) in rows {
                if let Some(values_object) = values.as_object() {
                    project
                        .world
                        .insert_row_json(handle, table, values_object)
                        .map_err(|error| {
                            let _ = project.world.despawn(handle);
                            Failure::from(error)
                        })?;
                }
            }
        }
        Ok(entity)
    }

    fn despawn_preview_brush(&mut self, entity: u64) -> Result<(), Failure> {
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let handle = project
            .world
            .handle_of(petunia_world::PersistentId(entity))?;
        project.world.despawn(handle)?;
        Ok(())
    }

    fn update_preview_property(
        &mut self,
        entity: u64,
        table: &str,
        column: &str,
        value: &Json,
    ) -> Result<(), Failure> {
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let world = &mut project.world;
        let handle = world.handle_of(petunia_world::PersistentId(entity))?;
        let kind = world.column_kind(table, column)?;
        let parsed = petunia_world::Value::from_json(kind, value, world.limits().max_text_bytes)
            .map_err(|reason| petunia_world::WorldError::InvalidValue {
                column: column.to_string(),
                reason,
            })?;
        world.set(handle, table, column, parsed)?;
        Ok(())
    }

    pub(crate) fn box_parts(&self, entity: u64) -> Option<(Vec3, Vec3, Vec3, Vec3, Json)> {
        let item = self.item_of(entity)?;
        let world = &self.project.as_ref()?.world;
        let handle = world.handle_of(petunia_world::PersistentId(entity)).ok()?;
        let row = world.row(handle, "Brush").ok()?;
        let color = world
            .row_to_json("Brush", row)
            .ok()
            .and_then(|map| map.get("color").cloned())
            .unwrap_or(Json::Null);
        Some((
            item.transform.position,
            item.transform.rotation_degrees,
            item.transform.scale,
            item.size,
            color,
        ))
    }

    /// The face of a brush under the mouse (its bounding box face for round shapes).
    pub(crate) fn face_under(&self, pointer: Pointer) -> Option<FaceHit> {
        let (hit, _) = self.surface_under(pointer, &[])?;
        let item = self.item_of(hit.entity)?;
        let rotation = gizmo::rotation_from_euler(item.transform.rotation_degrees);
        let local = gizmo::inverse_rotate(&rotation, hit.normal);
        let axis = (0..3)
            .max_by(|a, b| {
                component(local, *a)
                    .abs()
                    .total_cmp(&component(local, *b).abs())
            })
            .unwrap_or(1);
        let sign = component(local, axis).signum();
        let half = Vec3::new(
            item.size.x * item.transform.scale.x * 0.5,
            item.size.y * item.transform.scale.y * 0.5,
            item.size.z * item.transform.scale.z * 0.5,
        );
        let normal = gizmo::rotate_vector(&rotation, unit(axis) * sign);
        let center = item.transform.position + normal * component(half, axis);
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let along_u = gizmo::rotate_vector(&rotation, unit(u) * component(half, u));
        let along_v = gizmo::rotate_vector(&rotation, unit(v) * component(half, v));
        Some(FaceHit {
            entity: hit.entity,
            axis,
            sign,
            normal,
            center,
            corners: [
                center - along_u - along_v,
                center + along_u - along_v,
                center + along_u + along_v,
                center - along_u + along_v,
            ],
        })
    }

    /// Press with Push/Pull or Extrude. Extrude first creates a thin preview box on the face,
    /// then the drag grows it. The extrusion is committed as a single command on completion.
    pub fn face_drag_begin(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let Some(mut face) = self.face_under(pointer) else {
            return Ok(json!({ "started": false }));
        };
        let is_extrude = self.tool == gizmo::Tool::Extrude;
        let previous_selection = self.selection;
        let mut extruded_entity = None;
        if is_extrude {
            let (position, rotation, scale, size, color) = self
                .box_parts(face.entity)
                .ok_or_else(|| Failure::bad_request("só brushes podem ser extrudados"))?;
            let depth = self.snap.translate.max(0.1);
            let scale_along = component(scale, face.axis).max(1.0e-4);
            let new_size = with_component(size, face.axis, depth / scale_along);
            let new_position = face.center + face.normal * (depth * 0.5);
            let name = format!(
                "{} (extrusão)",
                self.world()
                    .map(|world| display_name(world, face.entity))
                    .unwrap_or_default()
            );
            let entity =
                self.spawn_preview_brush(&name, new_position, rotation, scale, new_size, color)?;
            extruded_entity = Some(entity);
            self.selection = Some(entity);
            let _ = position;
            face = FaceHit {
                entity,
                center: face.center + face.normal * depth,
                corners: face.corners.map(|corner| corner + face.normal * depth),
                ..face
            };
        }
        let (position, _, scale, size, _) = match self.box_parts(face.entity) {
            Some(parts) => parts,
            None => {
                if let Some(entity) = extruded_entity {
                    let _ = self.despawn_preview_brush(entity);
                }
                return Err(Failure::bad_request(
                    "só brushes podem ser empurrados ou puxados",
                ));
            }
        };
        let view = self.view_for(pointer);
        let ray = match view.ray(pointer.mouse()) {
            Some(ray) => ray,
            None => {
                if let Some(entity) = extruded_entity {
                    let _ = self.despawn_preview_brush(entity);
                }
                return Err(Failure::bad_request("ponto inválido"));
            }
        };
        let start_parameter = closest_on_line(&ray, face.center, face.normal).unwrap_or(0.0);
        self.selection = Some(face.entity);
        self.face_drag = Some(FaceDrag {
            face,
            start_face: face,
            start_size: size,
            start_position: position,
            scale_along: component(scale, face.axis).max(1.0e-4),
            start_parameter,
            is_extrude,
            extruded_entity,
            previous_selection,
            has_dragged: false,
            grown: 0.0,
        });
        self.hud = Some(String::new());
        Ok(json!({ "started": true, "entity": face.entity }))
    }

    /// The face follows the mouse along its normal; the opposite face stays put.
    pub fn face_drag(&mut self, pointer: Pointer, invert_snap: bool) -> Result<Json, Failure> {
        let Some(drag) = self.face_drag else {
            return Ok(Json::Null);
        };
        let view = self.view_for(pointer);
        let Some(ray) = view.ray(pointer.mouse()) else {
            return Ok(Json::Null);
        };
        let face = drag.start_face;
        let Some(parameter) = closest_on_line(&ray, face.center, face.normal) else {
            return Ok(Json::Null);
        };
        let start_world = component(drag.start_size, face.axis) * drag.scale_along;
        let mut world_size = start_world + (parameter - drag.start_parameter);
        if self.snap.enabled != invert_snap && self.snap.translate > 0.0 {
            world_size = (world_size / self.snap.translate).round() * self.snap.translate;
        }
        let world_size = world_size.max(MINIMUM_THICKNESS);
        let grown = world_size - start_world;
        let size = with_component(drag.start_size, face.axis, world_size / drag.scale_along);
        let position = drag.start_position + face.normal * (grown * 0.5);
        if drag.is_extrude {
            self.update_preview_property(face.entity, "Brush", "size", &vector_json(size))?;
            self.update_preview_property(
                face.entity,
                "Transform",
                "position",
                &vector_json(position),
            )?;
        } else {
            self.gesture_preview(face.entity, "Brush", "size", &vector_json(size))?;
            self.gesture_preview(face.entity, "Transform", "position", &vector_json(position))?;
        }
        let shift = face.normal * grown;
        self.face_drag = Some(FaceDrag {
            face: FaceHit {
                center: face.center + shift,
                corners: face.corners.map(|corner| corner + shift),
                ..face
            },
            has_dragged: true,
            grown,
            ..drag
        });
        let summary = format!("{:+.2} m · espessura {:.2} m", grown, world_size).replace('.', ",");
        self.hud = Some(summary.clone());
        Ok(json!({ "summary": summary }))
    }

    pub fn face_drag_end(&mut self) -> Result<Json, Failure> {
        let drag = self.face_drag.take();
        self.hud = None;
        if let Some(drag) = drag.filter(|drag| drag.is_extrude) {
            let Some(extruded_entity) = drag.extruded_entity else {
                return Ok(Json::Null);
            };
            if !drag.has_dragged {
                self.despawn_preview_brush(extruded_entity)?;
                self.selection = drag.previous_selection;
                return Ok(Json::Null);
            }
            let name = self
                .world()
                .map(|world| display_name(world, extruded_entity))
                .unwrap_or_default();
            let (position, rotation, scale, size, color) = self
                .box_parts(extruded_entity)
                .ok_or_else(|| Failure::bad_request("a extrusão não tem brush"))?;
            self.despawn_preview_brush(extruded_entity)?;
            let mut payload = brush_row(&name, position, rotation, scale, size, color);
            if let Some(map) = payload.as_object_mut() {
                map.insert("persistent_id".to_string(), json!(extruded_entity));
            }
            let record = CommandRecord::new("world.create", payload);
            let outcome = self.execute(&record)?;
            self.selection = Some(extruded_entity);
            return Ok(outcome);
        }
        self.gesture_commit()
    }

    pub fn face_drag_cancel(&mut self) -> Result<Json, Failure> {
        let drag = self.face_drag.take();
        self.hud = None;
        if let Some(drag) = drag.filter(|drag| drag.is_extrude) {
            if let Some(extruded_entity) = drag.extruded_entity {
                self.despawn_preview_brush(extruded_entity)?;
            }
            self.selection = drag.previous_selection;
            return Ok(Json::Null);
        }
        self.gesture_cancel()
    }

    /// Corner and edge handles of the selected box. Face centers stay on Push/Pull (`P`).
    /// Cylinders, stairs and meshes have none. Hidden on tools that have no transform gizmo.
    pub(crate) fn box_handles(&self) -> Vec<BoxHandle> {
        if !self.tool.has_gizmo() {
            return Vec::new();
        }
        let Some(entity) = self.selection else {
            return Vec::new();
        };
        let Some(item) = self.item_of(entity) else {
            return Vec::new();
        };
        if item.shape != Shape::Box {
            return Vec::new();
        }
        let rotation = gizmo::rotation_from_euler(item.transform.rotation_degrees);
        let half = Vec3::new(
            item.size.x * item.transform.scale.x * 0.5,
            item.size.y * item.transform.scale.y * 0.5,
            item.size.z * item.transform.scale.z * 0.5,
        );
        let mut handles = Vec::with_capacity(20);
        for x in [-1_i8, 0, 1] {
            for y in [-1_i8, 0, 1] {
                for z in [-1_i8, 0, 1] {
                    let zeros = i8::from(x == 0) + i8::from(y == 0) + i8::from(z == 0);
                    // 0 zeros: corner. 1 zero: edge. 2 zeros: face center (Push/Pull). 3: the pivot.
                    if zeros != 0 && zeros != 1 {
                        continue;
                    }
                    let signs = [x, y, z];
                    let local = Vec3::new(
                        signs[0] as f32 * half.x,
                        signs[1] as f32 * half.y,
                        signs[2] as f32 * half.z,
                    );
                    handles.push(BoxHandle {
                        entity,
                        signs,
                        center: item.transform.position + gizmo::rotate_vector(&rotation, local),
                    });
                }
            }
        }
        handles
    }

    pub(crate) fn box_handle_under(&self, pointer: Pointer) -> Option<BoxHandle> {
        let view = self.view_for(pointer);
        if !view.is_valid() {
            return None;
        }
        let mouse = pointer.mouse();
        self.box_handles()
            .into_iter()
            .filter_map(|handle| {
                let screen = view.project(handle.center)?;
                let gap = ((screen[0] - mouse[0]).powi(2) + (screen[1] - mouse[1]).powi(2)).sqrt();
                (gap <= BOX_HANDLE_HIT_PIXELS).then_some((handle, gap))
            })
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(handle, _)| handle)
    }

    pub(crate) fn box_handle_overlay(&self, out: &mut Vec<OverlayItem>) {
        for handle in self.box_handles() {
            let corner = handle.signs.iter().all(|sign| *sign != 0);
            out.push(OverlayItem::Square {
                center: handle.center,
                color: BOX_HANDLE_COLOR,
                size_pixels: if corner {
                    BOX_CORNER_PIXELS
                } else {
                    BOX_EDGE_PIXELS
                },
            });
        }
    }

    /// `brush.handle_begin`: a corner or an edge under the pointer starts a resize.
    pub fn box_handle_begin(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let Some(handle) = self.box_handle_under(pointer) else {
            return Ok(json!({ "started": false }));
        };
        let Some((position, rotation, scale, size, _)) = self.box_parts(handle.entity) else {
            return Ok(json!({ "started": false }));
        };
        self.box_drag = Some(BoxDrag {
            entity: handle.entity,
            signs: handle.signs,
            start_size: size,
            start_position: position,
            rotation_degrees: rotation,
            scale,
            start_point: handle.center,
        });
        self.hud = Some(String::new());
        Ok(json!({ "started": true, "entity": handle.entity }))
    }

    /// `brush.handle_drag`: the grabbed corner or edge follows the pointer on the view plane.
    /// Axes marked `0` in the handle stay put, and so does the opposite corner or edge.
    pub fn box_handle_drag(
        &mut self,
        pointer: Pointer,
        invert_snap: bool,
    ) -> Result<Json, Failure> {
        let Some(drag) = self.box_drag else {
            return Ok(Json::Null);
        };
        let view = self.view_for(pointer);
        let Some(ray) = view.ray(pointer.mouse()) else {
            return Ok(Json::Null);
        };
        let Some(hit) = plane_hit(&ray, drag.start_point, view.camera.forward()) else {
            return Ok(Json::Null);
        };
        let rotation = gizmo::rotation_from_euler(drag.rotation_degrees);
        let local_delta = gizmo::inverse_rotate(&rotation, hit - drag.start_point);
        let snap = (self.snap.enabled != invert_snap && self.snap.translate > 0.0)
            .then_some(self.snap.translate);
        let (position, size) = resized_box(
            drag.start_position,
            drag.rotation_degrees,
            drag.scale,
            drag.start_size,
            drag.signs,
            local_delta,
            snap,
        );
        self.gesture_preview(drag.entity, "Brush", "size", &vector_json(size))?;
        self.gesture_preview(drag.entity, "Transform", "position", &vector_json(position))?;
        let summary = format!("{:.2} × {:.2} × {:.2} m", size.x, size.y, size.z).replace('.', ",");
        self.hud = Some(summary.clone());
        Ok(json!({ "summary": summary }))
    }

    /// `brush.handle_end`: one undo step for the size and the position together.
    pub fn box_handle_end(&mut self) -> Result<Json, Failure> {
        self.box_drag = None;
        self.hud = None;
        self.gesture_commit()
    }

    /// `brush.handle_cancel`: the box returns to the size and position from before the drag.
    pub fn box_handle_cancel(&mut self) -> Result<Json, Failure> {
        self.box_drag = None;
        self.hud = None;
        self.gesture_cancel()
    }

    /// Cuts the selected box out of every box it overlaps (boolean difference on brushes). The
    /// cutter stays, selected, so it can be moved and used again or deleted.
    pub fn carve_selection(&mut self) -> Result<Json, Failure> {
        let cutter_id = self.selection.ok_or_else(no_selection)?;
        let cutter = self.axis_aligned_box(cutter_id).ok_or_else(|| {
            Failure::new(
                "EDITOR_CARVE_NEEDS_BOX",
                "para cavar, a seleção precisa ser uma caixa sem rotação",
                "Use uma caixa (Criar > Caixa) e zere a rotação.",
            )
        })?;
        let mut records = Vec::new();
        let mut cut = 0;
        for item in self.draw_items() {
            if item.entity == cutter_id || item.shape != Shape::Box {
                continue;
            }
            let Some(target) = self.axis_aligned_box(item.entity) else {
                continue;
            };
            let Some(pieces) = carve(target, cutter) else {
                continue;
            };
            let Some((_, _, _, _, color)) = self.box_parts(item.entity) else {
                continue;
            };
            let name = self
                .world()
                .map(|w| display_name(w, item.entity))
                .unwrap_or_default();
            records.push(CommandRecord::new(
                "world.remove",
                json!({ "entity": item.entity }),
            ));
            for piece in pieces {
                records.push(CommandRecord::new(
                    "world.create",
                    brush_row(
                        &name,
                        piece.center(),
                        Vec3::ZERO,
                        Vec3::ONE,
                        piece.size(),
                        color.clone(),
                    ),
                ));
            }
            cut += 1;
        }
        if records.is_empty() {
            return Err(Failure::new(
                "EDITOR_NOTHING_TO_CARVE",
                "a caixa selecionada não encosta em nenhuma outra caixa sem rotação",
                "Coloque a caixa de corte sobre a parede ou o bloco que quer cavar.",
            ));
        }
        self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        Ok(json!({ "carved": cut }))
    }

    /// Turns the selected box into floor, ceiling and four walls of the snap thickness.
    pub fn hollow_selection(&mut self) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        let item = self
            .item_of(id)
            .filter(|item| item.shape == Shape::Box)
            .ok_or_else(|| {
                Failure::new(
                    "EDITOR_HOLLOW_NEEDS_BOX",
                    "só uma caixa pode ficar oca",
                    "Selecione uma caixa (sala, caverna, corredor).",
                )
            })?;
        let (position, rotation, scale, size, color) = self
            .box_parts(id)
            .ok_or_else(|| Failure::bad_request("a caixa não tem brush"))?;
        let thickness_world = self.snap.translate.clamp(0.05, 2.0);
        let thickness = Vec3::new(
            thickness_world / scale.x.max(1.0e-4),
            thickness_world / scale.y.max(1.0e-4),
            thickness_world / scale.z.max(1.0e-4),
        );
        let matrix = gizmo::rotation_from_euler(rotation);
        let mut records = vec![CommandRecord::new("world.remove", json!({ "entity": id }))];
        for (name, center, wall) in hollow(size, thickness) {
            let scaled = Vec3::new(center.x * scale.x, center.y * scale.y, center.z * scale.z);
            let world_center = position + gizmo::rotate_vector(&matrix, scaled);
            records.push(CommandRecord::new(
                "world.create",
                brush_row(name, world_center, rotation, scale, wall, color.clone()),
            ));
        }
        let _ = item;
        self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        self.selection = None;
        Ok(json!({ "walls": 6 }))
    }

    /// Splits the selected box in two with a plane perpendicular to one of its own axes, `offset`
    /// meters from its center. Both halves stay boxes: the cut only changes that axis of the size
    /// and the center along it, so a rotated box is cut correctly too.
    pub fn cut_selection(&mut self, axis: usize, offset: f32) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        self.item_of(id)
            .filter(|item| item.shape == Shape::Box)
            .ok_or_else(|| {
                Failure::new(
                    "EDITOR_CUT_NEEDS_BOX",
                    "para cortar com plano, a seleção precisa ser uma caixa",
                    "Use uma caixa (Criar > Caixa); cilindros, escadas e malhas ficam para depois.",
                )
            })?;
        let (position, rotation, scale, size, color) = self
            .box_parts(id)
            .ok_or_else(|| Failure::bad_request("a caixa não tem brush"))?;
        let along_scale = component(scale, axis);
        if along_scale <= 1.0e-4 {
            return Err(Failure::bad_request(
                "a caixa precisa ter escala positiva no eixo do corte",
            ));
        }
        let half_local = component(size, axis) * 0.5;
        let half_world = half_local * along_scale;
        if !offset.is_finite() || offset.abs() > half_world - MINIMUM_THICKNESS {
            return Err(Failure::new(
                "EDITOR_CUT_PLANE_OUTSIDE",
                "o plano de corte precisa ficar dentro da caixa, sobrando uma parte de cada lado",
                "Aproxime a distância do centro da caixa.",
            ));
        }
        // The plane in the box's own units, then the size and center of each half.
        let local_cut = offset / along_scale;
        let first_size = half_local + local_cut;
        let second_size = half_local - local_cut;
        let first_center = (local_cut - half_local) * 0.5;
        let second_center = (local_cut + half_local) * 0.5;
        let name = self
            .world()
            .map(|world| display_name(world, id))
            .unwrap_or_default();
        let matrix = gizmo::rotation_from_euler(rotation);
        let piece = |center_local: f32, size_axis: f32, suffix: &str| {
            let center =
                position + gizmo::rotate_vector(&matrix, unit(axis) * (center_local * along_scale));
            let mut piece_size = size;
            piece_size = with_component(piece_size, axis, size_axis);
            brush_row(
                &format!("{name} ({suffix})"),
                center,
                rotation,
                scale,
                piece_size,
                color.clone(),
            )
        };
        let records = vec![
            CommandRecord::new("world.remove", json!({ "entity": id })),
            CommandRecord::new("world.create", piece(first_center, first_size, "A")),
            CommandRecord::new("world.create", piece(second_center, second_size, "B")),
        ];
        let result = self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result.as_array().and_then(|results| {
            results
                .iter()
                .find_map(|result| result.get("entity").and_then(Json::as_u64))
        });
        self.selection = entity;
        Ok(json!({ "pieces": 2, "entity": entity }))
    }

    /// Copies the selected box along a straight line: `count` items counting the original, each
    /// copy `offset` meters past the previous one. One undo step.
    pub fn arrange_line_selection(&mut self, count: usize, offset: Vec3) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        validate_arrangement(count)?;
        if !(offset.x.is_finite() && offset.y.is_finite() && offset.z.is_finite())
            || offset.length() < MINIMUM_THICKNESS
        {
            return Err(Failure::new(
                "EDITOR_ARRANGE_OFFSET",
                "o deslocamento entre as cópias precisa ser diferente de zero",
                "Preencha o passo em metros (X, Y ou Z).",
            ));
        }
        let (position, rotation, scale, size, color) = self.arrange_parts(id)?;
        let name = self
            .world()
            .map(|world| display_name(world, id))
            .unwrap_or_default();
        let mut records = Vec::new();
        for index in 1..count {
            records.push(CommandRecord::new(
                "world.create",
                brush_row(
                    &format!("{name} ({})", index + 1),
                    position + offset * index as f32,
                    rotation,
                    scale,
                    size,
                    color.clone(),
                ),
            ));
        }
        let entity = self.execute_creates(records)?;
        self.selection = entity;
        Ok(json!({ "created": count - 1, "entity": entity }))
    }

    /// Copies the selected box around a circle: `count` items counting the original, evenly
    /// spaced around `center` in the plane perpendicular to `axis`. Each copy turns with the
    /// circle. One undo step.
    pub fn arrange_circle_selection(
        &mut self,
        count: usize,
        center: Vec3,
        axis: usize,
    ) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        validate_arrangement(count)?;
        if !(center.x.is_finite() && center.y.is_finite() && center.z.is_finite()) {
            return Err(Failure::bad_request(
                "o centro do círculo precisa ser finito",
            ));
        }
        let (position, rotation, scale, size, color) = self.arrange_parts(id)?;
        let axis_vector = unit(axis);
        let from_center = position - center;
        let along = axis_vector * from_center.dot(axis_vector);
        let radial = from_center - along;
        if radial.length() < MINIMUM_THICKNESS {
            return Err(Failure::new(
                "EDITOR_ARRANGE_RADIUS",
                "a caixa selecionada está sobre o eixo do círculo",
                "Afaste o centro do eixo ou escolha outro eixo.",
            ));
        }
        let name = self
            .world()
            .map(|world| display_name(world, id))
            .unwrap_or_default();
        let base = gizmo::rotation_from_euler(rotation);
        let mut records = Vec::new();
        for index in 1..count {
            let angle = std::f32::consts::TAU * index as f32 / count as f32;
            let step = gizmo::axis_rotation(axis_vector, angle);
            let moved = center + along + gizmo::rotate_vector(&step, radial);
            let turned = gizmo::euler_from_rotation(&gizmo::multiply(&step, &base));
            records.push(CommandRecord::new(
                "world.create",
                brush_row(
                    &format!("{name} ({})", index + 1),
                    moved,
                    turned,
                    scale,
                    size,
                    color.clone(),
                ),
            ));
        }
        let entity = self.execute_creates(records)?;
        self.selection = entity;
        Ok(json!({ "created": count - 1, "entity": entity }))
    }

    /// Mirrors the selected box across a world-axis plane (`axis` = constant `plane`). A copy is
    /// created by default; with `duplicate` off the original itself moves. One undo step.
    pub fn mirror_selection(
        &mut self,
        axis: usize,
        plane: f32,
        duplicate: bool,
    ) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        self.item_of(id)
            .filter(|item| item.shape == Shape::Box)
            .ok_or_else(|| {
                Failure::new(
                    "EDITOR_MIRROR_NEEDS_BOX",
                    "para espelhar, a seleção precisa ser uma caixa",
                    "Use uma caixa (Criar > Caixa).",
                )
            })?;
        let (position, rotation, scale, size, color) = self
            .box_parts(id)
            .ok_or_else(|| Failure::bad_request("a caixa não tem brush"))?;
        if !plane.is_finite() {
            return Err(Failure::bad_request(
                "o plano do espelho precisa ser finito",
            ));
        }
        let mut mirrored = position;
        match axis {
            0 => mirrored.x = 2.0 * plane - position.x,
            1 => mirrored.y = 2.0 * plane - position.y,
            _ => mirrored.z = 2.0 * plane - position.z,
        }
        // R' = M · R · M, where M is the reflection; the result is again a proper rotation.
        let mut reflection = [[0.0; 3]; 3];
        for (index, row) in reflection.iter_mut().enumerate() {
            if let Some(cell) = row.get_mut(index) {
                *cell = if index == axis { -1.0 } else { 1.0 };
            }
        }
        let matrix = gizmo::rotation_from_euler(rotation);
        let turned = gizmo::euler_from_rotation(&gizmo::multiply(
            &gizmo::multiply(&reflection, &matrix),
            &reflection,
        ));
        if !duplicate {
            let records = vec![
                CommandRecord::new(
                    "world.set",
                    json!({
                        "entity": id,
                        "table": "Transform",
                        "column": "position",
                        "value": vector_json(mirrored),
                    }),
                ),
                CommandRecord::new(
                    "world.set",
                    json!({
                        "entity": id,
                        "table": "Transform",
                        "column": "rotation_degrees",
                        "value": vector_json(turned),
                    }),
                ),
            ];
            self.execute(&CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            ))?;
            return Ok(json!({ "entity": id, "mirrored": 1 }));
        }
        let name = self
            .world()
            .map(|world| display_name(world, id))
            .unwrap_or_default();
        let mut records = vec![CommandRecord::new(
            "world.create",
            brush_row(
                &format!("{name} (espelho)"),
                mirrored,
                turned,
                scale,
                size,
                color,
            ),
        )];
        records.extend(self.copy_strokes_records(id, 0));
        records.extend(self.copy_collider_records(id, 0));
        records.extend(self.copy_script_records(id, 0));
        let result = self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result
            .get(0)
            .and_then(|first| first.get("entity"))
            .and_then(Json::as_u64);
        self.selection = entity;
        Ok(json!({ "entity": entity, "mirrored": 1 }))
    }

    /// Runs `world.create` records as one undo step and returns the last created entity.
    fn execute_creates(&mut self, records: Vec<CommandRecord>) -> Result<Option<u64>, Failure> {
        if records.is_empty() {
            return Ok(None);
        }
        let result = self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result
            .as_array()
            .and_then(|results| results.last())
            .and_then(|last| last.get("entity"))
            .and_then(Json::as_u64);
        Ok(entity)
    }

    /// The transform parts of the selected box, for the arrangement copies.
    fn arrange_parts(&self, id: u64) -> Result<(Vec3, Vec3, Vec3, Vec3, Json), Failure> {
        self.item_of(id)
            .filter(|item| item.shape == Shape::Box)
            .ok_or_else(|| {
                Failure::new(
                    "EDITOR_ARRANGE_NEEDS_BOX",
                    "o arranjo copia caixas; a seleção precisa ser uma caixa",
                    "Use uma caixa (Criar > Caixa).",
                )
            })?;
        self.box_parts(id)
            .ok_or_else(|| Failure::bad_request("a caixa não tem brush"))
    }

    /// World box of a brush that is a box with no rotation (where carving is exact).
    fn axis_aligned_box(&self, entity: u64) -> Option<Aabb> {
        let item = self.item_of(entity)?;
        let rotation = item.transform.rotation_degrees;
        let straight = |degrees: f32| (degrees / 90.0 - (degrees / 90.0).round()).abs() < 1.0e-3;
        if item.shape != Shape::Box
            || !(straight(rotation.x) && straight(rotation.y) && straight(rotation.z))
        {
            return None;
        }
        let (low, high) = crate::tools::item_bounds(&item)?;
        Some(Aabb { low, high })
    }

    /// `brush.paint_face`: one colour on one face of a box. A face without a row keeps the box colour.
    /// `face` is 0..5 (+X, -X, +Y, -Y, +Z, -Z). With a pointer, the face under the mouse is used.
    pub(crate) fn paint_face(
        &mut self,
        under: Option<Pointer>,
        request: &Json,
    ) -> Result<Json, Failure> {
        let (entity, face) = if let Some(pointer) = under {
            let hit = self.face_under(pointer).ok_or_else(|| {
                Failure::new(
                    "EDITOR_PAINT_NO_FACE",
                    "não há uma face de caixa sob o mouse",
                    "Aponte para uma face da caixa e use Operações > Pintar face… de novo.",
                )
            })?;
            let index = box_face_index(hit.axis, hit.sign)
                .ok_or_else(|| Failure::bad_request("essa face não é um lado de caixa"))?;
            (hit.entity, index)
        } else {
            let entity = match request.get("entity").and_then(Json::as_u64) {
                Some(entity) => entity,
                None => self.selection.ok_or_else(no_selection)?,
            };
            let face = request
                .get("face")
                .and_then(Json::as_u64)
                .ok_or_else(|| Failure::bad_request("`face` precisa ser um inteiro de 0 a 5"))?;
            if face > 5 {
                return Err(Failure::bad_request(
                    "`face` precisa ser um inteiro de 0 a 5",
                ));
            }
            (entity, face as usize)
        };
        let item = self
            .item_of(entity)
            .ok_or_else(|| Failure::bad_request("esse objeto não está na cena"))?;
        if item.shape != Shape::Box {
            return Err(Failure::new(
                "EDITOR_PAINT_NEEDS_BOX",
                "pintar face vale para caixas",
                "Cilindros, escadas e malhas gravadas ficam com a cor do objeto.",
            ));
        }
        let color = paint_color(request.get("color"))?;
        let existing = {
            let world = self.world()?;
            painted_face_row(world, entity, face)
        };
        let color_entity = if let Some(row) = existing {
            self.execute(&CommandRecord::new(
                "world.set",
                json!({
                    "entity": row,
                    "table": "FaceColor",
                    "column": "color",
                    "value": color,
                }),
            ))?;
            row
        } else {
            let owner = i64::try_from(entity)
                .map_err(|_| Failure::bad_request("o id do objeto não cabe numa linha de cor"))?;
            let created = self.execute(&CommandRecord::new(
                "world.create",
                json!({
                    "rows": {
                        "FaceColor": { "owner": owner, "face": face, "color": color }
                    }
                }),
            ))?;
            created
                .get("entity")
                .and_then(Json::as_u64)
                .ok_or_else(|| Failure::bad_request("a linha de cor não devolveu um id"))?
        };
        Ok(json!({ "entity": entity, "face": face, "color_entity": color_entity }))
    }
}

pub(crate) fn box_face_index(axis: usize, sign: f32) -> Option<usize> {
    if axis > 2 || !sign.is_finite() || sign == 0.0 {
        return None;
    }
    let side = if sign > 0.0 { 0 } else { 1 };
    Some(axis * 2 + side)
}

fn paint_color(color: Option<&Json>) -> Result<Json, Failure> {
    let Some(color) = color else {
        return Err(Failure::bad_request("falta `color` com r, g e b"));
    };
    let component = |name: &str| -> Result<f64, Failure> {
        let value = color
            .get(name)
            .and_then(Json::as_f64)
            .ok_or_else(|| Failure::bad_request("a cor precisa de r, g e b finitos"))?;
        if !value.is_finite() {
            return Err(Failure::bad_request("a cor precisa de r, g e b finitos"));
        }
        Ok(value)
    };
    let red = component("r")?;
    let green = component("g")?;
    let blue = component("b")?;
    let alpha = match color.get("a") {
        Some(value) => {
            let alpha = value
                .as_f64()
                .ok_or_else(|| Failure::bad_request("`a` precisa ser um número finito"))?;
            if !alpha.is_finite() {
                return Err(Failure::bad_request("`a` precisa ser um número finito"));
            }
            alpha
        }
        None => 1.0,
    };
    Ok(json!({ "r": red, "g": green, "b": blue, "a": alpha }))
}

fn painted_face_row(world: &petunia_world::World, owner: u64, face: usize) -> Option<u64> {
    let owner = i64::try_from(owner).ok()?;
    let face = face as i64;
    for handle in world.entities_with_table("FaceColor").unwrap_or_default() {
        let Ok(petunia_world::Value::Int(row_owner)) = world.get(handle, "FaceColor", "owner")
        else {
            continue;
        };
        let Ok(petunia_world::Value::Int(row_face)) = world.get(handle, "FaceColor", "face") else {
            continue;
        };
        if *row_owner == owner && *row_face == face {
            return world.persistent_id(handle).ok().map(|id| id.0);
        }
    }
    None
}

fn no_selection() -> Failure {
    Failure::new(
        "EDITOR_NO_SELECTION",
        "nenhuma entidade está selecionada",
        "Clique em um objeto no viewport ou escolha um item na Hierarquia.",
    )
}

/// Resizes `start_size` from one corner or edge. `signs` is `-1`, `0` or `1` per axis: `0` is not
/// resized. The opposite point stays fixed. `local_delta` is meters along the box axes.
pub(crate) fn resized_box(
    start_position: Vec3,
    rotation_degrees: Vec3,
    scale: Vec3,
    start_size: Vec3,
    signs: [i8; 3],
    local_delta: Vec3,
    snap: Option<f32>,
) -> (Vec3, Vec3) {
    let [sign_x, sign_y, sign_z] = signs;
    let (shift_x, size_x) = resize_axis(sign_x, start_size.x, scale.x, local_delta.x, snap);
    let (shift_y, size_y) = resize_axis(sign_y, start_size.y, scale.y, local_delta.y, snap);
    let (shift_z, size_z) = resize_axis(sign_z, start_size.z, scale.z, local_delta.z, snap);
    let rotation = gizmo::rotation_from_euler(rotation_degrees);
    (
        start_position + gizmo::rotate_vector(&rotation, Vec3::new(shift_x, shift_y, shift_z)),
        Vec3::new(size_x, size_y, size_z),
    )
}

/// One axis of [`resized_box`]: `(center shift in scaled-local meters, new size)`.
fn resize_axis(sign: i8, start_size: f32, scale: f32, delta: f32, snap: Option<f32>) -> (f32, f32) {
    if sign == 0 {
        return (0.0, start_size);
    }
    let moving_start = sign as f32 * start_size * scale * 0.5;
    let fixed = -moving_start;
    let desired = moving_start + delta;
    let direction = if moving_start.abs() > 1.0e-8 {
        moving_start.signum()
    } else {
        sign as f32
    };
    let mut extent = (desired - fixed) * direction;
    if extent < MINIMUM_THICKNESS {
        extent = MINIMUM_THICKNESS;
    }
    if let Some(step) = snap.filter(|step| *step > 0.0) {
        extent = (extent / step).round() * step;
        if extent < MINIMUM_THICKNESS {
            extent = step.max(MINIMUM_THICKNESS);
        }
    }
    let moving = fixed + direction * extent;
    let scale_abs = scale.abs().max(1.0e-4);
    ((fixed + moving) * 0.5, extent / scale_abs)
}

fn plane_hit(ray: &petunia_render::Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let facing = ray.direction.dot(normal);
    if facing.abs() < 1.0e-3 {
        return None;
    }
    let distance = (origin - ray.origin).dot(normal) / facing;
    (distance > 0.0).then(|| ray.origin + ray.direction * distance)
}

/// Parameter of the point on `origin + direction * t` closest to the mouse ray.
pub(crate) fn closest_on_line(
    ray: &petunia_render::Ray,
    origin: Vec3,
    direction: Vec3,
) -> Option<f32> {
    let offset = origin - ray.origin;
    let b = direction.dot(ray.direction);
    let denominator = 1.0 - b * b;
    if denominator.abs() < 1.0e-4 {
        return None;
    }
    Some((b * ray.direction.dot(offset) - direction.dot(offset)) / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(low: f32, high: f32) -> Aabb {
        Aabb {
            low: Vec3::splat(low),
            high: Vec3::splat(high),
        }
    }

    #[test]
    fn painting_a_box_face_draws_that_side_and_one_undo_restores_the_colour() {
        let (mut session, _root) = opened("paint_face");
        let entity = brush_at(&mut session, "box", Vec3::new(30.0, 1.0, 30.0));
        session.select(Some(entity)).expect("select");
        let painted = session.handle(&json!({
            "op": "brush.paint_face",
            "face": 0,
            "color": { "r": 0.8, "g": 0.1, "b": 0.1, "a": 1.0 }
        }));
        assert_eq!(painted["ok"], true, "{painted}");
        let shell = session
            .draw_items()
            .into_iter()
            .find(|item| item.entity == entity && item.shape == Shape::Mesh)
            .expect("painted face");
        assert!((shell.color.r - 0.8).abs() < 1.0e-4);
        let solid = petunia_render::solid_shapes(session.world().expect("world"));
        assert!(solid.get(&entity).is_some_and(|shape| shape.is_box));
        let again = session.handle(&json!({
            "op": "brush.paint_face",
            "face": 0,
            "color": { "r": 0.1, "g": 0.2, "b": 0.9, "a": 1.0 }
        }));
        assert_eq!(again["ok"], true, "{again}");
        let rows = session
            .world()
            .expect("world")
            .entities_with_table("FaceColor")
            .expect("table")
            .len();
        assert_eq!(rows, 1, "a second stroke replaces the colour");
        session.undo().expect("undo colour");
        let restored = session
            .draw_items()
            .into_iter()
            .find(|item| item.entity == entity && item.shape == Shape::Mesh)
            .expect("first colour");
        assert!((restored.color.r - 0.8).abs() < 1.0e-4);
        session.undo().expect("undo paint");
        assert!(
            session
                .draw_items()
                .iter()
                .all(|item| item.entity != entity || item.shape == Shape::Box)
        );
        let cylinder = brush_at(&mut session, "cylinder", Vec3::new(34.0, 1.0, 30.0));
        session.select(Some(cylinder)).expect("select cylinder");
        let refused = session.handle(&json!({
            "op": "brush.paint_face",
            "face": 0,
            "color": { "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0 }
        }));
        assert_eq!(refused["ok"], false, "{refused}");
    }

    #[test]
    fn carving_a_box_out_of_a_box_keeps_exactly_the_rest() {
        let target = cube(0.0, 4.0);
        let cutter = cube(1.0, 3.0);
        let pieces = carve(target, cutter).expect("overlap");
        assert_eq!(pieces.len(), 6);
        let volume: f32 = pieces.iter().map(Aabb::volume).sum();
        assert!((volume - (64.0 - 8.0)).abs() < 1.0e-3, "{volume}");
        // No piece overlaps the hole or another piece.
        for (index, piece) in pieces.iter().enumerate() {
            assert!(!piece.overlaps(&cutter));
            for other in pieces.iter().skip(index + 1) {
                assert!(!piece.overlaps(other));
            }
        }
        // A doorway cut through a wall leaves the two sides and the lintel.
        let wall = Aabb {
            low: Vec3::new(0.0, 0.0, 0.0),
            high: Vec3::new(6.0, 3.0, 0.3),
        };
        let door = Aabb {
            low: Vec3::new(2.5, -1.0, -1.0),
            high: Vec3::new(3.5, 2.2, 1.0),
        };
        let parts = carve(wall, door).expect("overlap");
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(
            carve(
                wall,
                Aabb {
                    low: Vec3::splat(10.0),
                    high: Vec3::splat(11.0)
                }
            )
            .is_none()
        );
    }

    #[test]
    fn a_hollow_box_has_six_walls_that_fill_only_the_shell() {
        let size = Vec3::new(6.0, 3.0, 4.0);
        let walls = hollow(size, Vec3::splat(0.25));
        assert_eq!(walls.len(), 6);
        let volume: f32 = walls.iter().map(|(_, _, s)| s.x * s.y * s.z).sum();
        let inner = (6.0 - 0.5) * (3.0 - 0.5) * (4.0 - 0.5);
        assert!(
            (volume - (6.0 * 3.0 * 4.0 - inner)).abs() < 1.0e-3,
            "{volume}"
        );
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        let reply = session.handle(&json!({
            "op": "project.new_sample",
            "path": root.display().to_string(),
            "name": label,
        }));
        assert_eq!(reply["ok"], true, "{reply}");
        (session, root)
    }

    fn look_down_at(session: &mut EditorSession, point: Vec3) {
        session.camera.camera.position = point + Vec3::new(0.0, 10.0, 0.0);
        session.camera.camera.yaw_degrees = 0.0;
        session.camera.camera.pitch_degrees = -89.9;
    }

    fn brush_at(session: &mut EditorSession, shape: &str, position: Vec3) -> u64 {
        let created = session.handle(&json!({ "op": "create.brush", "shape": shape }));
        assert_eq!(created["ok"], true, "{created}");
        let entity = created["result"]["entity"].as_u64().expect("brush");
        let moved = session.handle(&json!({
            "op": "command.execute",
            "command": "row.set",
            "payload": {
                "entity": entity,
                "table": "Transform",
                "values": {
                    "position": { "x": position.x, "y": position.y, "z": position.z }
                }
            }
        }));
        assert_eq!(moved["ok"], true, "{moved}");
        entity
    }

    #[test]
    fn extrude_no_op_click_leaves_no_leftover() {
        let (mut session, root) = opened("extrude_noop");
        let center = Vec3::new(40.0, 1.0, 40.0);
        let entity = brush_at(&mut session, "box", center);
        session.select(Some(entity)).expect("select");
        look_down_at(&mut session, center);
        session.set_tool(gizmo::Tool::Extrude);
        let pointer = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        let count_before = session.draw_items().len();
        let history_before = session.dispatcher.undo_depth().0;
        let begin = session.face_drag_begin(pointer).expect("begin");
        assert_eq!(begin["started"], true);
        let _end = session.face_drag_end().expect("end");
        let count_after = session.draw_items().len();
        assert_eq!(
            count_after, count_before,
            "no leftover entity on no-op click"
        );
        assert_eq!(
            session.dispatcher.undo_depth().0,
            history_before,
            "no command entered undo history on no-op click"
        );
        assert_eq!(session.selection, Some(entity), "selection preserved");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn extrude_escape_cancel_restores_world_and_selection() {
        let (mut session, root) = opened("extrude_cancel");
        let center = Vec3::new(40.0, 1.0, 40.0);
        let entity = brush_at(&mut session, "box", center);
        session.select(Some(entity)).expect("select");
        look_down_at(&mut session, center);
        session.set_tool(gizmo::Tool::Extrude);
        let pointer_start = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        let pointer_move = Pointer {
            x: 400.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };
        let count_before = session.draw_items().len();
        let history_before = session.dispatcher.undo_depth().0;

        let begin = session.face_drag_begin(pointer_start).expect("begin");
        assert_eq!(begin["started"], true);
        let _drag = session.face_drag(pointer_move, false).expect("drag");
        let _cancel = session.face_drag_cancel().expect("cancel");

        let count_after = session.draw_items().len();
        assert_eq!(count_after, count_before, "cancel removes extruded entity");
        assert_eq!(
            session.dispatcher.undo_depth().0,
            history_before,
            "cancel leaves no undo entry"
        );
        assert_eq!(session.selection, Some(entity), "cancel restores selection");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn extrude_drag_and_commit_is_single_undo_step() {
        let (mut session, root) = opened("extrude_undo");
        let center = Vec3::new(40.0, 1.0, 40.0);
        let entity = brush_at(&mut session, "box", center);
        session.select(Some(entity)).expect("select");
        look_down_at(&mut session, center);
        session.set_tool(gizmo::Tool::Extrude);
        let pointer_start = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        let pointer_move = Pointer {
            x: 400.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };
        let count_before = session.draw_items().len();
        let history_before = session.dispatcher.undo_depth().0;

        let begin = session.face_drag_begin(pointer_start).expect("begin");
        assert_eq!(begin["started"], true);
        let _drag = session.face_drag(pointer_move, false).expect("drag");
        let end = session.face_drag_end().expect("end");
        let extruded_entity = end["entity"].as_u64().expect("extruded entity");

        assert_eq!(session.draw_items().len(), count_before + 1);
        assert_eq!(
            session.dispatcher.undo_depth().0,
            history_before + 1,
            "exactly one undo entry for entire extrusion"
        );
        assert_eq!(session.selection, Some(extruded_entity));

        // ONE undo must completely remove the extruded entity:
        let undone = session.undo().expect("undo");
        assert_eq!(undone["command"], "world.create");
        assert_eq!(
            session.draw_items().len(),
            count_before,
            "single undo removes extruded brush"
        );

        // ONE redo restores it:
        let redone = session.redo().expect("redo");
        assert_eq!(redone["command"], "world.create");
        assert_eq!(session.draw_items().len(), count_before + 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    fn set_rotation(session: &mut EditorSession, entity: u64, rotation: Vec3) {
        let rotated = session.handle(&json!({
            "op": "command.execute",
            "command": "row.set",
            "payload": {
                "entity": entity,
                "table": "Transform",
                "values": {
                    "rotation_degrees": { "x": rotation.x, "y": rotation.y, "z": rotation.z }
                }
            }
        }));
        assert_eq!(rotated["ok"], true, "{rotated}");
    }

    fn box_volume(items: &[petunia_render::DrawItem]) -> f32 {
        items
            .iter()
            .filter(|item| item.shape == Shape::Box)
            .map(|item| {
                item.size.x
                    * item.size.y
                    * item.size.z
                    * item.transform.scale.x
                    * item.transform.scale.y
                    * item.transform.scale.z
            })
            .sum()
    }

    #[test]
    fn cut_plane_splits_a_rotated_box_in_two_without_losing_volume() {
        let (mut session, root) = opened("cut_plane");
        let position = Vec3::new(40.0, 1.0, 40.0);
        let entity = brush_at(&mut session, "box", position);
        set_rotation(&mut session, entity, Vec3::new(0.0, 35.0, 0.0));
        session.select(Some(entity)).expect("select");
        let item = session.item_of(entity).expect("item");
        let before = box_volume(&session.draw_items());

        let cut = session.handle(&json!({
            "op": "brush.cut_plane",
            "axis": "x",
            "offset": 0.0,
        }));
        assert_eq!(cut["ok"], true, "{cut}");
        assert_eq!(cut["result"]["pieces"], 2);
        let after = box_volume(&session.draw_items());
        assert!(
            (after - before).abs() < 0.01,
            "volume {after} should match {before}"
        );
        let first = cut["result"]["entity"].as_u64().expect("first piece");
        assert_eq!(session.selection, Some(first));
        let piece = session.item_of(first).expect("piece");
        // Half the size on the cut axis, and the same rotation as the original.
        assert!(
            (piece.size.x - item.size.x * 0.5).abs() < 1.0e-3,
            "{}",
            piece.size.x
        );
        assert!((piece.transform.rotation_degrees.y - 35.0).abs() < 1.0e-3);
        // One undo restores the single box.
        session.undo().expect("undo");
        assert!(session.item_of(entity).is_some(), "original box is back");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cut_plane_outside_the_box_is_refused() {
        let (mut session, root) = opened("cut_plane_outside");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        session.select(Some(entity)).expect("select");
        let cut = session.handle(&json!({
            "op": "brush.cut_plane",
            "axis": "x",
            "offset": 100.0,
        }));
        assert_eq!(cut["ok"], false);
        assert_eq!(cut["error"]["code"], "EDITOR_CUT_PLANE_OUTSIDE");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn arrange_line_copies_the_box_and_undoes_in_one_step() {
        let (mut session, root) = opened("arrange_line");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        session.select(Some(entity)).expect("select");
        let before = session.draw_items().len();
        let arranged = session.handle(&json!({
            "op": "brush.arrange_line",
            "count": 4,
            "offset": { "x": 2.0, "y": 0.0, "z": 0.0 },
        }));
        assert_eq!(arranged["ok"], true, "{arranged}");
        assert_eq!(arranged["result"]["created"], 3);
        assert_eq!(session.draw_items().len(), before + 3);
        let copy = arranged["result"]["entity"].as_u64().expect("copy");
        let position = session.item_of(copy).expect("copy item").transform.position;
        assert!((position.x - 46.0).abs() < 1.0e-3, "{position:?}");
        session.undo().expect("undo");
        assert_eq!(session.draw_items().len(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn arrange_circle_places_and_turns_the_copies_around_the_axis() {
        let (mut session, root) = opened("arrange_circle");
        let center = Vec3::new(200.0, 0.0, 200.0);
        let entity = brush_at(&mut session, "box", Vec3::new(205.0, 0.0, 200.0));
        set_rotation(&mut session, entity, Vec3::new(0.0, 45.0, 0.0));
        session.select(Some(entity)).expect("select");
        let arranged = session.handle(&json!({
            "op": "brush.arrange_circle",
            "count": 4,
            "center": { "x": center.x, "y": center.y, "z": center.z },
            "axis": "y",
        }));
        assert_eq!(arranged["ok"], true, "{arranged}");
        assert_eq!(arranged["result"]["created"], 3);
        let ring: Vec<petunia_render::DrawItem> = session
            .draw_items()
            .into_iter()
            .filter(|item| {
                let offset = item.transform.position - center;
                (offset.length() - 5.0).abs() < 1.0e-2
            })
            .collect();
        assert_eq!(ring.len(), 4, "{ring:?}");
        let turned = ring
            .iter()
            .map(|item| item.transform.rotation_degrees.y.round() as i32)
            .collect::<Vec<_>>();
        let mut sorted = turned;
        sorted.sort_unstable();
        // 45° is the source; the copies turn with the circle (their Euler yaw is normalized
        // to (-180°, 180°], so 225° reads as -135° and 315° as -45°).
        assert_eq!(sorted, vec![-135, -45, 45, 135]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mirror_duplicates_a_rotated_box_and_mirrors_its_turn() {
        let (mut session, root) = opened("mirror_duplicate");
        let entity = brush_at(&mut session, "box", Vec3::new(205.0, 1.0, 200.0));
        set_rotation(&mut session, entity, Vec3::new(0.0, 30.0, 0.0));
        session.select(Some(entity)).expect("select");
        let before = session.draw_items().len();
        let mirrored = session.handle(&json!({
            "op": "brush.mirror",
            "axis": "x",
            "plane": 200.0,
            "duplicate": true,
        }));
        assert_eq!(mirrored["ok"], true, "{mirrored}");
        assert_eq!(session.draw_items().len(), before + 1);
        let copy = mirrored["result"]["entity"].as_u64().expect("copy");
        let copy_item = session.item_of(copy).expect("copy item");
        assert!((copy_item.transform.position.x - 195.0).abs() < 1.0e-3);
        assert!((copy_item.transform.rotation_degrees.y + 30.0).abs() < 1.0e-3);
        // The original stays where it was.
        let original = session.item_of(entity).expect("original");
        assert!((original.transform.position.x - 205.0).abs() < 1.0e-3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mirror_in_place_moves_the_box_and_refuses_other_shapes() {
        let (mut session, root) = opened("mirror_in_place");
        let entity = brush_at(&mut session, "box", Vec3::new(205.0, 1.0, 200.0));
        session.select(Some(entity)).expect("select");
        let before = session.draw_items().len();
        let mirrored = session.handle(&json!({
            "op": "brush.mirror",
            "axis": "z",
            "plane": 210.0,
            "duplicate": false,
        }));
        assert_eq!(mirrored["ok"], true, "{mirrored}");
        assert_eq!(session.draw_items().len(), before);
        let moved = session.item_of(entity).expect("moved");
        assert!((moved.transform.position.z - 220.0).abs() < 1.0e-3);
        let cylinder = brush_at(&mut session, "cylinder", Vec3::new(40.0, 1.0, 40.0));
        session.select(Some(cylinder)).expect("select");
        let refused = session.handle(&json!({
            "op": "brush.mirror",
            "axis": "x",
            "plane": 0.0,
        }));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["error"]["code"], "EDITOR_MIRROR_NEEDS_BOX");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resized_box_grows_from_the_opposite_side() {
        let start = Vec3::new(2.0, 2.0, 2.0);
        let (position, size) = resized_box(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ONE,
            start,
            [1, 1, 1],
            Vec3::new(1.0, 0.0, 0.0),
            None,
        );
        assert!((size.x - 3.0).abs() < 1.0e-4, "{size:?}");
        assert!((size.y - 2.0).abs() < 1.0e-4 && (size.z - 2.0).abs() < 1.0e-4);
        assert!((position.x - 0.5).abs() < 1.0e-4, "{position:?}");
        assert!(position.y.abs() < 1.0e-4 && position.z.abs() < 1.0e-4);

        let (_, edge) = resized_box(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ONE,
            start,
            [1, 0, 1],
            Vec3::new(1.0, 5.0, 0.0),
            None,
        );
        assert!((edge.x - 3.0).abs() < 1.0e-4, "{edge:?}");
        assert!(
            (edge.y - 2.0).abs() < 1.0e-4,
            "the axis the edge runs along stays"
        );
        assert!((edge.z - 2.0).abs() < 1.0e-4);

        let (turned, _) = resized_box(
            Vec3::ZERO,
            Vec3::new(0.0, 90.0, 0.0),
            Vec3::ONE,
            start,
            [1, 1, 1],
            Vec3::new(1.0, 0.0, 0.0),
            None,
        );
        let rotation = gizmo::rotation_from_euler(Vec3::new(0.0, 90.0, 0.0));
        let expected = gizmo::rotate_vector(&rotation, Vec3::new(0.5, 0.0, 0.0));
        assert!(
            (turned - expected).length() < 1.0e-4,
            "{turned:?} {expected:?}"
        );
    }

    fn screen_of(session: &EditorSession, point: Vec3) -> [f32; 2] {
        let view = gizmo::View {
            camera: session.camera.camera,
            width: 1200.0,
            height: 800.0,
        };
        view.project(point).expect("handle on screen")
    }

    #[test]
    fn a_box_corner_resizes_and_undoes_in_one_step() {
        let (mut session, root) = opened("box_handles");
        let cylinder = brush_at(&mut session, "cylinder", Vec3::new(10.0, 1.0, 10.0));
        session.select(Some(cylinder)).expect("select");
        assert!(
            session.box_handles().is_empty(),
            "only boxes have corner and edge handles"
        );

        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        session.select(Some(entity)).expect("select");
        session.snap.enabled = false;
        let looked = session.handle(&json!({ "op": "camera.view", "name": "front" }));
        assert_eq!(looked["ok"], true, "{looked}");
        let handles = session.box_handles();
        assert_eq!(handles.len(), 20, "8 corners and 12 edges");
        let mut squares = Vec::new();
        session.box_handle_overlay(&mut squares);
        assert_eq!(squares.len(), 20);
        let corner = handles
            .iter()
            .find(|handle| handle.signs == [1, 1, 1])
            .copied()
            .expect("+X +Y +Z corner");
        let before = session.item_of(entity).expect("box");
        let start = screen_of(&session, corner.center);
        let begun = session.handle(&json!({
            "op": "brush.handle_begin",
            "x": start[0], "y": start[1], "width": 1200.0, "height": 800.0,
        }));
        assert_eq!(begun["ok"], true, "{begun}");
        assert_eq!(begun["result"]["started"], true);
        let target = screen_of(&session, corner.center + Vec3::new(1.0, 0.0, 0.0));
        let dragged = session.handle(&json!({
            "op": "brush.handle_drag",
            "x": target[0], "y": target[1], "width": 1200.0, "height": 800.0,
        }));
        assert_eq!(dragged["ok"], true, "{dragged}");
        let ended = session.handle(&json!({ "op": "brush.handle_end" }));
        assert_eq!(ended["ok"], true, "{ended}");
        let after = session.item_of(entity).expect("resized");
        assert!(
            (after.size.x - (before.size.x + 1.0)).abs() < 1.0e-3,
            "size {} -> {}",
            before.size.x,
            after.size.x
        );
        assert!((after.size.y - before.size.y).abs() < 1.0e-3);
        assert!((after.size.z - before.size.z).abs() < 1.0e-3);
        assert!(
            (after.transform.position.x - (before.transform.position.x + 0.5)).abs() < 1.0e-3,
            "the opposite side stays: {:?}",
            after.transform.position
        );
        session.undo().expect("undo");
        let restored = session.item_of(entity).expect("restored");
        assert!((restored.size.x - before.size.x).abs() < 1.0e-3);
        assert!((restored.transform.position.x - before.transform.position.x).abs() < 1.0e-3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cancelling_a_box_handle_restores_the_box() {
        let (mut session, root) = opened("box_handle_cancel");
        let entity = brush_at(&mut session, "box", Vec3::new(40.0, 1.0, 40.0));
        session.select(Some(entity)).expect("select");
        session.snap.enabled = false;
        let _ = session.handle(&json!({ "op": "camera.view", "name": "front" }));
        let corner = session
            .box_handles()
            .into_iter()
            .find(|handle| handle.signs == [1, 0, 1])
            .expect("edge");
        let before = session.item_of(entity).expect("box");
        let depth = session.dispatcher.undo_depth().0;
        let start = screen_of(&session, corner.center);
        let begun = session.box_handle_begin(Pointer {
            x: start[0],
            y: start[1],
            width: 1200.0,
            height: 800.0,
        });
        assert_eq!(begun.expect("begin")["started"], true);
        let target = screen_of(&session, corner.center + Vec3::new(1.0, 0.0, 0.0));
        session
            .box_handle_drag(
                Pointer {
                    x: target[0],
                    y: target[1],
                    width: 1200.0,
                    height: 800.0,
                },
                false,
            )
            .expect("drag");
        session.box_handle_cancel().expect("cancel");
        let after = session.item_of(entity).expect("box");
        assert!((after.size.x - before.size.x).abs() < 1.0e-3);
        assert!((after.transform.position.x - before.transform.position.x).abs() < 1.0e-3);
        assert_eq!(session.dispatcher.undo_depth().0, depth);
        let _ = std::fs::remove_dir_all(&root);
    }
}
