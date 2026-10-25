//! The editor's transform tools on top of the session: active tool, orientation, pivot policy,
//! snapping to the grid and to other objects, free mode, floors, hover (pre-selection), preset
//! views and the overlay model the GUI paints. All value changes go through gestures, so a drag
//! is one undo step.
//!
//! Ported from Petunia3D: the pivot policy (`core::PivotPoint`), the snap targets
//! (`core::snap::SnapTarget`: Smart, Grid, Increment, Point, Face) and the single-tap / double-tap
//! activation (`docs/manual/modeling.md`: two quick taps enter the modal "free mode").

use petunia_math::{Color, Vec3};
use petunia_render::{
    DrawItem, OverlayItem, RenderFrame, SurfaceHit, build_mesh, mesh_points, model_matrix,
    pick_entity, pick_surface,
};
use petunia_world::PersistentId;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::{
    self, Drag, DragResult, Handle, Orientation, Pose, ROD_LENGTH, ScreenGizmo, Snap, Tool, View,
    gizmo_json,
};
use crate::session::{EditorSession, vec3_of, vector_json};

const SELECTION_COLOR: Color = Color::new(1.0, 0.62, 0.10, 1.0);
const HOVER_COLOR: Color = Color::new(0.49, 0.86, 1.0, 0.95);
const FACE_COLOR: Color = Color::new(1.0, 0.82, 0.25, 1.0);
/// How close (pixels) a corner of another object must be to catch the pivot.
const POINT_SNAP_PIXELS: f32 = 14.0;
/// Default height of a floor ("andar"), in meters.
pub const DEFAULT_FLOOR_HEIGHT: f32 = 3.0;

/// Where the gizmo turns and scales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotMode {
    /// The object's own pivot (`Transform.pivot`, editable with "Editar pivô").
    Object,
    /// The center of its bounding box.
    BoundsCenter,
    /// The bottom center of its bounding box (objects stay on the floor).
    Base,
}

impl PivotMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "object" | "origin" => Some(Self::Object),
            "bounds_center" => Some(Self::BoundsCenter),
            "base" => Some(Self::Base),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::BoundsCenter => "bounds_center",
            Self::Base => "base",
        }
    }
}

/// What a moving object snaps to (Petunia3D `SnapTarget`, minus Edge for now).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapTarget {
    /// Corner under the mouse, else the surface under it, else the grid.
    Smart,
    /// Absolute grid positions.
    Grid,
    /// Steps measured from where the drag started.
    Increment,
    /// Corners of other objects.
    Point,
    /// The surface under the mouse: the object sits on it, optionally tilted with the slope.
    Face,
}

impl SnapTarget {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "smart" => Some(Self::Smart),
            "grid" => Some(Self::Grid),
            "increment" => Some(Self::Increment),
            "point" => Some(Self::Point),
            "face" => Some(Self::Face),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Grid => "grid",
            Self::Increment => "increment",
            Self::Point => "point",
            Self::Face => "face",
        }
    }
}

/// A point in the viewport: the mouse and the viewport size, both in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Pointer {
    pub(crate) fn mouse(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Free mode (double tap on G, R or S): the mouse drives the value, X/Y/Z constrain the axis,
/// typed digits set an exact value.
#[derive(Clone, Debug, Default)]
pub struct FreeMode {
    pub axis: Option<usize>,
    pub typed: String,
}

/// World-space bounding box of one draw item (from its real mesh).
pub(crate) fn item_bounds(item: &DrawItem) -> Option<(Vec3, Vec3)> {
    let mesh = build_mesh(item.shape, item.size, item.detail).ok()?;
    let matrix = model_matrix(&item.transform);
    let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
    let mut low = Vec3::splat(f32::MAX);
    let mut high = Vec3::splat(f32::MIN);
    for [x, y, z] in &mesh.positions {
        let world = Vec3::new(
            entry(0) * x + entry(4) * y + entry(8) * z + entry(12),
            entry(1) * x + entry(5) * y + entry(9) * z + entry(13),
            entry(2) * x + entry(6) * y + entry(10) * z + entry(14),
        );
        low = Vec3::new(low.x.min(world.x), low.y.min(world.y), low.z.min(world.z));
        high = Vec3::new(
            high.x.max(world.x),
            high.y.max(world.y),
            high.z.max(world.z),
        );
    }
    (low.x <= high.x).then_some((low, high))
}

/// Lowest point of an item's mesh along its own up axis, in its own (unscaled) units.
fn local_bottom(item: &DrawItem) -> f32 {
    build_mesh(item.shape, item.size, item.detail)
        .map(|mesh| mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min))
        .unwrap_or(-item.size.y * 0.5)
}

