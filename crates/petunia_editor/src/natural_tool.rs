//! Terrain, river and road tools in the editor.
//!
//! Terrain painting accumulates brush stamps during a drag and applies them as ONE
//! `terrain.paint` command on release. Rivers and roads are drawn point by point and committed
//! as one `path.create` command. Both use the same click surface the other tools use.

use petunia_commands::CommandRecord;
use petunia_math::{Color, CubicBezierSpline, SplinePoint, Vec3};
use petunia_render::OverlayItem;
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::Tool;
use crate::session::EditorSession;
use crate::tools::Pointer;

const TERRAIN_RING_COLOR: Color = Color::new(1.0, 0.82, 0.25, 1.0);
const RIVER_COLOR: Color = Color::new(0.35, 0.85, 1.0, 1.0);
const ROAD_COLOR: Color = Color::new(0.95, 0.55, 0.25, 1.0);
const STAMP_SPACING_RATIO: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerrainPaintMode {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Erode,
}

impl TerrainPaintMode {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "raise" | "up" => Some(Self::Raise),
            "lower" | "down" => Some(Self::Lower),
            "smooth" => Some(Self::Smooth),
            "flatten" | "level" => Some(Self::Flatten),
            "erode" => Some(Self::Erode),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Raise => "raise",
            Self::Lower => "lower",
            Self::Smooth => "smooth",
            Self::Flatten => "flatten",
            Self::Erode => "erode",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NaturalPathKind {
    River,
    Road,
}

impl NaturalPathKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::River => "river",
            Self::Road => "road",
        }
    }
}

/// Per-session state of the natural-world tools.
pub(crate) struct NaturalState {
    pub terrain: Option<u64>,
    pub mode: TerrainPaintMode,
    pub radius: f32,
    pub strength: f32,
    pub path_kind: Option<NaturalPathKind>,
    pub path_width: f32,
    pub path_depth: f32,
    pub path_points: Vec<Vec3>,
    pub hover: Option<Vec3>,
    pub stamps: Vec<Json>,
    pub last_stamp: Option<Vec3>,
    pub dragging: bool,
}

impl Default for NaturalState {
    fn default() -> Self {
        Self {
            terrain: None,
            mode: TerrainPaintMode::Raise,
            radius: 4.0,
            strength: 0.5,
            path_kind: None,
            path_width: 6.0,
            path_depth: 1.0,
            path_points: Vec::new(),
            hover: None,
            stamps: Vec::new(),
            last_stamp: None,
            dragging: false,
        }
    }
}

impl NaturalState {
    pub(crate) fn reset_transient(&mut self) {
        self.stamps.clear();
        self.last_stamp = None;
        self.dragging = false;
        self.path_points.clear();
        self.hover = None;
    }
}

fn smooth_spline(points: &[Vec3], width: f32, depth: f32) -> CubicBezierSpline {
    let mut spline = CubicBezierSpline::new();
    for (index, position) in points.iter().enumerate() {
        let before = points
            .get(index.wrapping_sub(1))
            .copied()
            .unwrap_or(*position);
        let after = points.get(index + 1).copied().unwrap_or(*position);
        let tangent = (after - before).normalized_or_zero();
        let before_scale = (*position - before).length().min(8.0) / 3.0;
        let after_scale = (after - *position).length().min(8.0) / 3.0;
        spline.points.push(SplinePoint {
            position: *position,
            handle_in: tangent * -before_scale,
            handle_out: tangent * after_scale,
            width,
            depth,
            extra: 0.0,
        });
    }
    spline
}

impl EditorSession {
    fn terrain_owner(&self, entity: u64) -> Option<u64> {
        let world = self.world().ok()?;
        let handle = world.handle_of(PersistentId(entity)).ok()?;
        match world.get(handle, "TerrainChunk", "owner") {
            Ok(Value::Int(owner)) if *owner > 0 => Some(*owner as u64),
            _ => None,
        }
    }

