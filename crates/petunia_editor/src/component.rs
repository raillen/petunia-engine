//! Face, edge and vertex picks on the selected object (DR-53).
//!
//! There is no edit mode. Move, rotate and scale keep working; a click on the selected object
//! chooses a component instead of another object. The choice lives in the session, not in the file.

use petunia_math::{Color, PolygonMesh, Transform, Vec3};
use petunia_render::{DrawItem, OverlayItem, Shape};
use petunia_world::Value;
use serde_json::{Value as Json, json};

use crate::brush_ops::box_face_index;
use crate::failure::Failure;
use crate::gizmo::{
    Drag, DragResult, Handle, Pose, inverse_rotate, rotate_vector, rotation_from_euler,
};
use crate::session::EditorSession;
use crate::tools::Pointer;

const COMPONENT_COLOR: Color = Color::new(0.25, 0.82, 0.95, 1.0);

/// Corner index: bit 0 is +X, bit 1 is +Y, bit 2 is +Z.
const BOX_FACES: [[usize; 4]; 6] = [
    [1, 3, 7, 5],
    [0, 2, 6, 4],
    [2, 3, 7, 6],
    [0, 1, 5, 4],
    [4, 5, 7, 6],
    [0, 1, 3, 2],
];

const FACE_NAMES: [&str; 6] = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComponentKind {
    Face,
    Edge,
    Vertex,
}

impl ComponentKind {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "face" => Some(Self::Face),
            "edge" => Some(Self::Edge),
            "vertex" => Some(Self::Vertex),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Face => "face",
            Self::Edge => "edge",
            Self::Vertex => "vertex",
        }
    }

    fn plural(self) -> &'static str {
        match self {
            Self::Face => "faces",
            Self::Edge => "arestas",
            Self::Vertex => "vértices",
        }
    }
}

impl EditorSession {
    pub(crate) fn component_json(&self) -> Json {
        json!({
            "kind": self.component_kind.name(),
            "count": self.components.len(),
            "label": self.component_label(),
        })
    }

    pub(crate) fn component_set_kind(
        &mut self,
        name: &str,
        under: Option<Pointer>,
    ) -> Result<Json, Failure> {
        let kind = ComponentKind::parse(name)
            .ok_or_else(|| Failure::bad_request("`kind` é `face`, `edge` ou `vertex`"))?;
        self.component_kind = kind;
        self.components.clear();
        if let Some(pointer) = under {
            return self.component_pick(pointer, false);
        }
        Ok(self.component_json())
    }

    /// Picks the component under the pointer when it belongs to the selection.
    /// `picked: false` means the caller should select an object instead.
    pub(crate) fn component_pick(
        &mut self,
        pointer: Pointer,
        additive: bool,
    ) -> Result<Json, Failure> {
        let Some(selected) = self.selection else {
            return Ok(json!({ "picked": false }));
        };
        let Some((hit, point)) = self.surface_under(pointer, &[]) else {
            return Ok(json!({ "picked": false }));
        };
        if hit.entity != selected {
            return Ok(json!({ "picked": false, "entity": hit.entity }));
        }
        let Some(index) = self.component_index(selected, point) else {
            return Ok(json!({ "picked": false }));
        };
        if additive {
            if let Some(place) = self.components.iter().position(|item| *item == index) {
                self.components.remove(place);
            } else {
                self.components.push(index);
            }
        } else {
            self.components = vec![index];
        }
        let mut result = self.component_json();
        if let Some(object) = result.as_object_mut() {
            object.insert("picked".to_string(), Json::Bool(true));
        }
        Ok(result)
    }

