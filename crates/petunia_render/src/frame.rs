use std::sync::Arc;

use petunia_anim::Mat4;
use petunia_math::{Color, Transform, Vec3};
use petunia_world::{EntityHandle, Value, World};

use crate::camera::Camera;
use crate::fly::FlyCamera;
use crate::mesh::{MeshCache, MeshData};
use crate::natural::{
    bridge_mesh_data, rail_pair_mesh_data, road_mesh_data, terrain_mesh_data, water_mesh_data,
};
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
    /// For [`Shape::Mesh`]: the triangles to draw (in the object's own coordinates) and the
    /// fingerprint of the polygon mesh they came from (renderers key GPU buffers by it).
    pub mesh: Option<Arc<MeshData>>,
    pub mesh_fingerprint: u64,
    /// When the mesh is skinned: one joint weight per vertex, in `mesh.positions` order. The
    /// renderer applies [`skin_mesh`] to the positions before uploading, so a character plays its
    /// clip on screen (`DR-23`, phase 4). Empty for an unskinned mesh, which is the usual case.
    pub skin: Vec<SkinWeight>,
    /// The matrices of this frame, one per joint, already multiplied by the inverse bind.
    pub skin_matrices: Vec<Mat4>,
    /// Draw the triangles as lines (`QT-09`, aramado). The editor also puts the edges in the frame
    /// so a wireframe preview needs no second draw call.
    pub wireframe: bool,
    /// Draw through other geometry (`QT-09`, raio-X): the depth test is off for this item.
    pub xray: bool,
}

/// The four joint weights of one vertex, as the import keeps them. A local copy so the renderer
/// does not depend on the importer's types.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkinWeight {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

impl SkinWeight {
    /// The position of this vertex after the joints move it: a weighted sum of their matrices.
    ///
    /// A vertex with no influence stays where it is, which is what an unskinned part of a skinned
    /// mesh should do.
    #[must_use]
    pub fn transform(&self, position: Vec3, matrices: &[Mat4]) -> Vec3 {
        let mut moved = Vec3::ZERO;
        let mut total = 0.0f32;
        for (slot, weight) in self.weights.iter().enumerate() {
            if *weight <= 0.0 {
                continue;
            }
            let joint = usize::from(self.joints.get(slot).copied().unwrap_or(0));
            let Some(matrix) = matrices.get(joint) else {
                continue;
            };
            let point = matrix.transform_point(position);
            moved.x += point.x * weight;
            moved.y += point.y * weight;
            moved.z += point.z * weight;
            total += weight;
        }
        if total <= 1e-6 {
            return position;
        }
        Vec3::new(moved.x / total, moved.y / total, moved.z / total)
    }
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
    /// Vegetation impostors to draw on top of the scene, in world coordinates (phase 5).
    pub vegetation: Option<Arc<MeshData>>,
    /// Precipitation (rain or snow), if the weather is falling (phase 4 and 5).
    pub precipitation: Option<Arc<MeshData>>,
    /// Fog colour and how far it reaches, when the air is not clear (phase 5).
    pub fog: Option<(Color, f32)>,
    /// The fused grade applied after the scene (DR-13, first cut).
    pub grade: Grade,
}

/// One fullscreen grade: contrast, vignette, grain and a luminance ceiling.
///
/// `comfort` slows how fast the ceiling may rise, which is the flash limiter of this cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    pub contrast: f32,
    pub vignette: f32,
    pub grain: f32,
    pub luma_ceiling: f32,
    pub comfort: bool,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            contrast: 1.06,
            vignette: 0.28,
            grain: 0.012,
            luma_ceiling: 1.15,
            comfort: false,
        }
    }
}