    fn first_terrain(&self) -> Option<u64> {
        let world = self.world().ok()?;
        world
            .entities_with_table("Terrain")
            .ok()?
            .into_iter()
            .find_map(|handle| world.persistent_id(handle).ok().map(|id| id.0))
    }

    /// The terrain that a paint stroke should use: the chunk under the cursor, the selected
    /// terrain, or the first one in the scene.
    fn terrain_target(&self, pointer: Pointer) -> Option<u64> {
        if let Some((hit, _)) = self.surface_under(pointer, &[])
            && let Some(owner) = self.terrain_owner(hit.entity)
        {
            return Some(owner);
        }
        if let Some(selected) = self.selection
            && let Ok(world) = self.world()
            && let Ok(handle) = world.handle_of(PersistentId(selected))
            && world.has_row(handle, "Terrain")
        {
            return Some(selected);
        }
        self.first_terrain()
    }

    /// Point under the pointer on a surface, or on the y = 0 plane when nothing is hit.
    pub(crate) fn ground_point(&self, pointer: Pointer) -> Option<Vec3> {
        if let Some((_, point)) = self.surface_under(pointer, &[]) {
            return Some(point);
        }
        let view = self.view_for(pointer);
        let ray = view.ray(pointer.mouse())?;
        if ray.direction.y.abs() < 1.0e-6 {
            return None;
        }
        let distance = -ray.origin.y / ray.direction.y;
        (distance >= 0.0).then_some(ray.origin + ray.direction * distance)
    }

    pub fn create_terrain(&mut self) -> Result<Json, Failure> {
        let result = self.execute(&CommandRecord::new(
            "terrain.create",
            json!({ "cell_size": 1.0, "size_meters": 128.0 }),
        ))?;
        if let Some(entity) = result.get("entity").and_then(Json::as_u64) {
            self.selection = Some(entity);
            self.natural.terrain = Some(entity);
        }
        Ok(result)
    }

    pub fn set_terrain_options(
        &mut self,
        mode: Option<&str>,
        radius: Option<f32>,
        strength: Option<f32>,
    ) -> Result<Json, Failure> {
        if let Some(name) = mode {
            self.natural.mode = TerrainPaintMode::parse(name).ok_or_else(|| {
                Failure::bad_request(&format!(
                    "modo de terreno `{name}` desconhecido (raise, lower, smooth, flatten ou erode)"
                ))
            })?;
        }
        if let Some(radius) = radius {
            if !radius.is_finite() {
                return Err(Failure::bad_request("o raio precisa ser um número"));
            }
            self.natural.radius = radius.clamp(0.25, 128.0);
        }
        if let Some(strength) = strength {
            if !strength.is_finite() {
                return Err(Failure::bad_request("a força precisa ser um número"));
            }
            self.natural.strength = strength.clamp(0.0, 32.0);
        }
        Ok(self.natural_json())
    }

    fn push_terrain_stamp(&mut self, point: Vec3) {
        let mode = self.natural.mode.name();
        let mut stamp = json!({
            "center": { "x": point.x, "y": point.y, "z": point.z },
            "radius": self.natural.radius,
            "strength": self.natural.strength,
            "mode": mode,
        });
        if self.natural.mode == TerrainPaintMode::Flatten
            && let Some(object) = stamp.as_object_mut()
        {
            object.insert("target_height".to_string(), json!(point.y));
        }
        self.natural.stamps.push(stamp);
        self.natural.last_stamp = Some(point);
    }

    pub fn terrain_begin(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        let terrain = match self.terrain_target(pointer) {
            Some(terrain) => terrain,
            None => {
                // Painting with no terrain yet creates the default one: one undo step for the
                // creation plus one for the stroke.
                self.create_terrain()?;
                self.natural.terrain.ok_or_else(|| {
                    Failure::new(
                        "NATURAL_NO_TERRAIN",
                        "não foi possível criar o terreno",
                        "Crie um terreno pelo menu Criar.",
                    )
                })?
            }
        };
        let point = self.ground_point(pointer).ok_or_else(|| {
            Failure::bad_request("o ponteiro não aponta para uma superfície ou o plano do chão")
        })?;
        self.natural.terrain = Some(terrain);
        self.natural.dragging = true;
        self.natural.stamps.clear();
        self.push_terrain_stamp(point);
        Ok(self.natural_json())
    }

