use std::sync::Arc;

use petunia_math::{Color, Transform, Vec3};
use petunia_world::{EntityHandle, Value, World};

use crate::camera::Camera;
use crate::fly::FlyCamera;
use crate::mesh::{MeshCache, MeshData};
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
    /// An editable polygon mesh (`Mesh` table): the item carries its own triangles.
    Mesh,
    /// A terrain chunk (`Terrain` table): the item carries its own triangles and per-vertex colors.
    Terrain,
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
#[derive(Clone, Debug, PartialEq)]
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
    /// Drawn as edges only (the editor's X-ray): the surface stops hiding what is inside it.
    pub wireframe: bool,
    /// For [`Shape::Mesh`]: the triangles to draw (in the object's own coordinates) and the
    /// fingerprint of the polygon mesh they came from (renderers key GPU buffers by it).
    pub mesh: Option<Arc<MeshData>>,
    pub mesh_fingerprint: u64,
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

/// Most point lights one frame carries (the rest are dropped, nearest to the camera kept).
pub const MAX_POINT_LIGHTS: usize = 8;

/// A bulb: where it is, how far it reaches and the color already scaled by its intensity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    pub position: Vec3,
    pub range: f32,
    pub color: Vec3,
}

/// The lights of the scene. With `custom` false the renderer uses its built-in sun and ambient.
#[derive(Clone, Debug, PartialEq)]
pub struct Lighting {
    pub custom: bool,
    /// Direction **toward** the sun, and its color times its intensity.
    pub sun: Option<(Vec3, Vec3)>,
    pub points: Vec<PointLight>,
    /// Soft light on everything, as a color.
    pub ambient: Vec3,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            custom: false,
            sun: None,
            points: Vec::new(),
            ambient: Vec3::splat(0.35),
        }
    }
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
    pub lighting: Lighting,
    /// Optional horizontal cut plane: geometry above this world height is discarded. Used by the
    /// editor's X-ray/section view; `None` draws everything.
    pub section_height: Option<f32>,
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

fn transform_of(world: &World, handle: EntityHandle) -> Transform {
    Transform {
        position: vec3(world, handle, "Transform", "position").unwrap_or(Vec3::ZERO),
        rotation_degrees: vec3(world, handle, "Transform", "rotation_degrees")
            .unwrap_or(Vec3::ZERO),
        scale: vec3(world, handle, "Transform", "scale").unwrap_or(Vec3::ONE),
    }
}

/// Builds the frame from the world: every entity with a `Brush` or a `Mesh`. Entities with
/// unknown shapes are skipped (the Project Doctor reports them). Cuts every editable mesh into
/// triangles; a host that extracts often uses [`extract_frame_cached`].
pub fn extract_frame(world: &World, camera: Camera) -> RenderFrame {
    extract_frame_cached(world, camera, &mut MeshCache::new())
}

/// [`extract_frame`] reusing the triangles of meshes that did not change.
pub fn extract_frame_cached(world: &World, camera: Camera, cache: &mut MeshCache) -> RenderFrame {
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
        let transform = transform_of(world, handle);
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
            mesh: None,
            mesh_fingerprint: 0,
            wireframe: false,
        });
    }
    for handle in world.entities_with_table("Mesh").unwrap_or_default() {
        let Ok(Value::Mesh(geometry)) = world.get(handle, "Mesh", "geometry") else {
            continue;
        };
        let Some((low, high)) = geometry.bounds() else {
            continue;
        };
        let color = match world.get(handle, "Mesh", "color") {
            Ok(Value::Color(color)) => *color,
            _ => Color::WHITE,
        };
        let fingerprint = geometry.fingerprint();
        items.push(DrawItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: high - low,
            detail: 0,
            transform: transform_of(world, handle),
            color,
            mesh: Some(cache.drawing(fingerprint, geometry)),
            mesh_fingerprint: fingerprint,
            wireframe: false,
        });
    }
    for handle in world.entities_with_table("Terrain").unwrap_or_default() {
        let Ok(Value::Terrain(terrain)) = world.get(handle, "Terrain", "data") else {
            continue;
        };
        let entity = world.persistent_id(handle).map_or(0, |id| id.0);
        let cells = terrain.chunk_cells().max(1);
        let cell_size = terrain.cell_size();
        for chunk in terrain.chunks() {
            let coord = chunk.coord();
            let center = Vec3::new(
                (coord.x as f32 + 0.5) * cells as f32 * cell_size,
                0.0,
                (coord.z as f32 + 0.5) * cells as f32 * cell_size,
            );
            let distance = (center - camera.position).length();
            let step = crate::terrain::lod_step(distance).min(cells);
            let fingerprint = crate::terrain::chunk_fingerprint(terrain, coord, step);
            let mesh = cache.cached(fingerprint, || {
                crate::terrain::chunk_mesh(terrain, coord, step)
            });
            items.push(DrawItem {
                entity,
                selected: false,
                hovered: false,
                shape: Shape::Terrain,
                size: Vec3::ONE,
                detail: 0,
                transform: Transform::default(),
                color: Color::WHITE,
                mesh: Some(mesh),
                mesh_fingerprint: fingerprint,
                wireframe: false,
            });
        }
    }
    let lighting = extract_lighting(world, camera.position);
    RenderFrame {
        camera,
        items,
        sculpts: Vec::new(),
        overlay: Vec::new(),
        show_grid: false,
        grid_height: 0.0,
        clear_color: Color::rgb(0.53, 0.70, 0.88),
        lighting,
        section_height: None,
    }
}

