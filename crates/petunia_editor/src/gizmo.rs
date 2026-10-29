//! Transform gizmos, ported from Petunia3D (`editor/src/projection.rs::compute_gizmo` and the hit
//! rules of `editor/src/bridge/gizmo.rs`).
//!
//! The gizmo lives in **screen space**: rods have a fixed length in pixels (72 by default), so the
//! handles are equally easy to grab at any distance. [`screen_gizmo`] builds the model the GUI
//! paints; [`hit_test`] uses the same model, so what is highlighted is exactly what a click grabs.
//! Dragging is solved in world space ([`begin_drag`], [`drag_to`]).
//!
//! | Tool | Handles |
//! |---|---|
//! | Select (universal) | rods with arrows (move), small boxes at 60 % (scale), outer rings (rotate); the center is a dead zone so a click selects |
//! | Move | rods with arrows, plane squares (YZ, XZ, XY), center = move on the screen plane |
//! | Rotate | rings with the camera-facing arc bright, outer view ring (roll), center = trackball |
//! | Scale | rods with boxes, plane squares, center = uniform scale |

use petunia_math::{Transform, Vec3};
use petunia_render::{Camera, Ray, model_matrix, ray_through};
use serde_json::{Value as Json, json};

/// Default rod length in logical pixels (Petunia3D `GIZMO_ROD_LENGTH`).
pub const ROD_LENGTH: f32 = 72.0;
/// View-roll ring radius over the rod length (Petunia3D `GIZMO_VIEW_ROLL_RATIO`).
const VIEW_ROLL_RATIO: f32 = 1.18;
/// Half width of the clickable band of rings: 24 px in total (WCAG 2.5.8).
const RING_HIT_HALF_WIDTH: f32 = 12.0;
/// Center handle radius, and the universal gizmo's dead zone.
const CENTER_RADIUS: f32 = 12.0;
/// How far from a rod the mouse may be and still grab it.
const ROD_HIT_PIXELS: f32 = 10.0;
const RING_POINTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Select plus the universal gizmo (move, rotate and scale handles at once).
    Select,
    Move,
    Rotate,
    Scale,
    /// Drag a face of a brush to resize it (Petunia3D `push_pull`, P). No gizmo.
    PushPull,
    /// Click a face to grow a new brush out of it, then drag (Petunia3D `extrude`, E). No gizmo.
    Extrude,
    /// Drag over a cave to shape it; the brush mode says how, Ctrl does the opposite (B). No gizmo.
    Sculpt,
    /// Click points on a cave and dig a smooth tunnel through them (Shift+B). No gizmo.
    SculptPath,
    /// Drag over the terrain to sculpt it (T). No gizmo.
    Terrain,
}

impl Tool {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "select" => Some(Self::Select),
            "move" => Some(Self::Move),
            "rotate" => Some(Self::Rotate),
            "scale" => Some(Self::Scale),
            "push_pull" => Some(Self::PushPull),
            "extrude" => Some(Self::Extrude),
            "sculpt" => Some(Self::Sculpt),
            "sculpt_path" => Some(Self::SculptPath),
            "terrain" => Some(Self::Terrain),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Move => "move",
            Self::Rotate => "rotate",
            Self::Scale => "scale",
            Self::PushPull => "push_pull",
            Self::Extrude => "extrude",
            Self::Sculpt => "sculpt",
            Self::SculptPath => "sculpt_path",
            Self::Terrain => "terrain",
        }
    }

    /// Tools that work on faces instead of showing a transform gizmo.
    pub fn works_on_faces(self) -> bool {
        matches!(self, Self::PushPull | Self::Extrude)
    }

    /// Tools that show the transform gizmo on the selection.
    pub fn has_gizmo(self) -> bool {
        !matches!(
            self,
            Self::PushPull | Self::Extrude | Self::Sculpt | Self::SculptPath | Self::Terrain
        )
    }

    /// Tools that work on caves with a brush ring instead of a gizmo.
    pub fn is_sculpting(self) -> bool {
        matches!(self, Self::Sculpt | Self::SculptPath)
    }

    /// Tools that work on the terrain heightmap with a brush ring.
    pub fn is_terrain(self) -> bool {
        matches!(self, Self::Terrain)
    }
}

/// Axes of the transform frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Global,
    Local,
}

impl Orientation {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "global" => Some(Self::Global),
            "local" => Some(Self::Local),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Local => "local",
        }
    }
}

/// One grabbable part. `usize` is the axis (0 = X, 1 = Y, 2 = Z); for planes it is the plane's
/// normal (0 = YZ, 1 = XZ, 2 = XY).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    MoveAxis(usize),
    MovePlane(usize),
    MoveScreen,
    RotateAxis(usize),
    RotateView,
    Trackball,
    ScaleAxis(usize),
    ScalePlane(usize),
    ScaleUniform,
}

const AXIS_NAMES: [&str; 3] = ["x", "y", "z"];
const PLANE_NAMES: [&str; 3] = ["yz", "xz", "xy"];

impl Handle {
    pub fn name(self) -> String {
        let axis = |index: usize| AXIS_NAMES.get(index).copied().unwrap_or("?");
        let plane = |index: usize| PLANE_NAMES.get(index).copied().unwrap_or("?");
        match self {
            Self::MoveAxis(index) => format!("move_{}", axis(index)),
            Self::MovePlane(index) => format!("move_{}", plane(index)),
            Self::MoveScreen => "move_screen".to_string(),
            Self::RotateAxis(index) => format!("rotate_{}", axis(index)),
            Self::RotateView => "rotate_view".to_string(),
            Self::Trackball => "trackball".to_string(),
            Self::ScaleAxis(index) => format!("scale_{}", axis(index)),
            Self::ScalePlane(index) => format!("scale_{}", plane(index)),
            Self::ScaleUniform => "scale_uniform".to_string(),
        }
    }