    pub fn terrain_drag(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        if !self.natural.dragging {
            return Ok(self.natural_json());
        }
        let Some(point) = self.ground_point(pointer) else {
            return Ok(self.natural_json());
        };
        let spacing = self.natural.radius * STAMP_SPACING_RATIO;
        let far_enough = self
            .natural
            .last_stamp
            .is_none_or(|last| (point - last).length() >= spacing);
        if far_enough && self.natural.stamps.len() < 512 {
            self.push_terrain_stamp(point);
        }
        Ok(self.natural_json())
    }

    pub fn terrain_end(&mut self) -> Result<Json, Failure> {
        if !self.natural.dragging {
            return Ok(self.natural_json());
        }
        self.natural.dragging = false;
        let terrain = self.natural.terrain;
        let stamps = std::mem::take(&mut self.natural.stamps);
        self.natural.last_stamp = None;
        let Some(terrain) = terrain else {
            return Ok(self.natural_json());
        };
        if stamps.is_empty() {
            return Ok(self.natural_json());
        }
        let result = self.execute(&CommandRecord::new(
            "terrain.paint",
            json!({ "terrain": terrain, "stamps": stamps }),
        ))?;
        self.selection = Some(terrain);
        Ok(result)
    }

    pub fn terrain_cancel(&mut self) -> Json {
        self.natural.stamps.clear();
        self.natural.last_stamp = None;
        self.natural.dragging = false;
        self.natural_json()
    }

    /// Adds a river/road point under the pointer.
    pub fn natural_point(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let kind = match self.tool {
            Tool::RiverPath => Some(NaturalPathKind::River),
            Tool::RoadPath => Some(NaturalPathKind::Road),
            _ => self.natural.path_kind,
        };
        let kind = kind.unwrap_or(NaturalPathKind::River);
        let point = self.ground_point(pointer).ok_or_else(|| {
            Failure::bad_request("o ponteiro não aponta para uma superfície ou o plano do chão")
        })?;
        self.natural.path_kind = Some(kind);
        self.natural.path_points.push(point);
        Ok(self.natural_json())
    }

    pub fn natural_pop(&mut self) -> Json {
        self.natural.path_points.pop();
        self.natural_json()
    }

    pub fn natural_cancel(&mut self) -> Json {
        self.natural.path_points.clear();
        self.natural.path_kind = None;
        self.natural_json()
    }

    pub fn natural_commit(&mut self) -> Result<Json, Failure> {
        let kind = self
            .natural
            .path_kind
            .ok_or_else(|| Failure::bad_request("não há rio ou estrada em construção"))?;
        if self.natural.path_points.len() < 2 {
            return Err(Failure::bad_request(
                "desenhe pelo menos dois pontos antes de confirmar",
            ));
        }
        let spline = smooth_spline(
            &self.natural.path_points,
            self.natural.path_width,
            self.natural.path_depth,
        );
        let payload = match kind {
            NaturalPathKind::River => json!({
                "kind": "river",
                "spline": serde_json::to_value(&spline).unwrap_or(Json::Null),
                "flow_speed": 1.0,
                "float_force": 9.8,
                "swim_force": 4.0,
            }),
            NaturalPathKind::Road => json!({
                "kind": "road",
                "spline": serde_json::to_value(&spline).unwrap_or(Json::Null),
                "width": self.natural.path_width,
                "shoulder": 0.2,
                "material": "gravel",
            }),
        };
        let result = self.execute(&CommandRecord::new("path.create", payload))?;
        if let Some(entity) = result.get("entity").and_then(Json::as_u64) {
            self.selection = Some(entity);
        }
        self.natural.path_points.clear();
        self.natural.path_kind = None;
        Ok(result)
    }