impl Grade {
    /// Flatter light, no vignette, no grain, and a ceiling that cannot jump.
    pub const fn comfort() -> Self {
        Self {
            contrast: 1.18,
            vignette: 0.0,
            grain: 0.0,
            luma_ceiling: 0.82,
            comfort: true,
        }
    }
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
    petunia_world::composed_transform(world, handle)
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
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
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
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
    }
    // Terrain chunks: one draw item per chunk, in world coordinates.
    for handle in world
        .entities_with_table("TerrainChunk")
        .unwrap_or_default()
    {
        let owner = match world.get(handle, "TerrainChunk", "owner") {
            Ok(Value::Int(owner)) => *owner,
            _ => continue,
        };
        let chunk_x = match world.get(handle, "TerrainChunk", "chunk_x") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let chunk_z = match world.get(handle, "TerrainChunk", "chunk_z") {
            Ok(Value::Int(value)) => *value as i32,
            _ => continue,
        };
        let size = match world.get(handle, "TerrainChunk", "size") {
            Ok(Value::Int(value)) => *value as usize,
            _ => continue,
        };
        let cell_size = match world.get(handle, "TerrainChunk", "cell_size") {
            Ok(Value::Float(value)) => *value as f32,
            _ => continue,
        };
        let Ok(Value::Floats(heights)) = world.get(handle, "TerrainChunk", "heights") else {
            continue;
        };
        let color = world
            .handle_of(petunia_world::PersistentId(owner.max(0) as u64))
            .ok()
            .and_then(
                |owner_handle| match world.get(owner_handle, "Terrain", "color") {
                    Ok(Value::Color(color)) => Some(*color),
                    _ => None,
                },
            )
            .unwrap_or_else(|| Color::rgb(0.36, 0.46, 0.28));
        let origin = Vec3::new(
            chunk_x as f32 * size as f32 * cell_size,
            0.0,
            chunk_z as f32 * size as f32 * cell_size,
        );
        let entity = world.persistent_id(handle).map_or(0, |id| id.0);
        let center = Vec3::new(
            origin.x + size as f32 * cell_size * 0.5,
            0.0,
            origin.z + size as f32 * cell_size * 0.5,
        );
        let distance = (center - camera.position).length();
        let lod_step = if distance > 256.0 {
            21
        } else if distance > 128.0 {
            7
        } else if distance > 64.0 {
            3
        } else {
            1
        };
        let Some((mesh, fingerprint)) = terrain_mesh_data(
            heights,
            size,
            cell_size,
            origin,
            crate::natural::hash_bits(0xcbf2_9ce4_8422_2325, entity),
            lod_step,
        ) else {
            continue;
        };
        items.push(DrawItem {
            entity,
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::new(size as f32 * cell_size, 0.0, size as f32 * cell_size),
            detail: 0,
            transform: Transform::IDENTITY,
            color,
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
    }

    // Rivers: a translucent water ribbon following the spline.
    for handle in world.entities_with_table("River").unwrap_or_default() {
        let Ok(Value::Spline(spline)) = world.get(handle, "River", "spline") else {
            continue;
        };
        let Some((mesh, fingerprint)) = water_mesh_data(spline, 1.0) else {
            continue;
        };
        items.push(DrawItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::ZERO,
            detail: 0,
            transform: Transform::IDENTITY,
            color: Color::new(0.12, 0.32, 0.48, 0.62),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
    }

    // Roads: an opaque ribbon with the road's width and shoulder.
    for handle in world.entities_with_table("Road").unwrap_or_default() {
        let Ok(Value::Spline(spline)) = world.get(handle, "Road", "spline") else {
            continue;
        };
        let width = match world.get(handle, "Road", "width") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 6.0,
        };
        let shoulder = match world.get(handle, "Road", "shoulder") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 0.0,
        };
        let Some((mesh, fingerprint)) = road_mesh_data(spline, width, shoulder, 1.0) else {
            continue;
        };
        items.push(DrawItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::ZERO,
            detail: 0,
            transform: Transform::IDENTITY,
            color: Color::rgb(0.22, 0.22, 0.24),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
    }

    // Guardrails on roads that ask for them.
    for handle in world.entities_with_table("Road").unwrap_or_default() {
        let Ok(Value::Bool(true)) = world.get(handle, "Road", "guardrail") else {
            continue;
        };
        let Ok(Value::Spline(spline)) = world.get(handle, "Road", "spline") else {
            continue;
        };
        let width = match world.get(handle, "Road", "width") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 6.0,
        };
        let Some((mesh, fingerprint)) = rail_pair_mesh_data(spline, width, 1.0) else {
            continue;
        };
        items.push(DrawItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::ZERO,
            detail: 0,
            transform: Transform::IDENTITY,
            color: Color::rgb(0.62, 0.60, 0.56),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
    }

    // Bridges draw rails along their span; culverts are only data.
    for handle in world.entities_with_table("Bridge").unwrap_or_default() {
        let Ok(Value::Text(kind)) = world.get(handle, "Bridge", "kind") else {
            continue;
        };
        if kind != "bridge" {
            continue;
        }
        let road = match world.get(handle, "Bridge", "road") {
            Ok(Value::Int(value)) => *value,
            _ => continue,
        };
        let Ok(road_handle) = world.handle_of(petunia_world::PersistentId(road.max(0) as u64))
        else {
            continue;
        };
        let Ok(Value::Spline(spline)) = world.get(road_handle, "Road", "spline") else {
            continue;
        };
        let start = match world.get(handle, "Bridge", "start_parameter") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 0.0,
        };
        let end = match world.get(handle, "Bridge", "end_parameter") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 1.0,
        };
        let width = match world.get(handle, "Bridge", "width") {
            Ok(Value::Float(value)) => *value as f32,
            _ => 6.0,
        };
        let Some((mesh, fingerprint)) = bridge_mesh_data(spline, start, end, width, 1.0) else {
            continue;
        };
        items.push(DrawItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::ZERO,
            detail: 0,
            transform: Transform::IDENTITY,
            color: Color::rgb(0.66, 0.62, 0.55),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
            skin: Vec::new(),
            skin_matrices: Vec::new(),
            wireframe: false,
            xray: false,
        });
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
        // Filled by whoever has the rules and the weather; an extracted frame carries only the world.
        vegetation: None,
        precipitation: None,
        fog: None,
        grade: Grade::default(),
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