    /// Words for the heads-up display while dragging.
    pub fn label(self) -> String {
        let axis = |index: usize| AXIS_NAMES.get(index).copied().unwrap_or("?").to_uppercase();
        let plane = |index: usize| {
            PLANE_NAMES
                .get(index)
                .copied()
                .unwrap_or("?")
                .to_uppercase()
        };
        match self {
            Self::MoveAxis(index) => format!("Mover · eixo {}", axis(index)),
            Self::MovePlane(index) => format!("Mover · plano {}", plane(index)),
            Self::MoveScreen => "Mover · plano da tela".to_string(),
            Self::RotateAxis(index) => format!("Girar · eixo {}", axis(index)),
            Self::RotateView => "Girar · em torno da vista".to_string(),
            Self::Trackball => "Girar · livre".to_string(),
            Self::ScaleAxis(index) => format!("Escalar · eixo {}", axis(index)),
            Self::ScalePlane(index) => format!("Escalar · plano {}", plane(index)),
            Self::ScaleUniform => "Escalar · uniforme".to_string(),
        }
    }
}

/// Grid snapping. Moving snaps the **absolute** coordinate in the global frame (objects land on
/// the grid) and the distance in the local frame; rotation and scale snap the resulting value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub enabled: bool,
    /// Grid: moving lands on absolute multiples of the step. Increment: it moves BY steps from
    /// where it started (Petunia3D `SnapTarget::Increment`).
    pub absolute: bool,
    pub translate: f32,
    pub rotate_degrees: f32,
    pub scale: f32,
}

impl Default for Snap {
    fn default() -> Self {
        Self {
            enabled: true,
            absolute: true,
            translate: 0.5,
            rotate_degrees: 15.0,
            scale: 0.1,
        }
    }
}

fn snapped(value: f32, step: f32, enabled: bool) -> f32 {
    if enabled && step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

/// Where the object is, and the point the transform turns and scales around.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub scale: Vec3,
    pub pivot: Vec3,
}

/// The camera and the viewport size in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub width: f32,
    pub height: f32,
}

impl View {
    pub fn is_valid(&self) -> bool {
        self.width >= 1.0 && self.height >= 1.0 && self.width.is_finite() && self.height.is_finite()
    }

    fn aspect(&self) -> f32 {
        self.width / self.height
    }

    pub fn ray(&self, mouse: [f32; 2]) -> Option<Ray> {
        let ndc_x = mouse[0] / self.width * 2.0 - 1.0;
        let ndc_y = 1.0 - mouse[1] / self.height * 2.0;
        ray_through(&self.camera, self.aspect(), ndc_x, ndc_y)
    }

    /// World point to pixels (origin at the top-left, like the mouse). `None` behind the camera.
    pub fn project(&self, point: Vec3) -> Option<[f32; 2]> {
        let matrix = self.camera.view_projection(self.aspect());
        let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
        let clip = |row: usize| {
            entry(row) * point.x
                + entry(4 + row) * point.y
                + entry(8 + row) * point.z
                + entry(12 + row)
        };
        let w = clip(3);
        match self.camera.projection_mode {
            petunia_render::ProjectionMode::Perspective => {
                if w <= 0.05 {
                    return None;
                }
                Some([
                    (clip(0) / w * 0.5 + 0.5) * self.width,
                    (1.0 - (clip(1) / w * 0.5 + 0.5)) * self.height,
                ])
            }
            petunia_render::ProjectionMode::Orthographic => {
                let z_ndc = clip(2);
                if !(-1.0..=1.0).contains(&z_ndc) {
                    return None;
                }
                Some([
                    (clip(0) * 0.5 + 0.5) * self.width,
                    (1.0 - (clip(1) * 0.5 + 0.5)) * self.height,
                ])
            }
        }
    }

    pub fn right(&self) -> Vec3 {
        self.camera.right()
    }

    pub fn up(&self) -> Vec3 {
        self.camera
            .right()
            .cross(self.camera.forward())
            .normalized_or_zero()
    }

    /// Screen direction of a world direction, keeping foreshortening: an axis that points at the
    /// viewer shrinks instead of becoming a full-length diagonal (Petunia3D).
    fn screen_direction(&self, axis: Vec3) -> [f32; 2] {
        [axis.dot(self.right()), -axis.dot(self.up())]
    }
}

// ---- rotation helpers (our Euler order: roll Z, then pitch X, then yaw Y) ---------------------

/// 3x3 rotation, `[row][column]`.
pub type Matrix3 = [[f32; 3]; 3];

pub fn rotation_from_euler(degrees: Vec3) -> Matrix3 {
    let matrix = model_matrix(&Transform {
        position: Vec3::ZERO,
        rotation_degrees: degrees,
        scale: Vec3::ONE,
    });
    let entry = |row: usize, column: usize| matrix.get(column * 4 + row).copied().unwrap_or(0.0);
    [
        [entry(0, 0), entry(0, 1), entry(0, 2)],
        [entry(1, 0), entry(1, 1), entry(1, 2)],
        [entry(2, 0), entry(2, 1), entry(2, 2)],
    ]
}