    pub fn natural_width(
        &mut self,
        width: Option<f32>,
        depth: Option<f32>,
    ) -> Result<Json, Failure> {
        if let Some(width) = width {
            if !width.is_finite() || width <= 0.0 {
                return Err(Failure::bad_request("a largura precisa ser positiva"));
            }
            self.natural.path_width = width.clamp(0.1, 256.0);
        }
        if let Some(depth) = depth {
            if !depth.is_finite() || depth <= 0.0 {
                return Err(Failure::bad_request("a profundidade precisa ser positiva"));
            }
            self.natural.path_depth = depth.clamp(0.05, 64.0);
        }
        Ok(self.natural_json())
    }

    pub fn natural_hover(&mut self, pointer: Pointer) -> Json {
        self.natural.hover = self.ground_point(pointer);
        json!({
            "point": self.natural.hover.map(|point| json!({
                "x": point.x, "y": point.y, "z": point.z
            })),
            "path_points": self.natural.path_points.len(),
        })
    }

    pub(crate) fn natural_json(&self) -> Json {
        let selected_terrain = self.selection.filter(|id| {
            self.world().ok().is_some_and(|world| {
                world
                    .handle_of(PersistentId(*id))
                    .ok()
                    .is_some_and(|handle| world.has_row(handle, "Terrain"))
            })
        });
        let selected_road = self.selection.filter(|id| {
            self.world().ok().is_some_and(|world| {
                world
                    .handle_of(PersistentId(*id))
                    .ok()
                    .is_some_and(|handle| world.has_row(handle, "Road"))
            })
        });
        let guardrail = selected_road.is_some_and(|id| {
            self.world().ok().is_some_and(|world| {
                world
                    .handle_of(PersistentId(id))
                    .ok()
                    .is_some_and(|handle| {
                        matches!(
                            world.get(handle, "Road", "guardrail"),
                            Ok(Value::Bool(true))
                        )
                    })
            })
        });
        json!({
            "terrain": selected_terrain.or(self.natural.terrain),
            "mode": self.natural.mode.name(),
            "radius": self.natural.radius,
            "strength": self.natural.strength,
            "path_kind": self.natural.path_kind.map(NaturalPathKind::name),
            "path_points": self.natural.path_points.len(),
            "path_width": self.natural.path_width,
            "path_depth": self.natural.path_depth,
            "stamps": self.natural.stamps.len(),
            "dragging": self.natural.dragging,
            "selected_road": selected_road,
            "guardrail": guardrail,
        })
    }

    pub(crate) fn natural_overlay(&self, items: &mut Vec<OverlayItem>) {
        if !self.tool.is_natural() {
            return;
        }
        let color = match self.tool {
            Tool::RiverPath => RIVER_COLOR,
            Tool::RoadPath => ROAD_COLOR,
            _ => TERRAIN_RING_COLOR,
        };
        if self.tool.is_path_drawing() {
            for point in &self.natural.path_points {
                items.push(OverlayItem::Square {
                    center: *point,
                    color,
                    size_pixels: 9.0,
                });
            }
            let mut points = self.natural.path_points.clone();
            if let Some(hover) = self.natural.hover {
                points.push(hover);
            }
            for pair in points.windows(2) {
                if let (Some(from), Some(to)) = (pair.first(), pair.get(1)) {
                    items.push(OverlayItem::Line {
                        from: *from,
                        to: *to,
                        color,
                        width_pixels: 2.0,
                    });
                }
            }
            return;
        }
        let Some(center) = self.natural.hover else {
            return;
        };
        let radius = self.natural.radius;
        let segments = 40;
        for index in 0..segments {
            let first = index as f32 / segments as f32 * std::f32::consts::TAU;
            let second = (index + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let from = center + Vec3::new(first.cos() * radius, 0.05, first.sin() * radius);
            let to = center + Vec3::new(second.cos() * radius, 0.05, second.sin() * radius);
            items.push(OverlayItem::Line {
                from,
                to,
                color,
                width_pixels: 1.5,
            });
        }
    }
}