/// Reads the `Light` rows. Bad rows are skipped; the Project Doctor reports them.
fn extract_lighting(world: &World, camera: Vec3) -> Lighting {
    let mut lighting = Lighting::default();
    let mut first = true;
    let mut points: Vec<PointLight> = Vec::new();
    for handle in world.entities_with_table("Light").unwrap_or_default() {
        let Ok(Value::Text(kind)) = world.get(handle, "Light", "kind") else {
            continue;
        };
        let intensity = match world.get(handle, "Light", "intensity") {
            Ok(Value::Float(value)) if value.is_finite() => (*value as f32).clamp(0.0, 16.0),
            _ => 1.0,
        };
        let color = match world.get(handle, "Light", "color") {
            Ok(Value::Color(color)) if color.is_finite() => Vec3::new(color.r, color.g, color.b),
            _ => Vec3::ONE,
        } * intensity;
        lighting.custom = true;
        if first {
            if let Ok(Value::Float(ambient)) = world.get(handle, "Light", "ambient")
                && ambient.is_finite()
            {
                lighting.ambient = Vec3::splat((*ambient as f32).clamp(0.0, 1.0));
            }
            first = false;
        }
        let transform = transform_of(world, handle);
        match kind.as_str() {
            "sun" => {
                if lighting.sun.is_none() {
                    // It shines along its forward direction: the light comes from the opposite way.
                    lighting.sun = Some((transform.forward() * -1.0, color));
                }
            }
            "point" => {
                let range = match world.get(handle, "Light", "range") {
                    Ok(Value::Float(value)) if value.is_finite() => {
                        (*value as f32).clamp(0.1, 1000.0)
                    }
                    _ => 10.0,
                };
                points.push(PointLight {
                    position: transform.position,
                    range,
                    color,
                });
            }
            _ => {}
        }
    }
    // Too many bulbs: keep the ones nearest to the camera, in a stable order.
    points.sort_by(|a, b| {
        (a.position - camera)
            .length_squared()
            .total_cmp(&(b.position - camera).length_squared())
    });
    points.truncate(MAX_POINT_LIGHTS);
    lighting.points = points;
    lighting
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

/// The solid form of everything that is drawn: brushes, editable meshes and sculpted caves, in
/// each object's own coordinates. The physics makes colliders from it for objects that have none.
pub fn solid_shapes(world: &World) -> std::collections::BTreeMap<u64, petunia_math::SolidShape> {
    use petunia_math::SolidShape;

    let mut found = std::collections::BTreeMap::new();
    let frame = extract_frame(world, Camera::default());
    for item in &frame.items {
        if item.shape == Shape::Terrain {
            // Terrain gets a heightfield collider, not a triangle soup per chunk.
            continue;
        }
        let Ok(mesh) = crate::mesh::item_mesh(item) else {
            continue;
        };
        if let Some(solid) =
            SolidShape::from_triangles(&mesh.positions, &mesh.indices, item.shape == Shape::Box)
        {
            found.insert(item.entity, solid);
        }
    }
    let mut cache = crate::sculpt::SculptCache::new();
    for item in cache.items(&crate::sculpt::extract_sculpt_volumes(world)) {
        let parts: Vec<(&[[f32; 3]], &[u32])> = item
            .chunks
            .iter()
            .map(|chunk| {
                (
                    chunk.mesh.positions.as_slice(),
                    chunk.mesh.indices.as_slice(),
                )
            })
            .collect();
        if let Some(solid) = SolidShape::merged(&parts) {
            found.insert(item.entity, solid);
        }
    }
    found
}