/// Inverse of [`rotation_from_euler`]. Near pitch ±90° roll is set to zero (gimbal lock).
pub fn euler_from_rotation(matrix: &Matrix3) -> Vec3 {
    let [[m00, _m01, m02], [m10, m11, m12], [m20, _m21, m22]] = *matrix;
    let pitch = (-m12).clamp(-1.0, 1.0).asin();
    let (yaw, roll) = if pitch.cos().abs() > 1.0e-5 {
        (m02.atan2(m22), m10.atan2(m11))
    } else {
        ((-m20).atan2(m00), 0.0)
    };
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

fn multiply(a: &Matrix3, b: &Matrix3) -> Matrix3 {
    let mut out = [[0.0; 3]; 3];
    for (row, out_row) in out.iter_mut().enumerate() {
        for (column, cell) in out_row.iter_mut().enumerate() {
            *cell = (0..3)
                .map(|k| {
                    a.get(row).and_then(|r| r.get(k)).copied().unwrap_or(0.0)
                        * b.get(k).and_then(|r| r.get(column)).copied().unwrap_or(0.0)
                })
                .sum();
        }
    }
    out
}

fn apply(matrix: &Matrix3, vector: Vec3) -> Vec3 {
    let row = |index: usize| matrix.get(index).copied().unwrap_or([0.0; 3]);
    let dot = |r: [f32; 3]| r[0] * vector.x + r[1] * vector.y + r[2] * vector.z;
    Vec3::new(dot(row(0)), dot(row(1)), dot(row(2)))
}

fn transpose(matrix: &Matrix3) -> Matrix3 {
    let mut out = [[0.0; 3]; 3];
    for (row, out_row) in out.iter_mut().enumerate() {
        for (column, cell) in out_row.iter_mut().enumerate() {
            *cell = matrix
                .get(column)
                .and_then(|r| r.get(row))
                .copied()
                .unwrap_or(0.0);
        }
    }
    out
}

/// Right-handed rotation around a unit axis (Rodrigues).
fn axis_rotation(axis: Vec3, radians: f32) -> Matrix3 {
    let axis = axis.normalized_or_zero();
    let (sin, cos) = radians.sin_cos();
    let t = 1.0 - cos;
    let (x, y, z) = (axis.x, axis.y, axis.z);
    [
        [t * x * x + cos, t * x * y - sin * z, t * x * z + sin * y],
        [t * x * y + sin * z, t * y * y + cos, t * y * z - sin * x],
        [t * x * z - sin * y, t * y * z + sin * x, t * z * z + cos],
    ]
}

fn column(matrix: &Matrix3, index: usize) -> Vec3 {
    let at = |row: usize| {
        matrix
            .get(row)
            .and_then(|r| r.get(index))
            .copied()
            .unwrap_or(0.0)
    };
    Vec3::new(at(0), at(1), at(2))
}

fn component(vector: Vec3, index: usize) -> f32 {
    match index {
        0 => vector.x,
        1 => vector.y,
        _ => vector.z,
    }
}

fn set_component(vector: &mut Vec3, index: usize, value: f32) {
    match index {
        0 => vector.x = value,
        1 => vector.y = value,
        _ => vector.z = value,
    }
}

/// World directions of the frame's three axes.
pub fn frame_axes(pose: &Pose, orientation: Orientation) -> [Vec3; 3] {
    match orientation {
        Orientation::Global => [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        Orientation::Local => {
            let rotation = rotation_from_euler(pose.rotation_degrees);
            [
                column(&rotation, 0),
                column(&rotation, 1),
                column(&rotation, 2),
            ]
        }
    }
}

/// The frame a tool really uses: scale always works in the object's own axes (its `scale` is
/// local); move and rotate follow the chosen orientation.
fn tool_axes(
    tool: Tool,
    handle_is_scale: bool,
    pose: &Pose,
    orientation: Orientation,
) -> [Vec3; 3] {
    if tool == Tool::Scale || handle_is_scale {
        frame_axes(pose, Orientation::Local)
    } else {
        frame_axes(pose, orientation)
    }
}

// ---- screen model ------------------------------------------------------------------------------

/// One axis of the gizmo, in pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenAxis {
    pub end: [f32; 2],
    pub label: [f32; 2],
    pub rod: bool,
    pub arrow: Option<[[f32; 2]; 3]>,
    /// Center and half size of a scale box.
    pub scale_box: Option<([f32; 2], f32)>,
    pub ring: Vec<[f32; 2]>,
    /// Per ring point: true when that part of the ring faces the camera.
    pub ring_front: Vec<bool>,
}

/// The whole gizmo in pixels: what the GUI paints and what [`hit_test`] uses.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenGizmo {
    pub tool: Tool,
    pub origin: [f32; 2],
    pub rod_length: f32,
    pub axes: [ScreenAxis; 3],
    /// Plane squares by normal (0 = YZ, 1 = XZ, 2 = XY).
    pub planes: [Option<[[f32; 2]; 4]>; 3],
    pub view_ring_radius: Option<f32>,
    pub center: Option<Handle>,
}

pub fn screen_gizmo(
    tool: Tool,
    pose: &Pose,
    view: &View,
    orientation: Orientation,
    rod_length: f32,
) -> Option<ScreenGizmo> {
    if !view.is_valid() || !tool.has_gizmo() {
        return None;
    }
    let origin = view.project(pose.pivot)?;
    let length = if rod_length.is_finite() {
        rod_length.clamp(48.0, 160.0)
    } else {
        ROD_LENGTH
    };
    // Proportions from Petunia3D, derived from the rod length (72 px = default).
    let arrow_back = (length * 0.2).max(10.0);
    let arrow_half = (length * 0.07).max(4.0);
    let scale_center_at = length * 0.6;
    let box_half = (length * 0.07).max(4.0);
    let universal_ring = length * 1.26;
    let plane_near = length * 0.25;
    let plane_far = length * 0.44;

    let axes = tool_axes(tool, false, pose, orientation);
    let forward = view.camera.forward();
    let at = |direction: [f32; 2], distance: f32| {
        [
            origin[0] + direction[0] * distance,
            origin[1] + direction[1] * distance,
        ]
    };

    let mut screen_axes: [ScreenAxis; 3] = Default::default();
    for (index, slot) in screen_axes.iter_mut().enumerate() {
        let axis = axes.get(index).copied().unwrap_or(Vec3::ZERO);
        let direction = view.screen_direction(axis);
        slot.end = at(direction, length);
        slot.label = at(direction, length + 14.0);

        let ring_radius = match tool {
            Tool::Rotate => Some(length),
            Tool::Select => Some(universal_ring),
            Tool::Move
            | Tool::Scale
            | Tool::PushPull
            | Tool::Extrude
            | Tool::Sculpt
            | Tool::SculptPath
            | Tool::Terrain => None,
        };
        if let Some(radius) = ring_radius {
            let tangent = axes.get((index + 1) % 3).copied().unwrap_or(Vec3::ZERO);
            let bitangent = axes.get((index + 2) % 3).copied().unwrap_or(Vec3::ZERO);
            for segment in 0..=RING_POINTS {
                let angle = segment as f32 * std::f32::consts::TAU / RING_POINTS as f32;
                let point = tangent * angle.cos() + bitangent * angle.sin();
                slot.ring.push(at(view.screen_direction(point), radius));
                // A ring seen face-on has depth ~0 everywhere: count that as facing the camera,
                // or the whole ring would be drawn dim.
                slot.ring_front.push(-point.dot(forward) > -0.02);
            }
        }
        if tool == Tool::Rotate {
            continue;
        }
        slot.rod = true;
        match tool {
            Tool::Scale => slot.scale_box = Some((slot.end, arrow_half)),
            Tool::Select => {
                slot.scale_box = Some((at(direction, scale_center_at), box_half));
                slot.arrow = Some(arrow(slot.end, direction, arrow_back, arrow_half));
            }
            Tool::Move => slot.arrow = Some(arrow(slot.end, direction, arrow_back, arrow_half)),
            Tool::Rotate
            | Tool::PushPull
            | Tool::Extrude
            | Tool::Sculpt
            | Tool::SculptPath
            | Tool::Terrain => {}
        }
    }

    let mut planes = [None, None, None];
    if matches!(tool, Tool::Move | Tool::Scale) {
        for (normal, plane) in planes.iter_mut().enumerate() {
            let first =
                view.screen_direction(axes.get((normal + 1) % 3).copied().unwrap_or(Vec3::ZERO));
            let second =
                view.screen_direction(axes.get((normal + 2) % 3).copied().unwrap_or(Vec3::ZERO));
            let corner = |a: f32, b: f32| {
                [
                    origin[0] + first[0] * a + second[0] * b,
                    origin[1] + first[1] * a + second[1] * b,
                ]
            };
            *plane = Some([
                corner(plane_near, plane_near),
                corner(plane_far, plane_near),
                corner(plane_far, plane_far),
                corner(plane_near, plane_far),
            ]);
        }
    }

    Some(ScreenGizmo {
        tool,
        origin,
        rod_length: length,
        axes: screen_axes,
        planes,
        view_ring_radius: (tool == Tool::Rotate).then_some(length * VIEW_ROLL_RATIO),
        center: match tool {
            Tool::Move => Some(Handle::MoveScreen),
            Tool::Scale => Some(Handle::ScaleUniform),
            Tool::Rotate => Some(Handle::Trackball),
            Tool::Select
            | Tool::PushPull
            | Tool::Extrude
            | Tool::Sculpt
            | Tool::SculptPath
            | Tool::Terrain => None,
        },
    })
}

