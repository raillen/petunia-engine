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
use petunia_math::Vec3;
use petunia_render::Shape;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo;
use crate::session::{EditorSession, display_name, vector_json};
use crate::tools::Pointer;

/// Thinnest a brush may get when pushing or pulling a face, in meters.
const MINIMUM_THICKNESS: f32 = 0.05;

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
}

fn component(vector: Vec3, index: usize) -> f32 {
    match index {
        0 => vector.x,
        1 => vector.y,
        _ => vector.z,
    }
}

fn with_component(mut vector: Vec3, index: usize, value: f32) -> Vec3 {
    match index {
        0 => vector.x = value,
        1 => vector.y = value,
        _ => vector.z = value,
    }
    vector
}

fn unit(index: usize) -> Vec3 {
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
    fn box_parts(&self, entity: u64) -> Option<(Vec3, Vec3, Vec3, Vec3, Json)> {
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

    /// Press with Push/Pull or Extrude. Extrude first creates a thin box on the face (one undo
    /// step), then the drag grows it.
    pub fn face_drag_begin(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let Some(mut face) = self.face_under(pointer) else {
            return Ok(json!({ "started": false }));
        };
        if self.tool == gizmo::Tool::Extrude {
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
                    .map(|w| display_name(w, face.entity))
                    .unwrap_or_default()
            );
            let record = CommandRecord::new(
                "world.create",
                brush_row(&name, new_position, rotation, scale, new_size, color),
            );
            let created = self.execute(&record)?;
            let entity = created
                .get("entity")
                .and_then(Json::as_u64)
                .ok_or_else(|| Failure::bad_request("a extrusão não criou a entidade"))?;
            self.selection = Some(entity);
            let _ = position;
            face = FaceHit {
                entity,
                center: face.center + face.normal * depth,
                corners: face.corners.map(|corner| corner + face.normal * depth),
                ..face
            };
        }
        let (position, _, scale, size, _) = self
            .box_parts(face.entity)
            .ok_or_else(|| Failure::bad_request("só brushes podem ser empurrados ou puxados"))?;
        let view = self.view_for(pointer);
        let ray = view
            .ray(pointer.mouse())
            .ok_or_else(|| Failure::bad_request("ponto inválido"))?;
        let start_parameter = closest_on_line(&ray, face.center, face.normal).unwrap_or(0.0);
        self.selection = Some(face.entity);
        self.face_drag = Some(FaceDrag {
            face,
            start_face: face,
            start_size: size,
            start_position: position,
            scale_along: component(scale, face.axis).max(1.0e-4),
            start_parameter,
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
        self.gesture_preview(face.entity, "Brush", "size", &vector_json(size))?;
        self.gesture_preview(face.entity, "Transform", "position", &vector_json(position))?;
        let shift = face.normal * grown;
        self.face_drag = Some(FaceDrag {
            face: FaceHit {
                center: face.center + shift,
                corners: face.corners.map(|corner| corner + shift),
                ..face
            },
            ..drag
        });
        let summary = format!("{:+.2} m · espessura {:.2} m", grown, world_size).replace('.', ",");
        self.hud = Some(summary.clone());
        Ok(json!({ "summary": summary }))
    }

    pub fn face_drag_end(&mut self) -> Result<Json, Failure> {
        self.face_drag = None;
        self.hud = None;
        self.gesture_commit()
    }

    pub fn face_drag_cancel(&mut self) -> Result<Json, Failure> {
        self.face_drag = None;
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
}

fn no_selection() -> Failure {
    Failure::new(
        "EDITOR_NO_SELECTION",
        "nenhuma entidade está selecionada",
        "Clique em um objeto no viewport ou escolha um item na Hierarquia.",
    )
}

/// Parameter of the point on `origin + direction * t` closest to the mouse ray.
fn closest_on_line(ray: &petunia_render::Ray, origin: Vec3, direction: Vec3) -> Option<f32> {
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
}