/// The twelve edges of a box as constant-width lines.
fn box_lines(low: Vec3, high: Vec3, color: Color, width_pixels: f32, out: &mut Vec<OverlayItem>) {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { high.x } else { low.x },
            if y { high.y } else { low.y },
            if z { high.z } else { low.z },
        )
    };
    let edges = [
        ((false, false, false), (true, false, false)),
        ((false, true, false), (true, true, false)),
        ((false, false, true), (true, false, true)),
        ((false, true, true), (true, true, true)),
        ((false, false, false), (false, true, false)),
        ((true, false, false), (true, true, false)),
        ((false, false, true), (false, true, true)),
        ((true, false, true), (true, true, true)),
        ((false, false, false), (false, false, true)),
        ((true, false, false), (true, false, true)),
        ((false, true, false), (false, true, true)),
        ((true, true, false), (true, true, true)),
    ];
    for ((ax, ay, az), (bx, by, bz)) in edges {
        out.push(OverlayItem::Line {
            from: corner(ax, ay, az),
            to: corner(bx, by, bz),
            color,
            width_pixels,
        });
    }
}

fn snap_label(target: SnapTarget) -> &'static str {
    match target {
        SnapTarget::Smart => "inteligente",
        SnapTarget::Grid => "grade",
        SnapTarget::Increment => "incremento",
        SnapTarget::Point => "ponto",
        SnapTarget::Face => "superfície",
    }
}

impl EditorSession {
    pub fn set_tool(&mut self, tool: Tool) -> Json {
        if self.drag.is_some() {
            let _ = self.gizmo_cancel();
        }
        if self.face_drag.is_some() {
            let _ = self.face_drag_cancel();
        }
        self.tool = tool;
        self.hover = None;
        self.hovered_face = None;
        self.free = None;
        json!({ "tool": tool.name() })
    }

    /// Orientation of the move/rotate axes, the pivot policy, and "edit pivot".
    pub fn set_transform_options(
        &mut self,
        orientation: Option<&str>,
        pivot: Option<&str>,
        edit_pivot: Option<bool>,
    ) -> Result<Json, Failure> {
        if let Some(name) = orientation {
            self.orientation = Orientation::parse(name).ok_or_else(|| {
                Failure::bad_request(&format!(
                    "orientação `{name}` desconhecida (global ou local)"
                ))
            })?;
        }
        if let Some(name) = pivot {
            self.pivot_mode = PivotMode::parse(name).ok_or_else(|| {
                Failure::bad_request(&format!(
                    "pivô `{name}` desconhecido (object, bounds_center ou base)"
                ))
            })?;
        }
        if let Some(enabled) = edit_pivot {
            if self.drag.is_some() {
                self.gizmo_cancel()?;
            }
            self.edit_pivot = enabled;
        }
        Ok(self.transform_json())
    }

    /// Changes snapping. Steps must be finite and positive.
    pub fn set_snap(
        &mut self,
        enabled: Option<bool>,
        steps: [Option<f32>; 3],
        target: Option<&str>,
        align_to_slope: Option<bool>,
    ) -> Result<Json, Failure> {
        let valid = |value: Option<f32>| value.is_none_or(|step| step.is_finite() && step > 0.0);
        if !steps.iter().all(|step| valid(*step)) {
            return Err(Failure::bad_request(
                "os passos de snap precisam ser números positivos",
            ));
        }
        if let Some(name) = target {
            self.snap_target = SnapTarget::parse(name).ok_or_else(|| {
                Failure::bad_request(&format!(
                    "alvo de snap `{name}` desconhecido (smart, grid, increment, point ou face)"
                ))
            })?;
        }
        if let Some(value) = align_to_slope {
            self.align_to_slope = value;
        }
        let [translate, rotate_degrees, scale] = steps;
        let snap: &mut Snap = &mut self.snap;
        if let Some(value) = enabled {
            snap.enabled = value;
        }
        if let Some(value) = translate {
            snap.translate = value;
        }
        if let Some(value) = rotate_degrees {
            snap.rotate_degrees = value;
        }
        if let Some(value) = scale {
            snap.scale = value;
        }
        Ok(self.snap_json())
    }