fn arrow(end: [f32; 2], direction: [f32; 2], back: f32, half: f32) -> [[f32; 2]; 3] {
    let length = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
    let unit = if length > 1.0e-4 {
        [direction[0] / length, direction[1] / length]
    } else {
        [0.0, 0.0]
    };
    let base = [end[0] - unit[0] * back, end[1] - unit[1] * back];
    let perpendicular = [-unit[1], unit[0]];
    [
        end,
        [
            base[0] + perpendicular[0] * half,
            base[1] + perpendicular[1] * half,
        ],
        [
            base[0] - perpendicular[0] * half,
            base[1] - perpendicular[1] * half,
        ],
    ]
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn distance_to_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let length_squared = abx * abx + aby * aby;
    let along = if length_squared < 1.0e-6 {
        0.0
    } else {
        (((point[0] - a[0]) * abx + (point[1] - a[1]) * aby) / length_squared).clamp(0.0, 1.0)
    };
    distance(point, [a[0] + abx * along, a[1] + aby * along])
}

fn inside_quad(point: [f32; 2], quad: &[[f32; 2]; 4]) -> bool {
    // Convex quad: the point is on the same side of every edge.
    let mut sign = 0.0f32;
    for (a, b) in quad.iter().zip(quad.iter().cycle().skip(1)) {
        let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
        if cross.abs() < 1.0e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    sign != 0.0
}

/// Which handle is under the mouse (the Petunia3D rules, in this order: dead zone or center,
/// view ring, plane squares, then the nearest rod, box or ring).
pub fn hit_test(gizmo: &ScreenGizmo, mouse: [f32; 2]) -> Option<Handle> {
    let from_origin = distance(mouse, gizmo.origin);
    if from_origin <= CENTER_RADIUS {
        return gizmo.center; // `None` for the universal gizmo: the center is a dead zone
    }
    for (normal, plane) in gizmo.planes.iter().enumerate() {
        if let Some(quad) = plane
            && inside_quad(mouse, quad)
        {
            return Some(if gizmo.tool == Tool::Scale {
                Handle::ScalePlane(normal)
            } else {
                Handle::MovePlane(normal)
            });
        }
    }

    let mut best: Option<(Handle, f32)> = None;
    let mut consider = |handle: Handle, gap: f32, limit: f32| {
        if gap <= limit && best.is_none_or(|(_, nearest)| gap < nearest) {
            best = Some((handle, gap));
        }
    };
    // Improvement over Petunia3D, which tested the view ring first: its band (radius 85 ± 12)
    // overlaps the axis rings' band (72 ± 12), so the outer half of every axis ring was stolen.
    // Here the nearest ring wins.
    if let Some(radius) = gizmo.view_ring_radius {
        consider(
            Handle::RotateView,
            (from_origin - radius).abs(),
            RING_HIT_HALF_WIDTH,
        );
    }
    for (index, axis) in gizmo.axes.iter().enumerate() {
        if let Some((center, half)) = axis.scale_box {
            // Boxes win a little over the rod they sit on.
            consider(
                Handle::ScaleAxis(index),
                (distance(mouse, center) - half).max(0.0) * 0.5,
                half + 4.0,
            );
        }
        if axis.rod {
            let handle = if gizmo.tool == Tool::Scale {
                Handle::ScaleAxis(index)
            } else {
                Handle::MoveAxis(index)
            };
            consider(
                handle,
                distance_to_segment(mouse, gizmo.origin, axis.end),
                ROD_HIT_PIXELS,
            );
        }
        for pair in axis.ring.windows(2) {
            if let [a, b] = pair {
                consider(
                    Handle::RotateAxis(index),
                    distance_to_segment(mouse, *a, *b),
                    RING_HIT_HALF_WIDTH,
                );
            }
        }
    }
    best.map(|(handle, _)| handle)
}

// ---- dragging ----------------------------------------------------------------------------------

/// A drag in progress. Everything needed to compute values is captured when it starts.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub handle: Handle,
    pub orientation: Orientation,
    start: Pose,
    axes: [Vec3; 3],
    /// Axis parameter, plane hit, or angle at the start of the drag.
    start_point: Vec3,
    start_value: f32,
    start_mouse: [f32; 2],
    start_screen_origin: [f32; 2],
}