    pub(crate) fn component_overlay(&self, out: &mut Vec<OverlayItem>) {
        let Some(entity) = self.selection else {
            return;
        };
        let Some(item) = self.item_of(entity) else {
            return;
        };
        for index in &self.components {
            match self.component_kind {
                ComponentKind::Face => {
                    for (from, to) in face_edges(
                        &item,
                        self.mesh_of(entity).as_ref().map(|(mesh, _)| mesh),
                        *index,
                    ) {
                        out.push(OverlayItem::Line {
                            from,
                            to,
                            color: COMPONENT_COLOR,
                            width_pixels: 3.0,
                        });
                    }
                }
                ComponentKind::Edge => {
                    if let Some((from, to)) = edge_segment(
                        &item,
                        self.mesh_of(entity).as_ref().map(|(mesh, _)| mesh),
                        *index,
                    ) {
                        out.push(OverlayItem::Line {
                            from,
                            to,
                            color: COMPONENT_COLOR,
                            width_pixels: 4.0,
                        });
                    }
                }
                ComponentKind::Vertex => {
                    if let Some(center) = vertex_point(
                        &item,
                        self.mesh_of(entity).as_ref().map(|(mesh, _)| mesh),
                        *index,
                    ) {
                        out.push(OverlayItem::Square {
                            center,
                            color: COMPONENT_COLOR,
                            size_pixels: 9.0,
                        });
                    }
                }
            }
        }
    }

    fn component_label(&self) -> String {
        let count = self.components.len();
        if count == 0 {
            return String::new();
        }
        if count > 1 {
            return format!("{count} {}", self.component_kind.plural());
        }
        let index = self.components.first().copied().unwrap_or(0);
        match self.component_kind {
            ComponentKind::Face => FACE_NAMES
                .get(index as usize)
                .map(|name| format!("face {name}"))
                .unwrap_or_else(|| format!("face {index}")),
            ComponentKind::Edge => format!("aresta {index}"),
            ComponentKind::Vertex => format!("vértice {index}"),
        }
    }

    fn component_index(&self, entity: u64, point: Vec3) -> Option<u32> {
        let item = self.item_of(entity)?;
        if item.shape == Shape::Box {
            return box_component(&item, point, self.component_kind);
        }
        let (mesh, transform) = self.mesh_of(entity)?;
        let local = to_local(&transform, point)?;
        mesh_component(&mesh, local, self.component_kind)
    }