    pub(crate) fn snap_json(&self) -> Json {
        json!({
            "enabled": self.snap.enabled,
            "translate": self.snap.translate,
            "rotate_degrees": self.snap.rotate_degrees,
            "scale": self.snap.scale,
            "target": self.snap_target.name(),
            "align_to_slope": self.align_to_slope,
        })
    }

    pub(crate) fn view_for(&self, pointer: Pointer) -> View {
        View {
            camera: self.camera.camera,
            width: pointer.width,
            height: pointer.height,
        }
    }

    /// What the tools can hit and snap to (objects hidden or ghosted above the floor are out).
    pub(crate) fn draw_items(&self) -> Vec<DrawItem> {
        let items = self
            .project
            .as_ref()
            .map(|project| petunia_render::extract_frame(&project.world, self.camera.camera).items)
            .unwrap_or_default();
        self.pickable(items)
    }

    pub(crate) fn item_of(&self, entity: u64) -> Option<DrawItem> {
        self.draw_items()
            .into_iter()
            .find(|item| item.entity == entity)
    }

    /// The object's own pivot in world space: position + rotation · (scale ∘ pivot offset).
    fn object_pivot(&self, id: u64, position: Vec3, rotation: Vec3, scale: Vec3) -> Vec3 {
        let Some(project) = &self.project else {
            return position;
        };
        let offset = vec3_of(&project.world, id, "Transform", "pivot").unwrap_or(Vec3::ZERO);
        let scaled = Vec3::new(offset.x * scale.x, offset.y * scale.y, offset.z * scale.z);
        position + gizmo::rotate_vector(&gizmo::rotation_from_euler(rotation), scaled)
    }