impl Drag {
    /// The pose when the drag started (the gizmo stays at its pivot while rotating or scaling).
    pub fn start_pose(&self) -> Pose {
        self.start
    }

    /// Where the mouse was when the drag started (free mode rebuilds the drag from here).
    pub fn start_mouse(&self) -> [f32; 2] {
        self.start_mouse
    }

    /// The frame axes captured at the start (for guides).
    pub fn axes(&self) -> [Vec3; 3] {
        self.axes
    }
}

/// What a drag produces: the new transform values plus a short text for the heads-up display.
#[derive(Clone, Debug, PartialEq)]
pub struct DragResult {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub scale: Vec3,
    pub summary: String,
}

/// Parameter of the point on the line `origin + axis * t` that is closest to the mouse ray.
fn closest_on_axis(ray: &Ray, origin: Vec3, axis: Vec3) -> Option<f32> {
    let offset = origin - ray.origin;
    let b = axis.dot(ray.direction);
    let denominator = 1.0 - b * b;
    if denominator.abs() < 1.0e-4 {
        return None; // the axis points at the camera: no stable answer
    }
    let d = axis.dot(offset);
    let e = ray.direction.dot(offset);
    Some((b * e - d) / denominator)
}

fn hit_plane(ray: &Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let facing = ray.direction.dot(normal);
    if facing.abs() < 1.0e-3 {
        return None;
    }
    let distance = (origin - ray.origin).dot(normal) / facing;
    (distance > 0.0).then(|| ray.origin + ray.direction * distance)
}

/// Two unit vectors spanning the plane perpendicular to `normal`, right-handed: `u × v = n`.
fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let helper = if normal.y.abs() < 0.9 {
        Vec3::UP
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let u = normal.cross(helper).normalized_or_zero();
    (u, normal.cross(u).normalized_or_zero())
}

fn angle_on_plane(ray: &Ray, origin: Vec3, axis: Vec3) -> Option<f32> {
    let hit = hit_plane(ray, origin, axis)? - origin;
    let (u, v) = plane_basis(axis);
    Some(hit.dot(v).atan2(hit.dot(u)))
}

fn wrap_angle(mut radians: f32) -> f32 {
    while radians > std::f32::consts::PI {
        radians -= std::f32::consts::TAU;
    }
    while radians < -std::f32::consts::PI {
        radians += std::f32::consts::TAU;
    }
    radians
}

pub fn begin_drag(
    handle: Handle,
    gizmo: &ScreenGizmo,
    pose: &Pose,
    view: &View,
    orientation: Orientation,
    mouse: [f32; 2],
) -> Option<Drag> {
    let ray = view.ray(mouse)?;
    let is_scale = matches!(
        handle,
        Handle::ScaleAxis(_) | Handle::ScalePlane(_) | Handle::ScaleUniform
    );
    let axes = tool_axes(gizmo.tool, is_scale, pose, orientation);
    let axis = |index: usize| axes.get(index).copied().unwrap_or(Vec3::ZERO);
    let (start_point, start_value) = match handle {
        Handle::MoveAxis(index) => (Vec3::ZERO, closest_on_axis(&ray, pose.pivot, axis(index))?),
        Handle::ScaleAxis(index) => {
            let parameter = closest_on_axis(&ray, pose.pivot, axis(index))?;
            // Keep the start away from zero so the ratio stays well behaved.
            let minimum = (pose.pivot - view.camera.position).length() * 0.02;
            let parameter = if parameter.abs() < minimum {
                minimum.copysign(parameter)
            } else {
                parameter
            };
            (Vec3::ZERO, parameter)
        }
        Handle::MovePlane(normal) | Handle::ScalePlane(normal) => {
            (hit_plane(&ray, pose.pivot, axis(normal))?, 0.0)
        }
        Handle::MoveScreen => (hit_plane(&ray, pose.pivot, view.camera.forward())?, 0.0),
        Handle::RotateAxis(index) => (Vec3::ZERO, angle_on_plane(&ray, pose.pivot, axis(index))?),
        Handle::RotateView | Handle::Trackball | Handle::ScaleUniform => (Vec3::ZERO, 0.0),
    };
    Some(Drag {
        handle,
        orientation,
        start: *pose,
        axes,
        start_point,
        start_value,
        start_mouse: mouse,
        start_screen_origin: gizmo.origin,
    })
}

fn format_meters(value: f32) -> String {
    format!("{value:+.2} m").replace('.', ",")
}

fn format_degrees(value: f32) -> String {
    format!("{value:+.1}°").replace('.', ",")
}

fn format_factor(value: f32) -> String {
    format!("×{value:.2}").replace('.', ",")
}

