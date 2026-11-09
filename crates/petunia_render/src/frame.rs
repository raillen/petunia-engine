use std::sync::Arc;

use petunia_math::{Color, Transform, Vec3};
use petunia_world::{EntityHandle, Value, World};

use crate::camera::Camera;
use crate::fly::FlyCamera;
use crate::mesh::{MeshCache, MeshData};
use crate::natural::{
    bridge_mesh_data, rail_pair_mesh_data, road_mesh_data, terrain_mesh_data, water_mesh_data,
};
use crate::sculpt::SculptItem;
use crate::texture::{TextureId, TexturePixels, TextureStore};

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
}

/// Most frames one sheet may hold: a hostile scene cannot ask for endless frames.
pub const MAX_SPRITE_FRAMES: u32 = 4096;
/// Most frames a sheet may put in one row.
pub const MAX_SPRITE_COLUMNS: u32 = 64;
/// Fastest animation allowed, in frames per second.
pub const MAX_SPRITE_FPS: f32 = 240.0;

/// A sprite sheet: one picture cut into a grid of frames (DR-39). Which frame is shown comes from
/// the host's clock every frame, so nothing is written back to the scene and two sprites can play
/// the same sheet at different times.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteAnimation {
    /// How many frames of the sheet the animation uses (at least 1).
    pub frames: u32,
    /// Frames per row of the sheet; the first row is the **top** of the picture.
    pub columns: u32,
    pub fps: f32,
    /// Start over at the end instead of holding the last frame.
    pub looping: bool,
    pub playing: bool,
    /// The frame shown when it is not playing, and while editing.
    pub frame: u32,
}

impl SpriteAnimation {
    /// The whole picture as one frame: what a sprite without a `SpriteAnimation` row shows.
    pub fn single() -> Self {
        Self {
            frames: 1,
            columns: 1,
            fps: 0.0,
            looping: false,
            playing: false,
            frame: 0,
        }
    }

    /// The same animation with every value inside the limits. Bad numbers park the sprite instead
    /// of dropping it: a sprite with a broken row still draws its frame 0.
    pub fn sanitized(self) -> Self {
        let frames = self.frames.clamp(1, MAX_SPRITE_FRAMES);
        let columns = self.columns.clamp(1, MAX_SPRITE_COLUMNS);
        let fps = if self.fps.is_finite() {
            self.fps.clamp(0.0, MAX_SPRITE_FPS)
        } else {
            0.0
        };
        Self {
            frames,
            columns,
            fps,
            looping: self.looping,
            playing: self.playing,
            frame: self.frame.min(frames - 1),
        }
    }

    /// How many rows the sheet has.
    pub fn rows(&self) -> u32 {
        self.frames.div_ceil(self.columns.max(1))
    }

    /// Which frame is shown `seconds` after the start of the host's clock (0 = not running).
    pub fn frame_at(&self, seconds: f32) -> u32 {
        let frames = self.frames.max(1);
        let rest = self.frame.min(frames - 1);
        // A clock that is not running, a sheet that is paused and a speed that is not a speed all
        // park the sprite on the frame the row stores.
        let running = self.playing && self.fps.is_finite() && self.fps > 0.0;
        if !running || !seconds.is_finite() || seconds <= 0.0 {
            return rest;
        }
        // A very long run must not overflow: the count saturates far above any real sheet.
        let played = (seconds * self.fps).min(MAX_SPRITE_FRAMES as f32 * 1024.0) as u32;
        if self.looping {
            played % frames
        } else {
            played.min(frames - 1)
        }
    }

    /// The rectangle of `frame` in the sheet, from 0 to 1, with the mirroring applied inside it so
    /// a mirrored sheet frame still samples only its own picture.
    pub fn uv(&self, frame: u32, flip_x: bool, flip_y: bool) -> [f32; 4] {
        let columns = self.columns.max(1);
        let rows = self.rows().max(1);
        let frame = frame.min(self.frames.max(1) - 1);
        let column = frame % columns;
        let row = frame / columns;
        let mut left = column as f32 / columns as f32;
        let mut right = (column + 1) as f32 / columns as f32;
        let mut top = row as f32 / rows as f32;
        let mut bottom = (row + 1) as f32 / rows as f32;
        if flip_x {
            std::mem::swap(&mut left, &mut right);
        }
        if flip_y {
            std::mem::swap(&mut top, &mut bottom);
        }
        [left, top, right, bottom]
    }
}