    pub(crate) fn component_pivot(&self, entity: u64) -> Option<Vec3> {
        if self.components.is_empty() {
            return None;
        }
        let item = self.item_of(entity)?;
        let mesh = self.mesh_of(entity);
        let mesh_ref = mesh.as_ref().map(|(m, _)| m);
        let mut total = Vec3::ZERO;
        let mut count = 0usize;
        for &index in &self.components {
            match self.component_kind {
                ComponentKind::Face => {
                    let edges = face_edges(&item, mesh_ref, index);
                    for (from, to) in edges {
                        total = total + (from + to) * 0.5;
                        count += 1;
                    }
                }
                ComponentKind::Edge => {
                    if let Some((from, to)) = edge_segment(&item, mesh_ref, index) {
                        total = total + (from + to) * 0.5;
                        count += 1;
                    }
                }
                ComponentKind::Vertex => {
                    if let Some(point) = vertex_point(&item, mesh_ref, index) {
                        total = total + point;
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            Some(total * (1.0 / count as f32))
        } else {
            None
        }
    }

    pub(crate) fn component_drag(
        &mut self,
        entity: u64,
        drag: &Drag,
        start: &Pose,
        result: &DragResult,
        snap: f32,
    ) -> Result<Json, Failure> {
        let is_box = self
            .item_of(entity)
            .is_some_and(|item| item.shape == Shape::Box);

        if is_box {
            let (start_position, rotation_degrees, scale, start_size) = {
                let (pos, rot, sc, size) = match self.box_parts(entity) {
                    Some((p, r, s, sz, _)) => (p, r, s, sz),
                    None => (
                        start.position,
                        start.rotation_degrees,
                        start.scale,
                        Vec3::ONE,
                    ),
                };
                let gesture = self.gesture.as_ref();
                (
                    find_vec3(gesture, "Transform", "position").unwrap_or(pos),
                    find_vec3(gesture, "Transform", "rotation_degrees").unwrap_or(rot),
                    find_vec3(gesture, "Transform", "scale").unwrap_or(sc),
                    find_vec3(gesture, "Brush", "size").unwrap_or(size),
                )
            };

            let moving = matches!(
                drag.handle,
                Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::MoveScreen
            );

            if moving {
                let signs = match self.component_kind {
                    ComponentKind::Face => {
                        let face_index = self.components.first().copied().unwrap_or(0);
                        match face_index {
                            0 => [1, 0, 0],
                            1 => [-1, 0, 0],
                            2 => [0, 1, 0],
                            3 => [0, -1, 0],
                            4 => [0, 0, 1],
                            5 => [0, 0, -1],
                            _ => [0, 0, 0],
                        }
                    }
                    ComponentKind::Edge => {
                        let edge_index = self.components.first().copied().unwrap_or(0) as usize;
                        let edges = box_edges();
                        if let Some(edge) = edges.get(edge_index) {
                            let (c1, c2) = (edge[0], edge[1]);
                            [
                                if (c1 & 1) == (c2 & 1) {
                                    if c1 & 1 != 0 { 1 } else { -1 }
                                } else {
                                    0
                                },
                                if (c1 & 2) == (c2 & 2) {
                                    if c1 & 2 != 0 { 1 } else { -1 }
                                } else {
                                    0
                                },
                                if (c1 & 4) == (c2 & 4) {
                                    if c1 & 4 != 0 { 1 } else { -1 }
                                } else {
                                    0
                                },
                            ]
                        } else {
                            [0, 0, 0]
                        }
                    }
                    ComponentKind::Vertex => {
                        let corner = self.components.first().copied().unwrap_or(0) as usize;
                        [
                            if corner & 1 != 0 { 1 } else { -1 },
                            if corner & 2 != 0 { 1 } else { -1 },
                            if corner & 4 != 0 { 1 } else { -1 },
                        ]
                    }
                };

                let delta = result.position - start.position;
                let rotation = rotation_from_euler(rotation_degrees);
                let local_delta = inverse_rotate(&rotation, delta);
                let snap_step = (snap > 0.0).then_some(snap);
                let (new_pos, new_size) = crate::brush_ops::resized_box(
                    start_position,
                    rotation_degrees,
                    scale,
                    start_size,
                    signs,
                    local_delta,
                    snap_step,
                );
                self.gesture_preview(
                    entity,
                    "Brush",
                    "size",
                    &crate::session::vector_json(new_size),
                )?;
                self.gesture_preview(
                    entity,
                    "Transform",
                    "position",
                    &crate::session::vector_json(new_pos),
                )?;
                let summary = format!(
                    "{:.2} × {:.2} × {:.2} m",
                    new_size.x, new_size.y, new_size.z
                )
                .replace('.', ",");
                self.hud = Some(summary.clone());
                return Ok(json!({ "summary": summary }));
            }

            for (column, value, before) in [
                ("position", result.position, start.position),
                (
                    "rotation_degrees",
                    result.rotation_degrees,
                    start.rotation_degrees,
                ),
                ("scale", result.scale, start.scale),
            ] {
                if value != before {
                    self.gesture_preview(
                        entity,
                        "Transform",
                        column,
                        &crate::session::vector_json(value),
                    )?;
                }
            }
            self.hud = Some(result.summary.clone());
            return Ok(json!({ "summary": result.summary }));
        }

        let (original_mesh, transform) = {
            let from_gesture = self.gesture.as_ref().and_then(|g| {
                g.originals.iter().find_map(|(t, c, val)| {
                    if t == "Mesh" && c == "geometry" {
                        match val {
                            Value::Mesh(mesh) => {
                                Some((mesh.clone(), self.transform_of_entity(entity)))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
            });
            match from_gesture.or_else(|| self.mesh_of(entity)) {
                Some(pair) => pair,
                None => return Ok(Json::Null),
            }
        };

        let mut affected: Vec<usize> = Vec::new();
        for &index in &self.components {
            match self.component_kind {
                ComponentKind::Vertex => {
                    affected.push(index as usize);
                }
                ComponentKind::Edge => {
                    let edges = mesh_edges(&original_mesh);
                    if let Some(edge) = edges.get(index as usize) {
                        affected.push(edge[0]);
                        affected.push(edge[1]);
                    }
                }
                ComponentKind::Face => {
                    if let Some(corners) = original_mesh.faces.get(index as usize) {
                        for &corner in corners {
                            affected.push(corner as usize);
                        }
                    }
                }
            }
        }
        affected.sort_unstable();
        affected.dedup();

        let mut new_mesh = original_mesh.clone();
        let local_pivot = to_local(&transform, start.pivot).unwrap_or(Vec3::ZERO);
        let rotation = rotation_from_euler(transform.rotation_degrees);

        let moving = matches!(
            drag.handle,
            Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::MoveScreen
        );

        if moving {
            let delta = result.position - start.position;
            let local_delta = inverse_rotate(&rotation, delta);
            let local_scaled = Vec3::new(
                local_delta.x / transform.scale.x.max(1.0e-6),
                local_delta.y / transform.scale.y.max(1.0e-6),
                local_delta.z / transform.scale.z.max(1.0e-6),
            );
            for &v in &affected {
                if let Some(pos) = new_mesh.positions.get_mut(v) {
                    *pos = *pos + local_scaled;
                }
            }
        } else if matches!(drag.handle, Handle::RotateAxis(_)) {
            let rot_delta = result.rotation_degrees - start.rotation_degrees;
            let q = rotation_from_euler(rot_delta);
            for &v in &affected {
                if let Some(pos) = new_mesh.positions.get_mut(v) {
                    let rel = *pos - local_pivot;
                    *pos = local_pivot + rotate_vector(&q, rel);
                }
            }
        } else {
            let scale_factor = Vec3::new(
                result.scale.x / start.scale.x.max(1.0e-6),
                result.scale.y / start.scale.y.max(1.0e-6),
                result.scale.z / start.scale.z.max(1.0e-6),
            );
            for &v in &affected {
                if let Some(pos) = new_mesh.positions.get_mut(v) {
                    let rel = *pos - local_pivot;
                    *pos = local_pivot
                        + Vec3::new(
                            rel.x * scale_factor.x,
                            rel.y * scale_factor.y,
                            rel.z * scale_factor.z,
                        );
                }
            }
        }

        self.gesture_preview(entity, "Mesh", "geometry", &new_mesh.to_text_json())?;
        self.hud = Some(result.summary.clone());
        Ok(json!({ "summary": result.summary }))
    }
}

fn box_component(item: &DrawItem, point: Vec3, kind: ComponentKind) -> Option<u32> {
    let local = to_local(&item.transform, point)?;
    let half = item.size * 0.5;
    let axis = (0..3).max_by(|&left, &right| {
        let left_distance = axis_of(local, left).abs() / axis_of(half, left).max(1.0e-6);
        let right_distance = axis_of(local, right).abs() / axis_of(half, right).max(1.0e-6);
        left_distance.total_cmp(&right_distance)
    })?;
    let sign = axis_of(local, axis).signum();
    let face = box_face_index(axis, sign)?;
    let corners = BOX_FACES.get(face)?;
    match kind {
        ComponentKind::Face => Some(face as u32),
        ComponentKind::Vertex => {
            let (index, _) = corners.iter().enumerate().min_by(|(_, left), (_, right)| {
                let left_distance = (box_corner(item, **left) - point).length_squared();
                let right_distance = (box_corner(item, **right) - point).length_squared();
                left_distance.total_cmp(&right_distance)
            })?;
            corners.get(index).map(|corner| *corner as u32)
        }
        ComponentKind::Edge => {
            let edges = box_edges();
            let mut best: Option<(usize, f32)> = None;
            for slot in 0..4 {
                let start = *corners.get(slot)?;
                let end = *corners.get((slot + 1) % 4)?;
                let key = ordered(start, end);
                let Some(edge_index) = edges.iter().position(|edge| *edge == key) else {
                    continue;
                };
                let closest =
                    closest_on_segment(point, box_corner(item, start), box_corner(item, end));
                let distance = (closest - point).length_squared();
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((edge_index, distance));
                }
            }
            best.map(|(index, _)| index as u32)
        }
    }
}

fn mesh_component(mesh: &PolygonMesh, local: Vec3, kind: ComponentKind) -> Option<u32> {
    let face = mesh.faces.iter().position(|corners| {
        let mut points = Vec::new();
        for index in corners {
            let Some(point) = mesh.positions.get(*index as usize) else {
                return false;
            };
            points.push(*point);
        }
        let Some((first, rest)) = points.split_first() else {
            return false;
        };
        rest.windows(2).any(|pair| {
            let (Some(second), Some(third)) = (pair.first().copied(), pair.get(1).copied()) else {
                return false;
            };
            on_triangle(local, *first, second, third)
        })
    })?;
    match kind {
        ComponentKind::Face => Some(face as u32),
        ComponentKind::Vertex => {
            let corners = mesh.faces.get(face)?;
            corners
                .iter()
                .min_by(|left, right| {
                    let left_point = mesh
                        .positions
                        .get(**left as usize)
                        .copied()
                        .unwrap_or(Vec3::ZERO);
                    let right_point = mesh
                        .positions
                        .get(**right as usize)
                        .copied()
                        .unwrap_or(Vec3::ZERO);
                    (left_point - local)
                        .length_squared()
                        .total_cmp(&(right_point - local).length_squared())
                })
                .copied()
        }
        ComponentKind::Edge => {
            let corners = mesh.faces.get(face)?;
            let edges = mesh_edges(mesh);
            let mut best: Option<(usize, f32)> = None;
            for slot in 0..corners.len() {
                let Some(start) = corners.get(slot).copied() else {
                    continue;
                };
                let Some(end) = corners.get((slot + 1) % corners.len()).copied() else {
                    continue;
                };
                let key = ordered(start as usize, end as usize);
                let Some(edge_index) = edges.iter().position(|edge| *edge == key) else {
                    continue;
                };
                let Some(start_point) = mesh.positions.get(start as usize).copied() else {
                    continue;
                };
                let Some(end_point) = mesh.positions.get(end as usize).copied() else {
                    continue;
                };
                let closest = closest_on_segment(local, start_point, end_point);
                let distance = (closest - local).length_squared();
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((edge_index, distance));
                }
            }
            best.map(|(index, _)| index as u32)
        }
    }
}

fn find_vec3(gesture: Option<&crate::session::Gesture>, table: &str, column: &str) -> Option<Vec3> {
    gesture?.originals.iter().find_map(|(t, c, val)| {
        if t == table && c == column {
            match val {
                Value::Vec3(v) => Some(Vec3::new(v.x, v.y, v.z)),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn face_edges(item: &DrawItem, mesh: Option<&PolygonMesh>, index: u32) -> Vec<(Vec3, Vec3)> {
    if item.shape == Shape::Box {
        let Some(corners) = BOX_FACES.get(index as usize) else {
            return Vec::new();
        };
        return (0..4)
            .filter_map(|slot| {
                let start = box_corner(item, *corners.get(slot)?);
                let end = box_corner(item, *corners.get((slot + 1) % 4)?);
                Some((start, end))
            })
            .collect();
    }
    let Some(mesh) = mesh else {
        return Vec::new();
    };
    let Some(corners) = mesh.faces.get(index as usize) else {
        return Vec::new();
    };
    (0..corners.len())
        .filter_map(|slot| {
            let start = mesh.positions.get(*corners.get(slot)? as usize).copied()?;
            let end = mesh
                .positions
                .get(*corners.get((slot + 1) % corners.len())? as usize)
                .copied()?;
            Some((
                to_world(&item.transform, start),
                to_world(&item.transform, end),
            ))
        })
        .collect()
}

fn edge_segment(item: &DrawItem, mesh: Option<&PolygonMesh>, index: u32) -> Option<(Vec3, Vec3)> {
    if item.shape == Shape::Box {
        let edge = box_edges().get(index as usize).copied()?;
        return Some((
            box_corner(item, *edge.first()?),
            box_corner(item, *edge.get(1)?),
        ));
    }
    let mesh = mesh?;
    let edge = mesh_edges(mesh).get(index as usize).copied()?;
    let start = mesh.positions.get(*edge.first()?).copied()?;
    let end = mesh.positions.get(*edge.get(1)?).copied()?;
    Some((
        to_world(&item.transform, start),
        to_world(&item.transform, end),
    ))
}

fn vertex_point(item: &DrawItem, mesh: Option<&PolygonMesh>, index: u32) -> Option<Vec3> {
    if item.shape == Shape::Box {
        return Some(box_corner(item, index as usize));
    }
    let mesh = mesh?;
    mesh.positions
        .get(index as usize)
        .copied()
        .map(|point| to_world(&item.transform, point))
}

fn box_corner(item: &DrawItem, index: usize) -> Vec3 {
    let half = item.size * 0.5;
    let local = Vec3::new(
        if index & 1 == 0 { -half.x } else { half.x },
        if index & 2 == 0 { -half.y } else { half.y },
        if index & 4 == 0 { -half.z } else { half.z },
    );
    to_world(&item.transform, local)
}

fn box_edges() -> Vec<[usize; 2]> {
    let mut edges = Vec::new();
    for face in BOX_FACES {
        for slot in 0..4 {
            let Some(start) = face.get(slot).copied() else {
                continue;
            };
            let Some(end) = face.get((slot + 1) % 4).copied() else {
                continue;
            };
            let key = ordered(start, end);
            if !edges.contains(&key) {
                edges.push(key);
            }
        }
    }
    edges
}

pub(crate) fn mesh_edges(mesh: &PolygonMesh) -> Vec<[usize; 2]> {
    let mut edges = Vec::new();
    for face in &mesh.faces {
        for slot in 0..face.len() {
            let Some(start) = face.get(slot).copied() else {
                continue;
            };
            let Some(end) = face.get((slot + 1) % face.len()).copied() else {
                continue;
            };
            let key = ordered(start as usize, end as usize);
            if !edges.contains(&key) {
                edges.push(key);
            }
        }
    }
    edges
}

fn ordered(start: usize, end: usize) -> [usize; 2] {
    if start < end {
        [start, end]
    } else {
        [end, start]
    }
}

fn closest_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let edge = end - start;
    let length = edge.length_squared();
    if length < 1.0e-12 {
        return start;
    }
    let t = ((point - start).dot(edge) / length).clamp(0.0, 1.0);
    start + edge * t
}

fn on_triangle(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    let normal = (b - a).cross(c - a);
    if normal.length_squared() < 1.0e-12 {
        return false;
    }
    let unit = normal.normalized_or_zero();
    if (point - a).dot(unit).abs() > 0.05 {
        return false;
    }
    let v0 = b - a;
    let v1 = c - a;
    let v2 = point - a;
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let denominator = dot00 * dot11 - dot01 * dot01;
    if denominator.abs() < 1.0e-12 {
        return false;
    }
    let u = (dot11 * dot02 - dot01 * dot12) / denominator;
    let v = (dot00 * dot12 - dot01 * dot02) / denominator;
    u >= -0.08 && v >= -0.08 && u + v <= 1.08
}

fn axis_of(point: Vec3, axis: usize) -> f32 {
    match axis {
        0 => point.x,
        1 => point.y,
        _ => point.z,
    }
}

fn to_world(transform: &Transform, local: Vec3) -> Vec3 {
    let scaled = Vec3::new(
        local.x * transform.scale.x,
        local.y * transform.scale.y,
        local.z * transform.scale.z,
    );
    transform.position + rotate_vector(&rotation_from_euler(transform.rotation_degrees), scaled)
}

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

#[cfg(test)]
mod tests {
    use petunia_math::Vec3;
    use serde_json::json;

    use crate::gizmo::Tool;
    use crate::session::EditorSession;
    use crate::tools::Pointer;

    fn call(session: &mut EditorSession, request: serde_json::Value) -> serde_json::Value {
        let reply = session.handle(&request);
        assert_eq!(
            reply.get("ok"),
            Some(&serde_json::Value::Bool(true)),
            "{reply}"
        );
        reply
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    #[test]
    fn a_click_on_the_selected_box_picks_its_top_face_and_escape_keeps_the_box() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_component_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "component" }),
        );
        let created = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = created
            .get("entity")
            .and_then(serde_json::Value::as_u64)
            .expect("box");
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": entity, "table": "Transform",
                "values": { "position": { "x": 40.0, "y": 1.0, "z": 40.0 } }
            }}),
        );
        session.select(Some(entity)).expect("select");
        session.set_tool(Tool::Move);
        session.camera.camera.position = Vec3::new(40.0, 12.0, 40.0);
        session.camera.camera.yaw_degrees = 0.0;
        session.camera.camera.pitch_degrees = -89.9;
        let pointer = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        let picked = session.component_pick(pointer, false).expect("pick");
        assert_eq!(picked.get("picked"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(
            picked.get("label").and_then(serde_json::Value::as_str),
            Some("face +Y")
        );
        let vertex = session
            .component_set_kind("vertex", Some(pointer))
            .expect("vertex");
        assert_eq!(vertex.get("picked"), Some(&serde_json::Value::Bool(true)));
        assert!(
            vertex
                .get("label")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .starts_with("vértice")
        );
        let cleared = session.select(None).expect("escape");
        assert_eq!(
            cleared.get("component_cleared"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(session.selection, Some(entity));
        assert!(session.components.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moving_a_box_face_with_the_gizmo_grows_that_face_and_undo_reverts() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_component_move_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "component_move" }),
        );
        let created = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = created
            .get("entity")
            .and_then(serde_json::Value::as_u64)
            .expect("box");
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": entity, "table": "Transform",
                "values": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } }
            }}),
        );
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": entity, "table": "Brush",
                "values": { "size": { "x": 2.0, "y": 2.0, "z": 2.0 }, "detail": 16, "shape": "box" }
            }}),
        );
        session.select(Some(entity)).expect("select");
        session.set_tool(Tool::Move);
        session.camera.camera.position = Vec3::new(0.0, 5.0, 5.0);
        session.camera.camera.yaw_degrees = 0.0;
        session.camera.camera.pitch_degrees = -45.0;
        session.components = vec![2]; // face +Y
        session.component_kind = super::ComponentKind::Face;
        let pointer = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        let pivot = session.component_pivot(entity).expect("pivot");
        assert!((pivot.y - 2.0).abs() < 1e-4, "pivot y={}", pivot.y);

        // Start drag on the gizmo
        let pose = session.selection_pose().expect("pose").1;
        let view = session.view_for(pointer);
        let model = crate::gizmo::screen_gizmo(
            Tool::Move,
            &pose,
            &view,
            crate::gizmo::Orientation::Global,
            1.0,
        )
        .expect("model");
        let drag = crate::gizmo::begin_drag(
            crate::gizmo::Handle::MoveAxis(1),
            &model,
            &pose,
            &view,
            crate::gizmo::Orientation::Global,
            [400.0, 300.0],
        )
        .expect("drag");
        session.drag = Some(drag);

        let drag_result = crate::gizmo::DragResult {
            position: Vec3::new(0.0, 2.0, 0.0), // moved up by 1.0 m (start pos was 0.0, 1.0, 0.0)
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
            summary: "Y +1,00 m".to_string(),
        };
        session
            .component_drag(entity, &drag, &drag.start_pose(), &drag_result, 0.0)
            .expect("drag");
        session.gizmo_end().expect("commit");

        let (_, _, _, size, _) = session.box_parts(entity).expect("parts");
        assert!((size.y - 3.0).abs() < 1e-4, "new size.y = {}", size.y);

        // One undo restores the previous size
        session.undo().expect("undo");
        let (_, _, _, size_reverted, _) = session.box_parts(entity).expect("parts");
        assert!(
            (size_reverted.y - 2.0).abs() < 1e-4,
            "reverted size.y = {}",
            size_reverted.y
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moving_a_mesh_vertex_moves_that_vertex_and_undo_reverts() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_component_mesh_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "component_mesh" }),
        );
        let created = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = created
            .get("entity")
            .and_then(serde_json::Value::as_u64)
            .expect("box");
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "row.set", "payload": {
                "entity": entity, "table": "Transform",
                "values": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } }
            }}),
        );
        call(
            &mut session,
            json!({ "op": "mesh.convert", "entity": entity }),
        );

        session.select(Some(entity)).expect("select");
        session.set_tool(Tool::Move);
        session.camera.camera.position = Vec3::new(0.0, 4.0, 6.0);
        session.camera.camera.yaw_degrees = 0.0;
        session.camera.camera.pitch_degrees = -30.0;
        let pointer = Pointer {
            x: 400.0,
            y: 300.0,
            width: 800.0,
            height: 600.0,
        };
        session
            .component_set_kind("vertex", Some(pointer))
            .expect("vertex");
        assert!(!session.components.is_empty());

        let (mesh_before, _) = session.mesh_of(entity).expect("mesh");
        let v_idx = session.components[0] as usize;
        let initial_v_pos = mesh_before.positions[v_idx];

        let pose = session.selection_pose().expect("pose").1;
        let view = session.view_for(pointer);
        let model = crate::gizmo::screen_gizmo(
            Tool::Move,
            &pose,
            &view,
            crate::gizmo::Orientation::Global,
            1.0,
        )
        .expect("model");
        let drag = crate::gizmo::begin_drag(
            crate::gizmo::Handle::MoveAxis(1),
            &model,
            &pose,
            &view,
            crate::gizmo::Orientation::Global,
            [400.0, 300.0],
        )
        .expect("drag");
        session.drag = Some(drag);

        let drag_result = crate::gizmo::DragResult {
            position: Vec3::new(0.0, 2.5, 0.0),
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
            summary: "Y +1,50 m".to_string(),
        };
        session
            .component_drag(entity, &drag, &drag.start_pose(), &drag_result, 0.0)
            .expect("drag");
        session.gizmo_end().expect("commit");

        let (mesh_after, _) = session.mesh_of(entity).expect("mesh");
        assert!(
            (mesh_after.positions[v_idx].y - (initial_v_pos.y + 1.5)).abs() < 1e-4,
            "moved vertex y = {}",
            mesh_after.positions[v_idx].y
        );

        // One undo restores the vertex position
        session.undo().expect("undo");
        let (mesh_reverted, _) = session.mesh_of(entity).expect("mesh");
        assert!(
            (mesh_reverted.positions[v_idx].y - initial_v_pos.y).abs() < 1e-4,
            "reverted vertex y = {}",
            mesh_reverted.positions[v_idx].y
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