/// The transform for the mouse position. `None` when the ray gives no stable answer (keep the
/// last value).
pub fn drag_to(drag: &Drag, view: &View, mouse: [f32; 2], snap: &Snap) -> Option<DragResult> {
    let ray = view.ray(mouse)?;
    let start = drag.start;
    let pivot = start.pivot;
    let axis = |index: usize| drag.axes.get(index).copied().unwrap_or(Vec3::ZERO);
    // Absolute (grid) snapping only makes sense in world axes; otherwise snap the distance.
    let global = drag.orientation == Orientation::Global && snap.absolute;
    let mut result = DragResult {
        position: start.position,
        rotation_degrees: start.rotation_degrees,
        scale: start.scale,
        summary: String::new(),
    };

    // Move: snap the absolute coordinate in the global frame, the distance in the local one.
    let snap_move = |position: &mut Vec3, moved: Vec3, used_axes: &[usize]| {
        *position = start.position + moved;
        for &index in used_axes {
            if global {
                let value = snapped(component(*position, index), snap.translate, snap.enabled);
                set_component(position, index, value);
            }
        }
    };

    match drag.handle {
        Handle::MoveAxis(index) => {
            let delta = closest_on_axis(&ray, pivot, axis(index))? - drag.start_value;
            let delta = if global {
                delta
            } else {
                snapped(delta, snap.translate, snap.enabled)
            };
            snap_move(&mut result.position, axis(index) * delta, &[index]);
            let moved = result.position - start.position;
            result.summary = format_meters(moved.dot(axis(index)));
        }
        Handle::MovePlane(normal) => {
            let hit = hit_plane(&ray, pivot, axis(normal))?;
            let mut moved = hit - drag.start_point;
            moved = moved - axis(normal) * moved.dot(axis(normal));
            let used = [(normal + 1) % 3, (normal + 2) % 3];
            if !global {
                let mut local = Vec3::ZERO;
                for &index in &used {
                    local = local
                        + axis(index)
                            * snapped(moved.dot(axis(index)), snap.translate, snap.enabled);
                }
                moved = local;
            }
            snap_move(&mut result.position, moved, &used);
            let moved = result.position - start.position;
            result.summary = format!(
                "{} · {}",
                format_meters(moved.dot(axis(used[0]))),
                format_meters(moved.dot(axis(used[1])))
            );
        }
        Handle::MoveScreen => {
            let hit = hit_plane(&ray, pivot, view.camera.forward())?;
            snap_move(&mut result.position, hit - drag.start_point, &[0, 1, 2]);
            let moved = result.position - start.position;
            result.summary = format!("{} m", format!("{:.2}", moved.length()).replace('.', ","));
        }
        Handle::RotateAxis(_) | Handle::RotateView | Handle::Trackball => {
            let turn = match drag.handle {
                Handle::RotateAxis(index) => {
                    let delta =
                        wrap_angle(angle_on_plane(&ray, pivot, axis(index))? - drag.start_value);
                    let degrees = snapped(delta.to_degrees(), snap.rotate_degrees, snap.enabled);
                    result.summary = format_degrees(degrees);
                    axis_rotation(axis(index), degrees.to_radians())
                }
                Handle::RotateView => {
                    let angle = |point: [f32; 2]| {
                        // Screen y grows downward: negate so counter-clockwise is positive.
                        (-(point[1] - drag.start_screen_origin[1]))
                            .atan2(point[0] - drag.start_screen_origin[0])
                    };
                    let delta = wrap_angle(angle(mouse) - angle(drag.start_mouse));
                    let degrees = snapped(delta.to_degrees(), snap.rotate_degrees, snap.enabled);
                    result.summary = format_degrees(degrees);
                    axis_rotation(-view.camera.forward(), degrees.to_radians())
                }
                _ => {
                    let dx = (mouse[0] - drag.start_mouse[0]) * 0.5;
                    let dy = (mouse[1] - drag.start_mouse[1]) * 0.5;
                    result.summary = format!("{} · {}", format_degrees(dx), format_degrees(dy));
                    multiply(
                        &axis_rotation(view.up(), dx.to_radians()),
                        &axis_rotation(view.right(), dy.to_radians()),
                    )
                }
            };
            let rotated = multiply(&turn, &rotation_from_euler(start.rotation_degrees));
            result.rotation_degrees = euler_from_rotation(&rotated);
            result.position = pivot + apply(&turn, start.position - pivot);
        }
        Handle::ScaleAxis(_) | Handle::ScalePlane(_) | Handle::ScaleUniform => {
            let mut factors = Vec3::ONE;
            match drag.handle {
                Handle::ScaleAxis(index) => {
                    let factor = closest_on_axis(&ray, pivot, axis(index))? / drag.start_value;
                    set_component(&mut factors, index, factor);
                }
                Handle::ScalePlane(normal) => {
                    let hit = hit_plane(&ray, pivot, axis(normal))?;
                    let reach = (drag.start_point - pivot).length().max(1.0e-3);
                    let factor = (hit - pivot).length() / reach;
                    set_component(&mut factors, (normal + 1) % 3, factor);
                    set_component(&mut factors, (normal + 2) % 3, factor);
                }
                _ => {
                    let factor = ((drag.start_mouse[1] - mouse[1]) / 200.0).exp();
                    factors = Vec3::splat(factor);
                }
            }
            let minimum = snap.scale.max(0.01);
            let mut scale = start.scale;
            for index in 0..3 {
                let value = component(start.scale, index) * component(factors, index);
                let value = snapped(value, snap.scale, snap.enabled)
                    .max(minimum)
                    .clamp(0.01, 1000.0);
                set_component(&mut scale, index, value);
            }
            // Scale around the pivot, in the object's own axes.
            let rotation = rotation_from_euler(start.rotation_degrees);
            let ratio = Vec3::new(
                scale.x / start.scale.x.max(1.0e-6),
                scale.y / start.scale.y.max(1.0e-6),
                scale.z / start.scale.z.max(1.0e-6),
            );
            let local = apply(&transpose(&rotation), start.position - pivot);
            let stretched = Vec3::new(local.x * ratio.x, local.y * ratio.y, local.z * ratio.z);
            result.position = pivot + apply(&rotation, stretched);
            result.scale = scale;
            result.summary = format!(
                "{} · {} · {}",
                format_factor(ratio.x),
                format_factor(ratio.y),
                format_factor(ratio.z)
            );
        }
    }
    let finite = result.position.is_finite()
        && result.rotation_degrees.is_finite()
        && result.scale.is_finite();
    finite.then_some(result)
}

/// The transform for a typed value (free mode, Petunia3D "entrada numérica"): meters along the
/// constrained axis (X when moving freely), degrees around the axis (the view when free), or a
/// scale factor.
pub fn drag_typed(drag: &Drag, view: &View, value: f32) -> Option<DragResult> {
    if !value.is_finite() {
        return None;
    }
    let start = drag.start;
    let pivot = start.pivot;
    let axis = |index: usize| drag.axes.get(index).copied().unwrap_or(Vec3::ZERO);
    let mut result = DragResult {
        position: start.position,
        rotation_degrees: start.rotation_degrees,
        scale: start.scale,
        summary: String::new(),
    };
    match drag.handle {
        Handle::MoveAxis(_) | Handle::MoveScreen | Handle::MovePlane(_) => {
            let direction = match drag.handle {
                Handle::MoveAxis(index) => axis(index),
                _ => axis(0),
            };
            result.position = start.position + direction * value;
            result.summary = format_meters(value);
        }
        Handle::RotateAxis(_) | Handle::RotateView | Handle::Trackball => {
            let around = match drag.handle {
                Handle::RotateAxis(index) => axis(index),
                _ => -view.camera.forward(),
            };
            let turn = axis_rotation(around, value.to_radians());
            let rotated = multiply(&turn, &rotation_from_euler(start.rotation_degrees));
            result.rotation_degrees = euler_from_rotation(&rotated);
            result.position = pivot + apply(&turn, start.position - pivot);
            result.summary = format_degrees(value);
        }
        Handle::ScaleAxis(_) | Handle::ScalePlane(_) | Handle::ScaleUniform => {
            let factor = value.clamp(0.001, 1000.0);
            let mut factors = Vec3::splat(factor);
            if let Handle::ScaleAxis(index) = drag.handle {
                factors = Vec3::ONE;
                set_component(&mut factors, index, factor);
            }
            result.scale = Vec3::new(
                (start.scale.x * factors.x).clamp(0.01, 1000.0),
                (start.scale.y * factors.y).clamp(0.01, 1000.0),
                (start.scale.z * factors.z).clamp(0.01, 1000.0),
            );
            let rotation = rotation_from_euler(start.rotation_degrees);
            let local = apply(&transpose(&rotation), start.position - pivot);
            let stretched = Vec3::new(
                local.x * factors.x,
                local.y * factors.y,
                local.z * factors.z,
            );
            result.position = pivot + apply(&rotation, stretched);
            result.summary = format_factor(factor);
        }
    }
    Some(result)
}