/// A flat, textured quad: what a 2D game is made of (DR-38). It lies on the XY plane of its own
/// transform, so the same sprite works under a perspective or an orthographic camera.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteItem {
    /// Persistent id of the entity this item was built from (`0` when unknown).
    pub entity: u64,
    pub selected: bool,
    pub hovered: bool,
    /// Width and height in meters, before the object's own scale.
    pub width: f32,
    pub height: f32,
    pub transform: Transform,
    /// Tint multiplied over the texture; alpha fades the whole sprite.
    pub color: Color,
    /// Asset path exactly as the `Sprite` row names it (for example `assets/hero.png`).
    pub asset: String,
    /// Filled by [`RenderFrame::resolve_textures`].
    pub texture: Option<TextureId>,
    pub flip_x: bool,
    pub flip_y: bool,
    /// Draw order: a higher layer is drawn later, on top of a lower one.
    pub layer: i32,
    /// The sheet this sprite plays; [`SpriteAnimation::single`] when it has no animation row.
    pub animation: SpriteAnimation,
    /// The frame shown right now, filled by [`RenderFrame::animate_sprites`].
    pub frame: u32,
}

impl SpriteItem {
    /// The rectangle of the picture this sprite draws, from 0 to 1 (the whole picture unless the
    /// animation row cuts it into frames).
    pub fn uv(&self) -> [f32; 4] {
        self.animation.uv(self.frame, self.flip_x, self.flip_y)
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
    /// Flat textured quads (2D sprites), drawn after the terrain and before the overlay.
    pub sprites: Vec<SpriteItem>,
    /// Pixels of the textures the sprites use, ready to upload. Empty when nothing is textured.
    pub textures: Vec<Arc<TexturePixels>>,
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
    /// The host's clock in seconds, used by sprite animations (DR-39). The editor leaves it at 0
    /// while editing, so a scene at rest draws the same picture every frame; Play mode and the
    /// player advance it.
    pub seconds: f32,
}

impl RenderFrame {
    /// Marks the items that belong to `entity` as selected (and clears every other mark).
    pub fn mark_selected(&mut self, entity: Option<u64>) {
        for item in &mut self.items {
            item.selected = entity.is_some_and(|id| id != 0 && id == item.entity);
        }
        for sprite in &mut self.sprites {
            sprite.selected = entity.is_some_and(|id| id != 0 && id == sprite.entity);
        }
        for sculpt in &mut self.sculpts {
            sculpt.selected = entity.is_some_and(|id| id != 0 && id == sculpt.entity);
        }
    }

    /// Resolves which frame every animated sprite shows at `seconds` of the host's clock. A host
    /// that passes 0 (the editor while editing) shows the frame stored in the row, so a scene at
    /// rest draws the same picture every time (DR-39). The clock is kept in the frame, so a host
    /// or a test can see which moment was drawn.
    pub fn animate_sprites(&mut self, seconds: f32) {
        self.seconds = seconds;
        for sprite in &mut self.sprites {
            sprite.frame = sprite.animation.frame_at(seconds);
        }
    }

    /// Marks the object under the mouse (and clears every other mark).
    pub fn mark_hovered(&mut self, entity: Option<u64>) {
        for item in &mut self.items {
            item.hovered = entity.is_some_and(|id| id != 0 && id == item.entity);
        }
        for sprite in &mut self.sprites {
            sprite.hovered = entity.is_some_and(|id| id != 0 && id == sprite.entity);
        }
        for sculpt in &mut self.sculpts {
            sculpt.hovered = entity.is_some_and(|id| id != 0 && id == sculpt.entity);
        }
    }

