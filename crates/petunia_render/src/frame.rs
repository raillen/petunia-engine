use petunia_math::{Color, Transform, Vec3};
use petunia_world::{EntityHandle, Value, World};

use crate::camera::Camera;
use crate::fly::FlyCamera;
use crate::sculpt::SculptItem;

/// The solid shapes a brush can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Box,
    Ramp,
    Cylinder,
    Cone,
    Sphere,
    Stairs,
}

impl Shape {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "box" => Some(Self::Box),
            "ramp" => Some(Self::Ramp),
            "cylinder" => Some(Self::Cylinder),
            "cone" => Some(Self::Cone),
            "sphere" => Some(Self::Sphere),
            "stairs" => Some(Self::Stairs),
            _ => None,
        }
    }
}

/// One thing to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawItem {
    /// Persistent id of the entity this item was built from (`0` when unknown).
    pub entity: u64,
    /// The editor sets this so the renderer can highlight the selection.
    pub selected: bool,
    /// The editor sets this for the object under the mouse (pre-selection).
    pub hovered: bool,
    pub shape: Shape,
    pub size: Vec3,
    /// Segments around a round shape, or steps of a staircase (`0` means the default).
    pub detail: u32,
    pub transform: Transform,
    pub color: Color,
}

/// A 2D-looking helper drawn on top of the scene (editor gizmos). Sizes are in pixels, so a
/// handle keeps the same size on screen at any distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayItem {
    Line {
        from: Vec3,
        to: Vec3,
        color: Color,
        width_pixels: f32,
    },
    Square {
        center: Vec3,
        color: Color,
        size_pixels: f32,
    },
}

/// Everything the renderer needs for one frame. Read-only copy of the relevant world data.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderFrame {
    pub camera: Camera,
    pub items: Vec<DrawItem>,
    /// Sculpted volumes (caves, rocks). Empty after `extract_frame`: a host that draws them keeps a
    /// `SculptCache` and fills this from `extract_sculpt_volumes`.
    pub sculpts: Vec<SculptItem>,
    /// Gizmos and guides drawn after the scene, on top of it. Empty outside the editor.
    pub overlay: Vec<OverlayItem>,
    /// Draw the ground grid and the X/Z axes (editor only).
    pub show_grid: bool,
    /// Height of the grid: the floor being built (editor "andares").
    pub grid_height: f32,
    pub clear_color: Color,
}

impl RenderFrame {
    /// Marks the items that belong to `entity` as selected (and clears every other mark).
    pub fn mark_selected(&mut self, entity: Option<u64>) {
        for item in &mut self.items {
            item.selected = entity.is_some_and(|id| id != 0 && id == item.entity);
        }
        for sculpt in &mut self.sculpts {
            sculpt.selected = entity.is_some_and(|id| id != 0 && id == sculpt.entity);
        }
    }

    /// Marks the object under the mouse (and clears every other mark).
    pub fn mark_hovered(&mut self, entity: Option<u64>) {
        for item in &mut self.items {
            item.hovered = entity.is_some_and(|id| id != 0 && id == item.entity);
        }
        for sculpt in &mut self.sculpts {
            sculpt.hovered = entity.is_some_and(|id| id != 0 && id == sculpt.entity);
        }
    }
}

fn vec3(world: &World, handle: EntityHandle, table: &str, column: &str) -> Option<Vec3> {
    match world.get(handle, table, column) {
        Ok(Value::Vec3(value)) => Some(*value),
        _ => None,
    }
}

/// Builds the frame from the world: every entity with a `Brush` and a `Transform`. Entities with
/// unknown shapes are skipped (the Project Doctor reports them).
pub fn extract_frame(world: &World, camera: Camera) -> RenderFrame {
    let mut items = Vec::new();
    for handle in world.entities_with_table("Brush").unwrap_or_default() {
        let Ok(Value::Text(shape_name)) = world.get(handle, "Brush", "shape") else {
            continue;
        };
        let Some(shape) = Shape::parse(shape_name) else {
            continue;
        };
        let Some(size) = vec3(world, handle, "Brush", "size") else {
            continue;
        };
        let detail = match world.get(handle, "Brush", "detail") {
            Ok(Value::Int(value)) => u32::try_from(*value).unwrap_or(0),
            _ => 0,
        };
        let color = match world.get(handle, "Brush", "color") {
            Ok(Value::Color(color)) => *color,
            _ => Color::WHITE,
        };
        let transform = Transform {
            position: vec3(world, handle, "Transform", "position").unwrap_or(Vec3::ZERO),
            rotation_degrees: vec3(world, handle, "Transform", "rotation_degrees")
                .unwrap_or(Vec3::ZERO),
            scale: vec3(world, handle, "Transform", "scale").unwrap_or(Vec3::ONE),
        };
        let entity = world.persistent_id(handle).map_or(0, |id| id.0);
        items.push(DrawItem {
            entity,
            selected: false,
            hovered: false,
            shape,
            size,
            detail,
            transform,
            color,
        });
    }
    RenderFrame {
        camera,
        items,
        sculpts: Vec::new(),
        overlay: Vec::new(),
        show_grid: false,
        grid_height: 0.0,
        clear_color: Color::rgb(0.53, 0.70, 0.88),
    }
}

/// Camera placed at the first `PlayerStart`, or at the origin when there is none.
pub fn start_camera(world: &World) -> FlyCamera {
    let mut camera = Camera::default();
    if let Some(handle) = world
        .entities_with_table("PlayerStart")
        .unwrap_or_default()
        .first()
        .copied()
    {
        camera.position = vec3(world, handle, "Transform", "position").unwrap_or(Vec3::ZERO);
        if let Ok(Value::Float(yaw)) = world.get(handle, "PlayerStart", "yaw_degrees") {
            camera.yaw_degrees = *yaw as f32;
        }
    }
    FlyCamera::new(camera)
}