/// Rotation that turns the world up axis onto `normal` (shortest arc).
pub fn align_up_to(normal: Vec3) -> Matrix3 {
    let normal = normal.normalized_or_zero();
    let axis = Vec3::UP.cross(normal);
    let sin = axis.length();
    let cos = Vec3::UP.dot(normal);
    if sin < 1.0e-6 {
        return if cos > 0.0 {
            axis_rotation(Vec3::new(1.0, 0.0, 0.0), 0.0)
        } else {
            axis_rotation(Vec3::new(1.0, 0.0, 0.0), std::f32::consts::PI)
        };
    }
    axis_rotation(axis * (1.0 / sin), sin.atan2(cos))
}

pub fn compose(a: &Matrix3, b: &Matrix3) -> Matrix3 {
    multiply(a, b)
}

pub fn rotate_vector(matrix: &Matrix3, vector: Vec3) -> Vec3 {
    apply(matrix, vector)
}

pub fn inverse_rotate(matrix: &Matrix3, vector: Vec3) -> Vec3 {
    apply(&transpose(matrix), vector)
}

// ---- JSON for the GUI ----------------------------------------------------------------------------

fn point_json(point: [f32; 2]) -> Json {
    json!([point[0], point[1]])
}

/// The screen model as plain JSON for the painter.
pub fn gizmo_json(gizmo: &ScreenGizmo, highlight: Option<Handle>) -> Json {
    let axes: Vec<Json> = gizmo
        .axes
        .iter()
        .map(|axis| {
            json!({
                "end": point_json(axis.end),
                "label": point_json(axis.label),
                "rod": axis.rod,
                "arrow": axis.arrow.map(|corners| corners.iter().map(|c| point_json(*c)).collect::<Vec<_>>()),
                "scale_box": axis.scale_box.map(|(center, half)| json!({ "center": point_json(center), "half": half })),
                "ring": axis.ring.iter().map(|p| point_json(*p)).collect::<Vec<_>>(),
                "ring_front": axis.ring_front,
            })
        })
        .collect();
    let planes: Vec<Json> = gizmo
        .planes
        .iter()
        .map(|plane| match plane {
            Some(quad) => Json::Array(quad.iter().map(|c| point_json(*c)).collect()),
            None => Json::Null,
        })
        .collect();
    json!({
        "tool": gizmo.tool.name(),
        "origin": point_json(gizmo.origin),
        "rod_length": gizmo.rod_length,
        "axes": axes,
        "planes": planes,
        "view_ring_radius": gizmo.view_ring_radius,
        "center": gizmo.center.map(Handle::name),
        "center_radius": CENTER_RADIUS,
        "highlight": highlight.map(Handle::name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View {
            camera: Camera {
                position: Vec3::new(4.0, 3.0, 10.0),
                yaw_degrees: -20.0,
                pitch_degrees: -15.0,
                ..Camera::default()
            },
            width: 1000.0,
            height: 800.0,
        }
    }

    fn pose() -> Pose {
        Pose {
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
            pivot: Vec3::ZERO,
        }
    }

    fn middle(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
        [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
    }

    #[test]
    fn each_tool_has_its_own_shape() {
        let (view, pose) = (view(), pose());
        let model = |tool| {
            screen_gizmo(tool, &pose, &view, Orientation::Global, ROD_LENGTH).expect("on screen")
        };
        let movement = model(Tool::Move);
        assert!(
            movement.axes.iter().all(|axis| axis.arrow.is_some()
                && axis.scale_box.is_none()
                && axis.ring.is_empty())
        );
        assert!(movement.planes.iter().all(Option::is_some));
        let scale = model(Tool::Scale);
        assert!(
            scale
                .axes
                .iter()
                .all(|axis| axis.arrow.is_none() && axis.scale_box.is_some())
        );
        let rotate = model(Tool::Rotate);
        assert!(
            rotate
                .axes
                .iter()
                .all(|axis| !axis.rod && axis.ring.len() == RING_POINTS + 1)
        );
        assert!(rotate.view_ring_radius.is_some() && rotate.planes.iter().all(Option::is_none));
        let universal = model(Tool::Select);
        assert!(
            universal.axes.iter().all(|axis| axis.arrow.is_some()
                && axis.scale_box.is_some()
                && !axis.ring.is_empty())
        );
        assert_eq!(
            universal.center, None,
            "the universal center is a dead zone"
        );
        // A ring has a bright (facing) part and a dim one.
        assert!(rotate.axes[1].ring_front.iter().any(|front| *front));
        assert!(rotate.axes[1].ring_front.iter().any(|front| !*front));
    }

    #[test]
    fn hit_rules_follow_petunia3d() {
        let (view, pose) = (view(), pose());
        let movement =
            screen_gizmo(Tool::Move, &pose, &view, Orientation::Global, ROD_LENGTH).expect("model");
        assert_eq!(
            hit_test(&movement, movement.origin),
            Some(Handle::MoveScreen)
        );
        let x_rod = middle(movement.origin, movement.axes[0].end);
        assert_eq!(hit_test(&movement, x_rod), Some(Handle::MoveAxis(0)));
        let xz = movement.planes[1].expect("plane");
        let inside = middle(xz[0], xz[2]);
        assert_eq!(hit_test(&movement, inside), Some(Handle::MovePlane(1)));
        assert_eq!(hit_test(&movement, [5.0, 5.0]), None);

        let universal = screen_gizmo(Tool::Select, &pose, &view, Orientation::Global, ROD_LENGTH)
            .expect("model");
        assert_eq!(hit_test(&universal, universal.origin), None);
        let (box_center, _) = universal.axes[2].scale_box.expect("box");
        assert_eq!(hit_test(&universal, box_center), Some(Handle::ScaleAxis(2)));

        let rotate = screen_gizmo(Tool::Rotate, &pose, &view, Orientation::Global, ROD_LENGTH)
            .expect("model");
        let radius = rotate.view_ring_radius.expect("view ring");
        let on_view_ring = [rotate.origin[0] + radius, rotate.origin[1]];
        assert_eq!(hit_test(&rotate, on_view_ring), Some(Handle::RotateView));
        // Between the bands, the nearer ring wins (Petunia3D gave it to the view ring).
        let z_ring_top = rotate.axes[2].ring[RING_POINTS / 4];
        let toward_view = [
            z_ring_top[0] + (z_ring_top[0] - rotate.origin[0]) * 0.04,
            z_ring_top[1] + (z_ring_top[1] - rotate.origin[1]) * 0.04,
        ];
        // Some axis ring wins there (which one depends on the camera); never the view ring.
        assert!(matches!(
            hit_test(&rotate, toward_view),
            Some(Handle::RotateAxis(_))
        ));
        assert_eq!(hit_test(&rotate, rotate.origin), Some(Handle::Trackball));
    }

    #[test]
    fn moving_on_the_x_axis_changes_only_x_and_lands_on_the_grid() {
        let (view, pose) = (view(), pose());
        let model =
            screen_gizmo(Tool::Move, &pose, &view, Orientation::Global, ROD_LENGTH).expect("model");
        let start = middle(model.origin, model.axes[0].end);
        let drag = begin_drag(
            Handle::MoveAxis(0),
            &model,
            &pose,
            &view,
            Orientation::Global,
            start,
        )
        .expect("drag");
        // Aim at the point 2.3 m further along the axis than where the drag started.
        let mouse = view
            .project(Vec3::new(drag.start_value + 2.3, 0.0, 0.0))
            .expect("on screen");
        let result = drag_to(&drag, &view, mouse, &Snap::default()).expect("result");
        assert!(
            (result.position.x - 2.5).abs() < 1.0e-3,
            "{:?}",
            result.position
        );
        assert!(result.position.y.abs() < 1.0e-4 && result.position.z.abs() < 1.0e-4);
        assert_eq!(result.rotation_degrees, Vec3::ZERO);
    }

    #[test]
    fn rotating_around_a_pivot_moves_the_object_around_it() {
        let view = View {
            camera: Camera {
                position: Vec3::new(0.0, 10.0, 0.01),
                pitch_degrees: -89.0,
                ..Camera::default()
            },
            width: 1000.0,
            height: 800.0,
        };
        // Object at x = 2 turning around the world origin.
        let pose = Pose {
            position: Vec3::new(2.0, 0.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
            pivot: Vec3::ZERO,
        };
        let model = screen_gizmo(Tool::Rotate, &pose, &view, Orientation::Global, ROD_LENGTH)
            .expect("model");
        let ring = &model.axes[1].ring;
        let drag = begin_drag(
            Handle::RotateAxis(1),
            &model,
            &pose,
            &view,
            Orientation::Global,
            ring[0],
        )
        .expect("drag");
        let quarter = ring[RING_POINTS / 4];
        let result = drag_to(&drag, &view, quarter, &Snap::default()).expect("result");
        assert!(
            (result.rotation_degrees.y.abs() - 90.0).abs() < 1.0e-2,
            "{:?}",
            result.rotation_degrees
        );
        // Still two meters from the pivot, now on the z axis.
        assert!((result.position.length() - 2.0).abs() < 1.0e-3);
        assert!(result.position.x.abs() < 1.0e-3 && (result.position.z.abs() - 2.0).abs() < 1.0e-3);
    }

    #[test]
    fn euler_angles_survive_a_round_trip_through_a_matrix() {
        for degrees in [
            Vec3::new(10.0, 20.0, 30.0),
            Vec3::new(-45.0, 170.0, -60.0),
            Vec3::new(0.0, -90.0, 0.0),
        ] {
            let back = euler_from_rotation(&rotation_from_euler(degrees));
            let again = rotation_from_euler(back);
            let original = rotation_from_euler(degrees);
            for row in 0..3 {
                for col in 0..3 {
                    assert!(
                        (again[row][col] - original[row][col]).abs() < 1.0e-4,
                        "{degrees:?} -> {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn scaling_keeps_a_minimum_and_scales_around_the_base_pivot() {
        let view = view();
        // A crate whose pivot is its base: scaling up must keep the base on the floor.
        let pose = Pose {
            position: Vec3::new(0.0, 1.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
            pivot: Vec3::ZERO,
        };
        let model = screen_gizmo(Tool::Scale, &pose, &view, Orientation::Global, ROD_LENGTH)
            .expect("model");
        let drag = begin_drag(
            Handle::ScaleUniform,
            &model,
            &pose,
            &view,
            Orientation::Global,
            model.origin,
        )
        .expect("drag");
        let bigger = drag_to(
            &drag,
            &view,
            [model.origin[0], model.origin[1] - 139.0],
            &Snap::default(),
        )
        .expect("result");
        assert!((bigger.scale.x - 2.0).abs() < 1.0e-3, "{:?}", bigger.scale);
        assert!(
            (bigger.position.y - 2.0).abs() < 1.0e-3,
            "the base stays at y = 0"
        );
        let tiny = drag_to(
            &drag,
            &view,
            [model.origin[0], model.origin[1] + 5000.0],
            &Snap::default(),
        )
        .expect("result");
        assert!(tiny.scale.x >= 0.1 - 1.0e-6);
    }
}