    /// Points every sprite at the texture its asset names, and collects the pixels the renderer
    /// has to upload. Sprites whose asset is not loaded stay as plain colored quads.
    pub fn resolve_textures(&mut self, store: &TextureStore) {
        let mut used = std::collections::BTreeSet::new();
        for sprite in &mut self.sprites {
            sprite.texture = store.id_of(&sprite.asset);
            if let Some(id) = sprite.texture {
                used.insert(id);
            }
        }
        self.textures = used.into_iter().filter_map(|id| store.pixels(id)).collect();
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
            color: Color::new(0.18, 0.42, 0.72, 0.55),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
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
            color: Color::rgb(0.42, 0.39, 0.35),
            mesh: Some(cache.store_drawing(fingerprint, mesh)),
            mesh_fingerprint: fingerprint,
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
        });
    }

    let mut sprites = Vec::new();
    for handle in world.entities_with_table("Sprite").unwrap_or_default() {
        let size = vec3(world, handle, "Sprite", "size").unwrap_or(Vec3::ONE);
        if !size.x.is_finite() || !size.y.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        let asset = match world.get(handle, "Sprite", "asset") {
            Ok(Value::Text(asset)) => asset.clone(),
            _ => String::new(),
        };
        let color = match world.get(handle, "Sprite", "color") {
            Ok(Value::Color(color)) => *color,
            _ => Color::WHITE,
        };
        let layer = match world.get(handle, "Sprite", "layer") {
            Ok(Value::Int(layer)) => i32::try_from(*layer).unwrap_or(0),
            _ => 0,
        };
        let animation = animation_of(world, handle);
        sprites.push(SpriteItem {
            entity: world.persistent_id(handle).map_or(0, |id| id.0),
            selected: false,
            hovered: false,
            width: size.x,
            height: size.y,
            transform: transform_of(world, handle),
            color,
            asset,
            texture: None,
            flip_x: matches!(world.get(handle, "Sprite", "flip_x"), Ok(Value::Bool(true))),
            flip_y: matches!(world.get(handle, "Sprite", "flip_y"), Ok(Value::Bool(true))),
            layer,
            frame: animation.frame,
            animation,
        });
    }
    // Painter's order: the layer decides who is on top, and the id breaks ties the same way on
    // every machine, so a scene draws the same picture every frame.
    sprites.sort_by_key(|sprite| (sprite.layer, sprite.entity));

    let lighting = extract_lighting(world, camera.position);
    RenderFrame {
        camera,
        items,
        sprites,
        textures: Vec::new(),
        sculpts: Vec::new(),
        overlay: Vec::new(),
        show_grid: false,
        grid_height: 0.0,
        clear_color: Color::rgb(0.53, 0.70, 0.88),
        lighting,
        seconds: 0.0,
    }
}