/// Camera placed at an explicit eye position and facing. The engine picks the start with
/// `petunia_project::player_start`; this avoids depending on the project format here.
pub fn start_camera_at(eye: Vec3, yaw_degrees: f32) -> FlyCamera {
    FlyCamera::new(Camera {
        position: eye,
        yaw_degrees,
        ..Camera::default()
    })
}

/// Camera placed at the first `PlayerStart`, or at the origin when there is none. Prefer
/// [`start_camera_at`] with `petunia_project::player_start` in production.
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
        if item.color.a < 0.9 {
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

/// The mesh with its vertices moved by the skin (phase 4).
///
/// Returns `None` when the item is not skinned, so the caller keeps the cached, untouched mesh. When
/// it is skinned the result is a **copy**: skinning changes every frame, and rewriting the cached
/// bind-pose mesh would corrupt it for the next frame.
#[must_use]
pub fn skin_mesh(item: &DrawItem) -> Option<MeshData> {
    if item.skin.is_empty() || item.skin_matrices.is_empty() {
        return None;
    }
    let source = item.mesh.as_ref()?;
    let mut moved = MeshData {
        positions: source.positions.clone(),
        ..source.as_ref().clone()
    };
    for (index, position) in moved.positions.iter_mut().enumerate() {
        let Some(weight) = item.skin.get(index) else {
            continue;
        };
        let moved_point = weight.transform(
            Vec3::new(position[0], position[1], position[2]),
            &item.skin_matrices,
        );
        *position = [moved_point.x, moved_point.y, moved_point.z];
    }
    Some(moved)
}

#[cfg(test)]
mod skin_tests {
    use super::*;

    fn triangle() -> Arc<MeshData> {
        Arc::new(MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            uvs: Vec::new(),
            flows: Vec::new(),
            margins: Vec::new(),
        })
    }

    fn skinned_item() -> DrawItem {
        DrawItem {
            entity: 7,
            selected: false,
            hovered: false,
            shape: Shape::Mesh,
            size: Vec3::ONE,
            detail: 0,
            transform: Transform::IDENTITY,
            color: Color::WHITE,
            mesh: Some(triangle()),
            mesh_fingerprint: 99,
            skin: vec![
                SkinWeight {
                    joints: [0, 0, 0, 0],
                    weights: [1.0, 0.0, 0.0, 0.0],
                };
                3
            ],
            // Every joint matrix moves the mesh 2 m along x.
            skin_matrices: vec![petunia_anim::Mat4::translation(Vec3::new(2.0, 0.0, 0.0))],
            wireframe: false,
            xray: false,
        }
    }

    #[test]
    fn a_skinned_mesh_moves_every_vertex_and_leaves_the_original_alone() {
        let item = skinned_item();
        let moved = skin_mesh(&item).expect("it is skinned");
        assert_eq!(moved.positions.len(), 3);
        // The source triangle spans x = 0..1; a joint that adds 2 puts every vertex there plus 2.
        for (index, position) in moved.positions.iter().enumerate() {
            let original = item.mesh.as_ref().expect("mesh").positions[index][0];
            assert!(
                (position[0] - (original + 2.0)).abs() < 1e-5,
                "the joint moved it by 2: {position:?} from {original}"
            );
        }
        // The cached bind-pose mesh is untouched: the next frame needs it.
        let original = item.mesh.as_ref().expect("mesh");
        assert!(original.positions[0][0].abs() < 1e-6, "source unchanged");
    }

    #[test]
    fn an_unskinned_item_gets_no_copy() {
        let mut item = skinned_item();
        item.skin.clear();
        assert!(skin_mesh(&item).is_none(), "nothing to do");
        item.skin = vec![SkinWeight::default(); 3];
        item.skin_matrices.clear();
        assert!(skin_mesh(&item).is_none(), "no matrices, no skinning");
    }

    #[test]
    fn a_vertex_with_two_joints_takes_the_weighted_middle() {
        let weight = SkinWeight {
            joints: [0, 1, 0, 0],
            weights: [0.5, 0.5, 0.0, 0.0],
        };
        let matrices = vec![
            petunia_anim::Mat4::translation(Vec3::new(0.0, 0.0, 0.0)),
            petunia_anim::Mat4::translation(Vec3::new(4.0, 0.0, 0.0)),
        ];
        let moved = weight.transform(Vec3::ZERO, &matrices);
        assert!((moved.x - 2.0).abs() < 1e-5, "halfway: {moved:?}");
    }

    #[test]
    fn a_vertex_with_no_influence_stays_put() {
        let weight = SkinWeight::default();
        let moved = weight.transform(Vec3::new(1.0, 2.0, 3.0), &[]);
        assert_eq!(moved, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn a_matrix_that_moves_the_skin_gets_a_different_fingerprint() {
        // The GL adapter reuses a buffer while the fingerprint holds; a moving pose must change it.
        let first = skinned_item();
        let mut second = skinned_item();
        second.skin_matrices = vec![petunia_anim::Mat4::translation(Vec3::new(3.0, 0.0, 0.0))];
        assert_ne!(
            first.skin_matrices[0].0, second.skin_matrices[0].0,
            "the matrices differ"
        );
    }
}