    /// The pivot for the current policy, before any drag.
    fn pivot_for(&self, id: u64, position: Vec3, rotation: Vec3, scale: Vec3) -> Vec3 {
        let bounds = || self.item_of(id).as_ref().and_then(item_bounds);
        match self.pivot_mode {
            PivotMode::Object => self.object_pivot(id, position, rotation, scale),
            PivotMode::BoundsCenter => bounds().map_or(position, |(low, high)| (low + high) * 0.5),
            PivotMode::Base => bounds().map_or(position, |(low, high)| {
                Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5)
            }),
        }
    }

    /// Where the selected object is, if it has a `Transform`, with the pivot to use. During a
    /// drag the gizmo follows the object when moving and stays put when turning or scaling
    /// (Petunia3D's `displayed_pivot`). Editing the pivot always uses the object's own pivot.
    pub(crate) fn selection_pose(&self) -> Option<(u64, Pose)> {
        let id = self.selection?;
        let world = &self.project.as_ref()?.world;
        let handle = world.handle_of(PersistentId(id)).ok()?;
        if !world.has_row(handle, "Transform") {
            return None;
        }
        let position = vec3_of(world, id, "Transform", "position").unwrap_or(Vec3::ZERO);
        let rotation = vec3_of(world, id, "Transform", "rotation_degrees").unwrap_or(Vec3::ZERO);
        let scale = vec3_of(world, id, "Transform", "scale").unwrap_or(Vec3::ONE);
        let pivot = match self.drag {
            Some(drag) if !self.edit_pivot => {
                let start = drag.start_pose();
                match drag.handle {
                    Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::MoveScreen => {
                        start.pivot + (position - start.position)
                    }
                    _ => start.pivot,
                }
            }
            _ if self.edit_pivot => self.object_pivot(id, position, rotation, scale),
            _ => self.pivot_for(id, position, rotation, scale),
        };
        Some((
            id,
            Pose {
                position,
                rotation_degrees: rotation,
                scale,
                pivot,
            },
        ))
    }

    /// The gizmo actually shown: editing the pivot always shows the move gizmo.
    fn gizmo_tool(&self) -> Tool {
        if self.edit_pivot {
            Tool::Move
        } else {
            self.tool
        }
    }

    fn screen_gizmo(&self, pointer: Pointer) -> Option<(u64, Pose, ScreenGizmo)> {
        let (id, pose) = self.selection_pose()?;
        let model = gizmo::screen_gizmo(
            self.gizmo_tool(),
            &pose,
            &self.view_for(pointer),
            self.orientation,
            self.gizmo_size,
        )?;
        Some((id, pose, model))
    }

    /// Pre-selection: the gizmo handle under the mouse, the face under it (face tools), or else
    /// the object under it.
    pub fn pointer_hover(&mut self, pointer: Pointer) -> Json {
        if let Some(drag) = self.drag {
            return json!({ "handle": drag.handle.name(), "entity": null, "face": null });
        }
        if self.tool.is_sculpting() {
            self.hover = None;
            self.hovered_face = None;
            return self.sculpt_hover(pointer);
        }
        if self.tool.works_on_faces() && !self.edit_pivot {
            self.hover = None;
            self.hovered_face = self.face_under(pointer);
            self.hovered = self.hovered_face.map(|face| face.entity);
            return json!({
                "handle": null,
                "entity": self.hovered,
                "face": self.hovered_face.map(|face| face.axis),
            });
        }
        self.hovered_face = None;
        self.hover = self
            .screen_gizmo(pointer)
            .and_then(|(_, _, model)| gizmo::hit_test(&model, pointer.mouse()));
        self.hovered = if self.hover.is_some() {
            None
        } else {
            self.entity_under(pointer).map(|(entity, _)| entity)
        };
        json!({ "handle": self.hover.map(Handle::name), "entity": self.hovered, "face": null })
    }

    pub(crate) fn entity_under(&self, pointer: Pointer) -> Option<(u64, f32)> {
        let view = self.view_for(pointer);
        if !view.is_valid() {
            return None;
        }
        let ray = view.ray(pointer.mouse())?;
        let brush = pick_entity(&self.draw_items(), &ray);
        let sculpt = self
            .sculpt_hit(&ray, &[])
            .map(|hit| (hit.entity, hit.distance));
        match (brush, sculpt) {
            (Some(brush), Some(sculpt)) => Some(if sculpt.1 < brush.1 { sculpt } else { brush }),
            (brush, sculpt) => brush.or(sculpt),
        }
    }

    pub(crate) fn surface_under(
        &self,
        pointer: Pointer,
        ignore: &[u64],
    ) -> Option<(SurfaceHit, Vec3)> {
        let view = self.view_for(pointer);
        if !view.is_valid() {
            return None;
        }
        let ray = view.ray(pointer.mouse())?;
        // Objects sit on brushes and on cave floors alike.
        let brush = pick_surface(&self.draw_items(), &ray, ignore);
        let sculpt = self.sculpt_hit(&ray, ignore);
        let hit = match (brush, sculpt) {
            (Some(brush), Some(sculpt)) => {
                if sculpt.distance < brush.distance {
                    sculpt
                } else {
                    brush
                }
            }
            (brush, sculpt) => brush.or(sculpt)?,
        };
        Some((hit, ray.origin + ray.direction * hit.distance))
    }

    // ---- gizmo drag -------------------------------------------------------------------------

    /// Starts dragging the handle under the mouse. `false` when there is none.
    pub fn gizmo_begin(&mut self, pointer: Pointer) -> bool {
        let Some((_, pose, model)) = self.screen_gizmo(pointer) else {
            return false;
        };
        let Some(handle) = gizmo::hit_test(&model, pointer.mouse()) else {
            return false;
        };
        self.start_drag(handle, &pose, &model, pointer)
    }

    fn start_drag(
        &mut self,
        handle: Handle,
        pose: &Pose,
        model: &ScreenGizmo,
        pointer: Pointer,
    ) -> bool {
        let view = self.view_for(pointer);
        match gizmo::begin_drag(
            handle,
            model,
            pose,
            &view,
            self.orientation,
            pointer.mouse(),
        ) {
            Some(drag) => {
                self.drag = Some(drag);
                self.hover = Some(handle);
                self.hud = Some(String::new());
                true
            }
            None => false,
        }
    }

    fn effective_snap(&self, invert: bool) -> Snap {
        Snap {
            enabled: self.snap.enabled != invert,
            absolute: self.snap_target != SnapTarget::Increment,
            ..self.snap
        }
    }

    /// Moves the dragged handle to the mouse. `invert_snap` flips snapping for this move (Ctrl).
    pub fn gizmo_drag(&mut self, pointer: Pointer, invert_snap: bool) -> Result<Json, Failure> {
        let (Some(drag), Some(entity)) = (self.drag, self.selection) else {
            return Ok(Json::Null);
        };
        let snap = self.effective_snap(invert_snap);
        let view = self.view_for(pointer);
        let typed = self
            .free
            .as_ref()
            .map(|free| free.typed.replace(',', "."))
            .filter(|text| !text.is_empty() && text != "-");
        let result = match &typed {
            Some(text) => text
                .parse::<f32>()
                .ok()
                .and_then(|value| gizmo::drag_typed(&drag, &view, value)),
            None => gizmo::drag_to(&drag, &view, pointer.mouse(), &snap),
        };
        let Some(mut result) = result else {
            return Ok(Json::Null);
        };
        let moving = matches!(
            drag.handle,
            Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::MoveScreen
        );
        if moving && !self.edit_pivot && snap.enabled && typed.is_none() {
            self.snap_to_objects(&drag, pointer, &mut result);
        }
        let start = drag.start_pose();
        if self.edit_pivot {
            // Only the pivot moves: store it in the object's own axes and scale.
            let new_pivot = start.pivot + (result.position - start.position);
            let rotation = gizmo::rotation_from_euler(start.rotation_degrees);
            let local = gizmo::inverse_rotate(&rotation, new_pivot - start.position);
            let offset = Vec3::new(
                local.x / start.scale.x.max(1.0e-6),
                local.y / start.scale.y.max(1.0e-6),
                local.z / start.scale.z.max(1.0e-6),
            );
            self.gesture_preview(entity, "Transform", "pivot", &vector_json(offset))?;
            self.hud = Some(format!("Pivô {}", result.summary));
            return Ok(json!({ "summary": self.hud }));
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
                self.gesture_preview(entity, "Transform", column, &vector_json(value))?;
            }
        }
        self.hud = Some(result.summary.clone());
        Ok(json!({ "summary": result.summary }))
    }

    /// Snapping to other objects (Petunia3D Smart / Point / Face targets): the pivot jumps onto
    /// a corner of another object, or the object sits on the surface under the mouse, tilted
    /// with the slope when "alinhar à inclinação" is on. Elevations count: the top of a crate is
    /// a surface like the floor.
    fn snap_to_objects(&self, drag: &Drag, pointer: Pointer, result: &mut DragResult) {
        let Some(entity) = self.selection else {
            return;
        };
        let start = drag.start_pose();
        let target = self.snap_target;
        let free_plane = !matches!(drag.handle, Handle::MoveAxis(_));

        if matches!(target, SnapTarget::Smart | SnapTarget::Point) {
            let view = self.view_for(pointer);
            let pivot_now = result.position + (start.pivot - start.position);
            let nearest = mesh_points(&self.draw_items(), &[entity])
                .into_iter()
                .filter_map(|point| view.project(point).map(|screen| (point, screen)))
                .map(|(point, screen)| {
                    let gap =
                        ((screen[0] - pointer.x).powi(2) + (screen[1] - pointer.y).powi(2)).sqrt();
                    (point, gap)
                })
                .filter(|(_, gap)| *gap <= POINT_SNAP_PIXELS)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((point, _)) = nearest {
                let delta = point - pivot_now;
                // Keep the constraint: along one axis, or inside the dragged plane.
                let axes = drag.axes();
                let allowed: Vec<usize> = match drag.handle {
                    Handle::MoveAxis(index) => vec![index],
                    Handle::MovePlane(normal) => vec![(normal + 1) % 3, (normal + 2) % 3],
                    _ => vec![0, 1, 2],
                };
                let mut constrained = Vec3::ZERO;
                for index in allowed {
                    let axis = axes.get(index).copied().unwrap_or(Vec3::ZERO);
                    constrained = constrained + axis * delta.dot(axis);
                }
                result.position = result.position + constrained;
                result.summary = format!("{} · no ponto", result.summary);
                return;
            }
        }

        if !(matches!(target, SnapTarget::Smart | SnapTarget::Face) && free_plane) {
            return;
        }
        let Some(item) = self.item_of(entity) else {
            return;
        };
        let floor = self.floor_elevation();
        let (point, normal, on_object) = match self.surface_under(pointer, &[entity]) {
            Some((hit, point)) => (point, hit.normal, true),
            None => {
                // Nothing under the mouse: the floor being built.
                let view = self.view_for(pointer);
                let Some(ray) = view.ray(pointer.mouse()) else {
                    return;
                };
                if ray.direction.y.abs() < 1.0e-4 {
                    return;
                }
                let distance = (floor - ray.origin.y) / ray.direction.y;
                if distance <= 0.0 {
                    return;
                }
                (ray.origin + ray.direction * distance, Vec3::UP, false)
            }
        };
        if target == SnapTarget::Smart && !on_object && drag.handle != Handle::MoveScreen {
            return; // smart: a plain plane move off any object stays on the grid
        }
        let tilt = self.align_to_slope && normal.dot(Vec3::UP) > 0.05;
        let up = if tilt { normal } else { Vec3::UP };
        if tilt {
            let yaw_only =
                gizmo::rotation_from_euler(Vec3::new(0.0, start.rotation_degrees.y, 0.0));
            let rotation = gizmo::compose(&gizmo::align_up_to(normal), &yaw_only);
            result.rotation_degrees = gizmo::euler_from_rotation(&rotation);
        }
        let bottom = local_bottom(&item) * start.scale.y;
        // Grid-snap the spot on flat surfaces; the height always comes from the surface.
        let step = self.snap.translate;
        let flat = normal.y > 0.99;
        let spot = if flat && self.snap_target != SnapTarget::Increment && step > 0.0 {
            Vec3::new(
                (point.x / step).round() * step,
                point.y,
                (point.z / step).round() * step,
            )
        } else {
            point
        };
        result.position = spot - up * bottom;
        let slope = normal.dot(Vec3::UP).clamp(-1.0, 1.0).acos().to_degrees();
        result.summary = if slope > 0.5 {
            format!("na superfície · inclinação {slope:.0}°")
        } else {
            format!("na superfície · altura {:.2} m", point.y).replace('.', ",")
        };
    }

    /// Ends the drag: one undo step for the whole movement.
    pub fn gizmo_end(&mut self) -> Result<Json, Failure> {
        self.drag = None;
        self.hover = None;
        self.hud = None;
        self.free = None;
        self.gesture_commit()
    }

    pub fn gizmo_cancel(&mut self) -> Result<Json, Failure> {
        self.drag = None;
        self.hover = None;
        self.hud = None;
        self.free = None;
        self.gesture_cancel()
    }

    // ---- free mode (double tap) -------------------------------------------------------------

    /// Double tap on G, R or S: the mouse drives the value without grabbing a handle.
    pub fn free_begin(&mut self, tool: Tool, pointer: Pointer) -> Result<Json, Failure> {
        if !matches!(tool, Tool::Move | Tool::Rotate | Tool::Scale) {
            return Err(Failure::bad_request(
                "o modo livre existe para mover, girar e escalar",
            ));
        }
        if self.drag.is_some() {
            self.gizmo_cancel()?;
        }
        self.tool = tool;
        self.edit_pivot = false;
        self.free = Some(FreeMode::default());
        if !self.restart_free(pointer) {
            self.free = None;
            return Err(Failure::new(
                "EDITOR_NO_SELECTION",
                "o modo livre precisa de um objeto selecionado",
                "Selecione um objeto e toque duas vezes no atalho.",
            ));
        }
        Ok(json!({ "free": true }))
    }

    fn restart_free(&mut self, pointer: Pointer) -> bool {
        let axis = self.free.as_ref().and_then(|free| free.axis);
        let start_mouse = self
            .drag
            .map(|drag| drag.start_mouse())
            .unwrap_or(pointer.mouse());
        if self.drag.is_some() {
            let _ = self.gesture_cancel();
            self.drag = None;
        }
        let Some((_, pose, model)) = self.screen_gizmo(pointer) else {
            return false;
        };
        let handle = match (self.tool, axis) {
            (Tool::Move, Some(index)) => Handle::MoveAxis(index),
            (Tool::Move, None) => Handle::MoveScreen,
            (Tool::Rotate, Some(index)) => Handle::RotateAxis(index),
            (Tool::Rotate, None) => Handle::RotateView,
            (Tool::Scale, Some(index)) => Handle::ScaleAxis(index),
            _ => Handle::ScaleUniform,
        };
        let start = Pointer {
            x: start_mouse[0],
            y: start_mouse[1],
            ..pointer
        };
        self.start_drag(handle, &pose, &model, start)
    }

    /// X, Y or Z during free mode: constrain to that axis (the same key again frees it).
    pub fn free_axis(&mut self, axis: usize, pointer: Pointer) -> Result<Json, Failure> {
        let Some(free) = self.free.as_mut() else {
            return Ok(Json::Null);
        };
        free.axis = if free.axis == Some(axis) || axis > 2 {
            None
        } else {
            Some(axis)
        };
        if !self.restart_free(pointer) {
            return Err(Failure::bad_request("não foi possível restringir o eixo"));
        }
        self.gizmo_drag(pointer, false)
    }

    /// Typed text during free mode (digits, `-`, `,` or `.`); empty text returns to the mouse.
    pub fn free_type(&mut self, text: &str, pointer: Pointer) -> Result<Json, Failure> {
        let Some(free) = self.free.as_mut() else {
            return Ok(Json::Null);
        };
        free.typed = text
            .chars()
            .filter(|character| character.is_ascii_digit() || matches!(character, '-' | ',' | '.'))
            .take(16)
            .collect();
        self.gizmo_drag(pointer, false)
    }

    // ---- overlay ----------------------------------------------------------------------------

    /// World-space helpers the renderer draws: constant-width outlines of the selection and of
    /// the object under the mouse, and the face a face tool would work on.
    pub(crate) fn outline_overlay(&self, frame: &RenderFrame) -> Vec<OverlayItem> {
        let mut items = Vec::new();
        for item in &frame.items {
            let selected = Some(item.entity) == self.selection;
            let hovered = Some(item.entity) == self.hovered && !selected;
            if !(selected || hovered) {
                continue;
            }
            if let Some((low, high)) = item_bounds(item) {
                if selected {
                    box_lines(low, high, SELECTION_COLOR, 2.0, &mut items);
                } else {
                    box_lines(low, high, HOVER_COLOR, 1.5, &mut items);
                }
            }
        }
        if let Some(face) = self.face_drag.map(|drag| drag.face).or(self.hovered_face) {
            for (index, corner) in face.corners.iter().enumerate() {
                let next = face
                    .corners
                    .get((index + 1) % 4)
                    .copied()
                    .unwrap_or(*corner);
                items.push(OverlayItem::Line {
                    from: *corner,
                    to: next,
                    color: FACE_COLOR,
                    width_pixels: 3.0,
                });
            }
        }
        items
    }

    /// Everything the GUI paints on top of the 3D view, in logical pixels: the gizmo, the pivot
    /// marker, axis guides while dragging, the heads-up text, the floor and the camera axes.
    pub fn viewport_overlay(&self, width: f32, height: f32) -> Json {
        let pointer = Pointer {
            x: 0.0,
            y: 0.0,
            width,
            height,
        };
        let view = self.view_for(pointer);
        let camera = json!({
            "right": vector_json(view.right()),
            "up": vector_json(view.up()),
            "forward": vector_json(self.camera.camera.forward()),
        });
        if !view.is_valid() || self.project.is_none() {
            return json!({ "camera": camera, "gizmo": null });
        }
        let highlight = self.drag.map(|drag| drag.handle).or(self.hover);
        let model = self.screen_gizmo(pointer);
        let pivot = self
            .selection_pose()
            .and_then(|(_, pose)| view.project(pose.pivot))
            .map(|point| json!(point));

        let mut guides = Vec::new();
        if let (Some(drag), Some((_, pose))) = (self.drag, self.selection_pose()) {
            let axes = drag.axes();
            let guide_axes: Vec<usize> = match drag.handle {
                Handle::MoveAxis(index) | Handle::ScaleAxis(index) | Handle::RotateAxis(index) => {
                    vec![index]
                }
                Handle::MovePlane(normal) | Handle::ScalePlane(normal) => {
                    vec![(normal + 1) % 3, (normal + 2) % 3]
                }
                _ => Vec::new(),
            };
            for index in guide_axes {
                let axis = axes.get(index).copied().unwrap_or(Vec3::ZERO);
                for reach in [200.0f32, 50.0, 10.0] {
                    if let (Some(a), Some(b)) = (
                        view.project(pose.pivot - axis * reach),
                        view.project(pose.pivot + axis * reach),
                    ) {
                        guides.push(json!({ "from": a, "to": b, "axis": index }));
                        break;
                    }
                }
            }
        }
        let face_hud = self.face_drag.map(|_| {
            json!({
                "title": if self.tool == Tool::Extrude { "Extrudar" } else { "Empurrar / puxar" },
                "detail": self.hud.clone().unwrap_or_default(),
                "snap": "Ctrl inverte o snap · Esc cancela",
            })
        });
        let hud = self
            .drag
            .map(|drag| {
                let mut title = drag.handle.label();
                if self.edit_pivot {
                    title = format!("Editar pivô · {title}");
                }
                let free = self.free.as_ref();
                if free.is_some() {
                    title = format!("[MODO LIVRE] {title}");
                }
                let typed = free.map(|free| free.typed.clone()).unwrap_or_default();
                let note = if free.is_some() {
                    "X Y Z restringem · digite um valor · Enter confirma · Esc cancela".to_string()
                } else if self.snap.enabled {
                    format!("Snap: {} · Ctrl desliga", snap_label(self.snap_target))
                } else {
                    "Snap desligado · Ctrl liga".to_string()
                };
                json!({
                    "title": title,
                    "detail": if typed.is_empty() { self.hud.clone().unwrap_or_default() } else { format!("{typed}▏") },
                    "snap": note,
                })
            })
            .or(face_hud);
        json!({
            "camera": camera,
            "tool": self.tool.name(),
            "gizmo": model.map(|(_, _, model)| gizmo_json(&model, highlight)),
            "pivot": pivot,
            "edit_pivot": self.edit_pivot,
            "guides": guides,
            "hud": hud,
            "free": self.free.is_some(),
            "floor": self.floor_json(),
        })
    }

    // ---- views ------------------------------------------------------------------------------

    /// The selection, or else the whole scene: its center and how far to stand back to see it.
    fn view_target(&self) -> Result<(Vec3, f32), Failure> {
        let world = self.world()?;
        if let Some((id, pose)) = self.selection_pose() {
            let extent = match vec3_of(world, id, "Brush", "size") {
                Some(size) => (size.x * pose.scale.x)
                    .max(size.y * pose.scale.y)
                    .max(size.z * pose.scale.z),
                None => 1.0,
            };
            return Ok((pose.position, extent * 1.8 + 3.0));
        }
        let mut low = Vec3::splat(f32::MAX);
        let mut high = Vec3::splat(f32::MIN);
        for (item_low, item_high) in self.draw_items().iter().filter_map(item_bounds) {
            low = Vec3::new(
                low.x.min(item_low.x),
                low.y.min(item_low.y),
                low.z.min(item_low.z),
            );
            high = Vec3::new(
                high.x.max(item_high.x),
                high.y.max(item_high.y),
                high.z.max(item_high.z),
            );
        }
        if low.x > high.x {
            return Ok((Vec3::ZERO, 12.0));
        }
        let radius = (high - low).length() * 0.5;
        Ok(((low + high) * 0.5, radius * 1.3 + 3.0))
    }

    /// Looks at the selection (or the scene) from a standard direction.
    pub fn camera_view(&mut self, name: &str) -> Result<Json, Failure> {
        let (yaw, pitch) = match name {
            "front" => (0.0, 0.0),
            "back" => (180.0, 0.0),
            // Petunia3D convention: "right" looks at the +X face (camera on +X).
            "left" => (90.0, 0.0),
            "right" => (-90.0, 0.0),
            "top" => (0.0, -89.0),
            "bottom" => (0.0, 89.0),
            other => {
                return Err(Failure::bad_request(&format!(
                    "vista `{other}` desconhecida (front, back, left, right, top ou bottom)"
                )));
            }
        };
        self.look_from(yaw, pitch)
    }

    /// Looks at the target from a direction (target → eye).
    pub fn camera_view_direction(&mut self, direction: Vec3) -> Result<Json, Failure> {
        let direction = direction.normalized_or_zero();
        if direction.length_squared() == 0.0 {
            return Err(Failure::bad_request(
                "a direção precisa ser diferente de zero",
            ));
        }
        let forward = -direction;
        let pitch = forward
            .y
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees()
            .clamp(-89.0, 89.0);
        let yaw = forward.x.atan2(-forward.z).to_degrees();
        self.look_from(yaw, pitch)
    }

    /// Turns the camera around the target to an absolute heading (degrees), keeping the pitch.
    pub fn camera_yaw(&mut self, degrees: f32) -> Result<Json, Failure> {
        if !degrees.is_finite() {
            return Err(Failure::bad_request("o ângulo precisa ser um número"));
        }
        let pitch = self.camera.camera.pitch_degrees;
        self.look_from(degrees, pitch)
    }

    fn look_from(&mut self, yaw: f32, pitch: f32) -> Result<Json, Failure> {
        let (center, distance) = self.view_target()?;
        let camera = &mut self.camera.camera;
        camera.yaw_degrees = yaw;
        camera.pitch_degrees = pitch;
        camera.position = center - camera.forward() * distance;
        Ok(Json::Null)
    }

    pub(crate) fn transform_json(&self) -> Json {
        json!({
            "orientation": self.orientation.name(),
            "pivot": self.pivot_mode.name(),
            "edit_pivot": self.edit_pivot,
            "gizmo_size": self.gizmo_size,
        })
    }

    pub(crate) fn default_gizmo_size() -> f32 {
        ROD_LENGTH
    }
}