/// Reads the `SpriteAnimation` row of a sprite (DR-39); without the row the whole picture is frame
/// 0. Bad numbers park the sprite instead of dropping it, and the Project Doctor reports them.
fn animation_of(world: &World, handle: EntityHandle) -> SpriteAnimation {
    if world.get(handle, "SpriteAnimation", "frames").is_err() {
        return SpriteAnimation::single();
    }
    let count = |column: &str, fallback: i64| {
        let value = match world.get(handle, "SpriteAnimation", column) {
            Ok(Value::Int(value)) => *value,
            _ => fallback,
        };
        u32::try_from(value.clamp(1, i64::from(MAX_SPRITE_FRAMES))).unwrap_or(1)
    };
    let flag = |column: &str, fallback: bool| match world.get(handle, "SpriteAnimation", column) {
        Ok(Value::Bool(value)) => *value,
        _ => fallback,
    };
    let fps = match world.get(handle, "SpriteAnimation", "fps") {
        Ok(Value::Float(value)) => *value as f32,
        _ => 0.0,
    };
    let frames = count("frames", 1);
    let frame = u32::try_from(match world.get(handle, "SpriteAnimation", "frame") {
        Ok(Value::Int(value)) => *value,
        _ => 0,
    })
    .unwrap_or(0);
    SpriteAnimation {
        frames,
        columns: count("columns", 1),
        fps,
        looping: flag("loop", true),
        playing: flag("playing", true),
        frame: frame.min(frames - 1),
    }
    .sanitized()
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

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_world::{TableSpec, ValueKind};

    fn empty_world() -> World {
        let mut world = World::new(11);
        world
            .define_table(
                TableSpec::new("Transform")
                    .column("position", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("scale", ValueKind::Vec3, Value::Vec3(Vec3::ONE)),
            )
            .expect("the Transform table");
        world
            .define_table(
                TableSpec::new("Sprite")
                    .column("asset", ValueKind::Text, Value::Text(String::new()))
                    .column("size", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
                    .column("color", ValueKind::Color, Value::Color(Color::WHITE))
                    .column("layer", ValueKind::Int, Value::Int(0))
                    .column("flip_x", ValueKind::Bool, Value::Bool(false))
                    .column("flip_y", ValueKind::Bool, Value::Bool(false)),
            )
            .expect("the Sprite table");
        world
    }

    /// Adds one sprite and returns its persistent id.
    fn add_sprite(world: &mut World, asset: &str, size: (f32, f32), layer: i64) -> u64 {
        let handle = world.spawn().expect("an entity");
        world
            .insert_row_values(
                handle,
                "Sprite",
                vec![
                    Value::Text(asset.to_string()),
                    Value::Vec3(Vec3::new(size.0, size.1, 0.0)),
                    Value::Color(Color::WHITE),
                    Value::Int(layer),
                    Value::Bool(false),
                    Value::Bool(false),
                ],
            )
            .expect("a Sprite row");
        world
            .insert_row_values(
                handle,
                "Transform",
                vec![
                    Value::Vec3(Vec3::ZERO),
                    Value::Vec3(Vec3::ZERO),
                    Value::Vec3(Vec3::ONE),
                ],
            )
            .expect("a Transform row");
        world.persistent_id(handle).expect("an id").0
    }

    #[test]
    fn sprites_are_sorted_by_layer_and_then_by_id() {
        let mut world = empty_world();
        add_sprite(&mut world, "front.png", (1.0, 1.0), 2);
        add_sprite(&mut world, "back.png", (1.0, 1.0), 0);
        add_sprite(&mut world, "middle.png", (1.0, 1.0), 1);
        let frame = extract_frame(&world, Camera::default());
        let assets: Vec<&str> = frame
            .sprites
            .iter()
            .map(|sprite| sprite.asset.as_str())
            .collect();
        assert_eq!(assets, ["back.png", "middle.png", "front.png"]);
    }

    #[test]
    fn a_sprite_carries_its_size_and_flip_flags() {
        let mut world = empty_world();
        add_sprite(&mut world, "hero.png", (2.0, 3.0), 0);
        let frame = extract_frame(&world, Camera::default());
        let sprite = frame.sprites.first().expect("one sprite");
        assert_eq!((sprite.width, sprite.height), (2.0, 3.0));
        assert!(!sprite.flip_x);
        assert!(!sprite.flip_y);
        assert_eq!(sprite.texture, None);
        // Without an animation row the whole picture is the frame.
        assert_eq!(sprite.frame, 0);
        assert_eq!(sprite.uv(), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn a_sprite_without_a_size_is_skipped() {
        let mut world = empty_world();
        add_sprite(&mut world, "flat.png", (0.0, 1.0), 0);
        add_sprite(&mut world, "negativa.png", (1.0, -1.0), 0);
        let frame = extract_frame(&world, Camera::default());
        assert!(frame.sprites.is_empty(), "{:?}", frame.sprites);
    }

    #[test]
    fn resolving_textures_points_the_sprite_at_the_pixels() {
        let mut world = empty_world();
        add_sprite(&mut world, "assets/hero.png", (1.0, 1.0), 0);
        add_sprite(&mut world, "assets/missing.png", (1.0, 1.0), 0);
        let mut frame = extract_frame(&world, Camera::default());
        let mut store = TextureStore::new();
        let id = store
            .insert("assets/hero.png", 1, 1, vec![10, 20, 30, 255])
            .expect("one pixel");
        frame.resolve_textures(&store);
        assert_eq!(frame.sprites[0].texture, Some(id));
        assert_eq!(frame.sprites[1].texture, None);
        assert_eq!(frame.textures.len(), 1);
        assert_eq!(frame.textures[0].rgba, [10, 20, 30, 255]);
    }

    #[test]
    fn selecting_an_entity_marks_its_sprite() {
        let mut world = empty_world();
        let chosen = add_sprite(&mut world, "hero.png", (1.0, 1.0), 0);
        add_sprite(&mut world, "foe.png", (1.0, 1.0), 0);
        let mut frame = extract_frame(&world, Camera::default());
        frame.mark_selected(Some(chosen));
        assert!(frame.sprites[0].selected);
        assert!(!frame.sprites[1].selected);
    }

    /// Four frames in two columns at 8 per second, playing.
    fn walking_sheet() -> SpriteAnimation {
        SpriteAnimation {
            frames: 4,
            columns: 2,
            fps: 8.0,
            looping: true,
            playing: true,
            frame: 0,
        }
    }

    #[test]
    fn a_sheet_plays_with_the_clock_and_loops_or_holds_the_last_frame() {
        let looping = walking_sheet();
        assert_eq!(looping.frame_at(0.0), 0, "a clock at rest shows frame 0");
        assert_eq!(looping.frame_at(0.1), 0, "0.8 frames played");
        assert_eq!(looping.frame_at(0.13), 1, "1.04 frames played");
        assert_eq!(looping.frame_at(0.5), 0, "4 frames played wraps around");
        // Without looping it stops on the last frame and stays there.
        let once = SpriteAnimation {
            looping: false,
            ..looping
        };
        assert_eq!(once.frame_at(0.5), 3);
        assert_eq!(once.frame_at(60.0), 3);
    }

    #[test]
    fn a_paused_sheet_shows_the_frame_stored_in_the_row() {
        let paused = SpriteAnimation {
            playing: false,
            frame: 2,
            ..walking_sheet()
        };
        assert_eq!(paused.frame_at(5.0), 2);
        // A clock that is not running parks it too, whatever the row says.
        let playing = SpriteAnimation {
            playing: true,
            ..paused
        };
        for seconds in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(playing.frame_at(seconds), 2, "clock {seconds}");
        }
        // Parked at rest means the editor draws the same picture every frame.
        assert_eq!(paused.sanitized().frame_at(9.0), 2);
    }

    #[test]
    fn hostile_animation_numbers_park_the_sprite_instead_of_breaking_it() {
        let broken = SpriteAnimation {
            frames: 0,
            columns: 0,
            fps: f32::NAN,
            looping: true,
            playing: true,
            frame: 99,
        }
        .sanitized();
        assert_eq!((broken.frames, broken.columns), (1, 1));
        assert_eq!(broken.fps, 0.0);
        assert_eq!(broken.frame, 0);
        assert_eq!(broken.frame_at(3.0), 0, "no fps means no playing");

        let huge = SpriteAnimation {
            frames: u32::MAX,
            columns: u32::MAX,
            fps: 1.0e9,
            ..broken
        }
        .sanitized();
        assert_eq!(huge.frames, MAX_SPRITE_FRAMES);
        assert_eq!(huge.columns, MAX_SPRITE_COLUMNS);
        assert_eq!(huge.fps, MAX_SPRITE_FPS);
    }

    #[test]
    fn the_frame_rectangle_cuts_the_sheet_into_a_grid_and_mirrors_inside_it() {
        // Six frames in three columns: two rows, and the first row is the top of the picture.
        let sheet = SpriteAnimation {
            frames: 6,
            columns: 3,
            ..walking_sheet()
        };
        assert_eq!(sheet.rows(), 2);
        let third = 1.0_f32 / 3.0;
        assert_eq!(sheet.uv(0, false, false), [0.0, 0.0, third, 0.5]);
        assert_eq!(sheet.uv(3, false, false), [0.0, 0.5, third, 1.0]);
        // The mirrored frame swaps its ends, so the mirroring stays inside its own picture.
        assert_eq!(sheet.uv(3, true, true), [third, 1.0, 0.0, 0.5]);
        // A frame past the end draws the last one instead of sampling the void.
        assert_eq!(sheet.uv(99, false, false), sheet.uv(5, false, false));
    }

    #[test]
    fn a_sprite_draws_the_frame_of_its_animation_row_at_the_clock() {
        let mut world = empty_world();
        // The row is the only thing that makes a sprite animated: no new object type.
        world
            .define_table(
                TableSpec::new("SpriteAnimation")
                    .column("frames", ValueKind::Int, Value::Int(1))
                    .column("columns", ValueKind::Int, Value::Int(1))
                    .column("fps", ValueKind::Float, Value::Float(8.0))
                    .column("loop", ValueKind::Bool, Value::Bool(true))
                    .column("playing", ValueKind::Bool, Value::Bool(true))
                    .column("frame", ValueKind::Int, Value::Int(0)),
            )
            .expect("the SpriteAnimation table");
        let handle = world.spawn().expect("an entity");
        world
            .insert_row_values(
                handle,
                "Transform",
                vec![
                    Value::Vec3(Vec3::ZERO),
                    Value::Vec3(Vec3::ZERO),
                    Value::Vec3(Vec3::ONE),
                ],
            )
            .expect("a Transform row");
        world
            .insert_row_values(
                handle,
                "Sprite",
                vec![
                    Value::Text("assets/andar.png".to_string()),
                    Value::Vec3(Vec3::new(1.0, 1.0, 0.0)),
                    Value::Color(Color::WHITE),
                    Value::Int(0),
                    Value::Bool(false),
                    Value::Bool(false),
                ],
            )
            .expect("a Sprite row");
        world
            .insert_row_values(
                handle,
                "SpriteAnimation",
                vec![
                    Value::Int(4),
                    Value::Int(2),
                    Value::Float(4.0),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::Int(0),
                ],
            )
            .expect("a SpriteAnimation row");

        let mut frame = extract_frame(&world, Camera::default());
        assert_eq!(frame.sprites[0].frame, 0, "at rest the row decides");
        assert_eq!(frame.sprites[0].uv(), [0.0, 0.0, 0.5, 0.5]);
        // 0.6 s at 4 fps is 2.4 frames played: the third frame, on the second row.
        frame.animate_sprites(0.6);
        assert_eq!(frame.sprites[0].frame, 2);
        assert_eq!(frame.sprites[0].uv(), [0.0, 0.5, 0.5, 1.0]);
        assert_eq!(
            frame.seconds, 0.6,
            "the frame carries the clock it was built from"
        );
    }
}
