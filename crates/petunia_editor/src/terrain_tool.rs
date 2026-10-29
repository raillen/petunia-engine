//! Terrain sculpting in the editor.
//!
//! A drag is previewed **live** on the world and saved as **one** `terrain.patch` command: the
//! session keeps the original terrain, applies dabs while the mouse moves and, on release,
//! restores the original and replays the final chunks through the command dispatcher. Cancel
//! restores the original and leaves no history.
//!
//! The brush is a 3D volume in world space, so it works from any camera angle. The cursor ring is
//! drawn by the overlay; a host may also dim the surface (X-ray) while a drag is active.

use std::collections::BTreeSet;

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_math::{Color, Vec3};
use petunia_render::{OverlayItem, Ray};
use petunia_terrain::{Brush, BrushShape, ChunkCoord, Falloff, Terrain, TerrainOperation};
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::{EditorSession, vector_json};
use crate::tools::Pointer;

const RING_SEGMENTS: usize = 40;
const RING_COLOR: Color = Color::new(0.35, 0.85, 1.0, 1.0);
/// A new dab is applied once the mouse has moved this fraction of the radius.
const DAB_SPACING: f32 = 0.35;
const MAX_RAY_DISTANCE: f32 = 800.0;
const MIN_RADIUS: f32 = 0.25;
const MAX_RADIUS: f32 = 64.0;

pub(crate) struct TerrainDrag {
    entity: u64,
    /// The terrain before the first dab, used to undo a cancel and to build the patch.
    original: Terrain,
    touched: BTreeSet<ChunkCoord>,
    last_center: Option<Vec3>,
}

/// Per-session state of the terrain tool.
pub(crate) struct TerrainState {
    pub drag: Option<TerrainDrag>,
    /// Surface point and normal under the mouse.
    pub cursor: Option<(Vec3, Vec3)>,
    pub radius: f32,
    pub strength: f32,
    pub falloff: Falloff,
    /// `raise`, `lower`, `smooth`, `flatten`, `noise` or `terrace`.
    pub operation: String,
    /// `sphere`, `box`, `cylinder` or `capsule`.
    pub shape: String,
    pub material: u8,
    /// Paint a material layer instead of changing heights.
    pub paint: bool,
    /// Apply the brush mirrored across the world x = 0 and z = 0 planes.
    pub mirror_x: bool,
    pub mirror_z: bool,
    /// Discard geometry above `section_height` (X-ray/cut view).
    pub section: bool,
    pub section_height: f32,
}

impl TerrainState {
    pub fn new() -> Self {
        Self {
            drag: None,
            cursor: None,
            radius: 2.0,
            strength: 1.0,
            falloff: Falloff::Smooth,
            operation: "raise".to_string(),
            shape: "sphere".to_string(),
            material: 0,
            paint: false,
            mirror_x: false,
            mirror_z: false,
            section: false,
            section_height: 0.0,
        }
    }

    pub(crate) fn reset_transient(&mut self) {
        self.drag = None;
        self.cursor = None;
    }
}

fn falloff_name(falloff: Falloff) -> &'static str {
    match falloff {
        Falloff::Constant => "constant",
        Falloff::Linear => "linear",
        Falloff::Smooth => "smooth",
        Falloff::Gaussian => "gaussian",
    }
}

fn parse_falloff(name: &str) -> Option<Falloff> {
    match name {
        "constant" => Some(Falloff::Constant),
        "linear" => Some(Falloff::Linear),
        "smooth" => Some(Falloff::Smooth),
        "gaussian" => Some(Falloff::Gaussian),
        _ => None,
    }
}

fn shape_of(name: &str, radius: f32) -> BrushShape {
    match name {
        "box" => BrushShape::Box {
            half_extents: Vec3::splat(radius),
        },
        "cylinder" => BrushShape::Cylinder {
            radius,
            half_height: radius,
        },
        "capsule" => BrushShape::Capsule {
            radius,
            half_height: radius,
        },
        _ => BrushShape::Sphere { radius },
    }
}

fn chunk_json(chunk: &petunia_terrain::TerrainChunk) -> Json {
    json!({
        "x": chunk.coord().x,
        "z": chunk.coord().z,
        "heights": chunk.heights(),
        "materials": chunk.materials(),
    })
}

fn removed_chunk_json(coord: ChunkCoord) -> Json {
    json!({ "x": coord.x, "z": coord.z, "remove": true })
}

/// Marches along the ray until it goes below the terrain surface, then bisects.
fn raycast_terrain(ray: &Ray, terrain: &Terrain, entity: u64) -> Option<(u64, Vec3, Vec3, f32)> {
    let step = terrain.cell_size().max(0.25);
    let mut previous = 0.0;
    let mut distance = 0.0;
    while distance <= MAX_RAY_DISTANCE {
        let point = ray.origin + ray.direction * distance;
        let Some(height) = terrain.height_at(point.x, point.z) else {
            previous = distance;
            distance += step;
            continue;
        };
        if point.y <= height {
            let mut low = previous;
            let mut high = distance;
            for _ in 0..16 {
                let middle = (low + high) * 0.5;
                let sample = ray.origin + ray.direction * middle;
                let Some(sample_height) = terrain.height_at(sample.x, sample.z) else {
                    break;
                };
                if sample.y > sample_height {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let hit_distance = (low + high) * 0.5;
            let hit = ray.origin + ray.direction * hit_distance;
            let height = terrain.height_at(hit.x, hit.z).unwrap_or(hit.y);
            let normal = terrain.normal_at(hit.x, hit.z).unwrap_or(Vec3::UP);
            return Some((
                entity,
                Vec3::new(hit.x, height, hit.z),
                normal,
                hit_distance,
            ));
        }
        previous = distance;
        distance += step;
    }
    None
}

impl EditorSession {
    /// The terrain surface under a ray, nearest first.
    pub(crate) fn terrain_hit(&self, ray: &Ray) -> Option<(u64, Vec3, Vec3, f32)> {
        let world = self.world().ok()?;
        let mut best: Option<(u64, Vec3, Vec3, f32)> = None;
        for handle in world.entities_with_table("Terrain").unwrap_or_default() {
            let Ok(Value::Terrain(terrain)) = world.get(handle, "Terrain", "data") else {
                continue;
            };
            let entity = world.persistent_id(handle).map_or(0, |id| id.0);
            let Some(hit) = raycast_terrain(ray, terrain, entity) else {
                continue;
            };
            if best.is_none_or(|(_, _, _, distance)| hit.3 < distance) {
                best = Some(hit);
            }
        }
        best
    }

    /// Updates the brush cursor without starting a drag (the ring follows the mouse).
    pub fn terrain_hover(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let view = self.view_for(pointer);
        let Some(ray) = view.ray(pointer.mouse()) else {
            return Ok(self.terrain_json());
        };
        self.terrain.cursor = self
            .terrain_hit(&ray)
            .map(|(_, point, normal, _)| (point, normal));
        Ok(self.terrain_json())
    }

    /// Turns the X-ray view on or off (the terrain becomes translucent while the tool is active).
    pub fn set_terrain_xray(&mut self, enabled: bool) -> Json {
        self.terrain_xray = enabled;
        json!({ "terrain_xray": enabled })
    }

    /// Enables the horizontal cut plane. Without an explicit height it uses the brush cursor.
    pub fn set_terrain_section(&mut self, enabled: bool, height: Option<f32>) -> Json {
        self.terrain.section = enabled;
        if let Some(height) = height.filter(|value| value.is_finite()) {
            self.terrain.section_height = height;
        } else if let Some((point, _)) = self.terrain.cursor {
            self.terrain.section_height = point.y;
        }
        json!({
            "enabled": self.terrain.section,
            "height": self.terrain.section_height,
        })
    }

    pub fn terrain_begin(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let view = self.view_for(pointer);
        let ray = view
            .ray(pointer.mouse())
            .ok_or_else(|| Failure::bad_request("o ponteiro está fora do viewport"))?;
        let Some((entity, point, normal, _)) = self.terrain_hit(&ray) else {
            return Err(Failure::new(
                "TERRAIN_NO_HIT",
                "não há terreno sob o mouse",
                "Crie um em Criar > Terreno, ou clique sobre ele.",
            ));
        };
        let original = {
            let world = self.world()?;
            let handle = world.handle_of(PersistentId(entity))?;
            match world.get(handle, "Terrain", "data") {
                Ok(Value::Terrain(terrain)) => terrain.clone(),
                _ => {
                    return Err(Failure::new(
                        "TERRAIN_INVALID",
                        "a entidade não tem um terreno",
                        "Selecione um terreno criado com Criar > Terreno.",
                    ));
                }
            }
        };
        self.selection = Some(entity);
        self.terrain.drag = Some(TerrainDrag {
            entity,
            original,
            touched: BTreeSet::new(),
            last_center: None,
        });
        self.terrain.cursor = Some((point, normal));
        self.terrain_apply(point, normal)
    }

    pub fn terrain_drag(&mut self, pointer: Pointer) -> Result<Json, Failure> {
        let Some(entity) = self.terrain.drag.as_ref().map(|drag| drag.entity) else {
            return Ok(self.terrain_json());
        };
        let view = self.view_for(pointer);
        let Some(ray) = view.ray(pointer.mouse()) else {
            return Ok(self.terrain_json());
        };
        let Some((hit_entity, point, normal, _)) = self.terrain_hit(&ray) else {
            self.terrain.cursor = None;
            return Ok(self.terrain_json());
        };
        if hit_entity != entity {
            return Ok(self.terrain_json());
        }
        self.terrain.cursor = Some((point, normal));
        let spaced = self.terrain.drag.as_ref().is_none_or(|drag| {
            drag.last_center
                .is_none_or(|last| (point - last).length() >= self.terrain.radius * DAB_SPACING)
        });
        if spaced {
            self.terrain_apply(point, normal)?;
        }
        Ok(self.terrain_json())
    }

    pub fn terrain_end(&mut self) -> Result<Json, Failure> {
        let Some(drag) = self.terrain.drag.take() else {
            return Ok(json!({ "changed": false }));
        };
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let handle = project.world.handle_of(PersistentId(drag.entity))?;
        let current = match project.world.get(handle, "Terrain", "data") {
            Ok(Value::Terrain(terrain)) => terrain.clone(),
            _ => drag.original.clone(),
        };
        let mut patch = Vec::new();
        let mut changed = false;
        for coord in &drag.touched {
            let before = drag.original.capture_chunk(*coord);
            let after = current.capture_chunk(*coord);
            if before != after {
                changed = true;
            }
            patch.push(match after {
                Some(chunk) => chunk_json(&chunk),
                None => removed_chunk_json(*coord),
            });
        }
        project
            .world
            .set(handle, "Terrain", "data", Value::Terrain(drag.original))?;
        if !changed {
            return Ok(json!({ "changed": false }));
        }
        let record = CommandRecord::new(
            "terrain.patch",
            json!({ "entity": drag.entity, "chunks": patch }),
        );
        // One undo step for the terrain and the objects that follow it: `surface.follow` returns a
        // plain `surface.set` inverse, so this batch never nests another batch.
        let follow = CommandRecord::new("surface.follow", json!({ "terrain": drag.entity }));
        let batch = CommandRecord::new(BATCH_COMMAND, json!({ "records": [record, follow] }));
        self.execute(&batch)?;
        Ok(json!({ "changed": true, "chunks": patch.len() }))
    }

    pub fn terrain_cancel(&mut self) -> Json {
        let Some(drag) = self.terrain.drag.take() else {
            return json!({ "changed": false });
        };
        if let Some(project) = self.project.as_mut()
            && let Ok(handle) = project.world.handle_of(PersistentId(drag.entity))
        {
            let _ = project
                .world
                .set(handle, "Terrain", "data", Value::Terrain(drag.original));
        }
        json!({ "changed": false })
    }

    pub fn terrain_set(&mut self, request: &Json) -> Result<Json, Failure> {
        if let Some(radius) = request.get("radius").and_then(Json::as_f64) {
            self.terrain.radius = (radius as f32).clamp(MIN_RADIUS, MAX_RADIUS);
        }
        if let Some(strength) = request.get("strength").and_then(Json::as_f64) {
            self.terrain.strength = (strength as f32).clamp(0.0, MAX_RADIUS);
        }
        if let Some(name) = request.get("falloff").and_then(Json::as_str) {
            self.terrain.falloff = parse_falloff(name).ok_or_else(|| {
                Failure::bad_request("`falloff` precisa ser constant, linear, smooth ou gaussian")
            })?;
        }
        if let Some(name) = request.get("operation").and_then(Json::as_str) {
            if !matches!(
                name,
                "raise" | "lower" | "smooth" | "flatten" | "noise" | "terrace"
            ) {
                return Err(Failure::bad_request(
                    "`operation` precisa ser raise, lower, smooth, flatten, noise ou terrace",
                ));
            }
            self.terrain.operation = name.to_string();
        }
        if let Some(name) = request.get("shape").and_then(Json::as_str) {
            if !matches!(name, "sphere" | "box" | "cylinder" | "capsule") {
                return Err(Failure::bad_request(
                    "`shape` precisa ser sphere, box, cylinder ou capsule",
                ));
            }
            self.terrain.shape = name.to_string();
        }
        if let Some(material) = request.get("material").and_then(Json::as_u64) {
            self.terrain.material = u8::try_from(material.min(255)).unwrap_or(0);
        }
        if let Some(paint) = request.get("paint").and_then(Json::as_bool) {
            self.terrain.paint = paint;
        }
        if let Some(mirror) = request.get("mirror_x").and_then(Json::as_bool) {
            self.terrain.mirror_x = mirror;
        }
        if let Some(mirror) = request.get("mirror_z").and_then(Json::as_bool) {
            self.terrain.mirror_z = mirror;
        }
        Ok(self.terrain_json())
    }

    pub fn terrain_json(&self) -> Json {
        json!({
            "radius": self.terrain.radius,
            "strength": self.terrain.strength,
            "falloff": falloff_name(self.terrain.falloff),
            "operation": self.terrain.operation,
            "shape": self.terrain.shape,
            "material": self.terrain.material,
            "paint": self.terrain.paint,
            "mirror_x": self.terrain.mirror_x,
            "mirror_z": self.terrain.mirror_z,
            "xray": self.terrain_xray,
            "section": self.terrain.section,
            "section_height": self.terrain.section_height,
            "active": self.terrain.drag.is_some(),
            "cursor": self.terrain.cursor.map(|(point, normal)| json!({
                "point": vector_json(point),
                "normal": vector_json(normal),
            })),
        })
    }

    fn terrain_apply(&mut self, point: Vec3, normal: Vec3) -> Result<Json, Failure> {
        let Some(entity) = self.terrain.drag.as_ref().map(|drag| drag.entity) else {
            return Ok(self.terrain_json());
        };
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let handle = project.world.handle_of(PersistentId(entity))?;
        let mut terrain = match project.world.get(handle, "Terrain", "data") {
            Ok(Value::Terrain(terrain)) => terrain.clone(),
            _ => return Ok(self.terrain_json()),
        };
        let radius = self.terrain.radius;
        let operation = match self.terrain.operation.as_str() {
            "lower" => TerrainOperation::Lower,
            "smooth" => TerrainOperation::Smooth,
            "flatten" => TerrainOperation::Flatten {
                target_height: Some(point.y),
            },
            "noise" => TerrainOperation::Noise {
                seed: 0,
                feature_size: (radius * 4.0).max(1.0),
                amplitude: self.terrain.strength,
            },
            "terrace" => TerrainOperation::Terrace {
                step: self.terrain.strength.max(0.05),
            },
            _ => TerrainOperation::Raise,
        };
        let mut centers = vec![point];
        if self.terrain.mirror_x {
            centers.push(Vec3::new(-point.x, point.y, point.z));
        }
        if self.terrain.mirror_z {
            centers.push(Vec3::new(point.x, point.y, -point.z));
        }
        if self.terrain.mirror_x && self.terrain.mirror_z {
            centers.push(Vec3::new(-point.x, point.y, -point.z));
        }
        let paint = self.terrain.paint;
        let material = self.terrain.material;
        let falloff = self.terrain.falloff;
        let strength = self.terrain.strength;
        let shape = self.terrain.shape.clone();
        let mut changed = 0;
        for center in centers {
            let brush = Brush::new(shape_of(&shape, radius), center, falloff, strength);
            let (minimum, maximum) = terrain.brush_chunk_region(&brush);
            changed += if paint {
                terrain.paint(&brush, material).map_err(terrain_failure)?
            } else {
                terrain.apply(&brush, operation).map_err(terrain_failure)?
            };
            if let Some(drag) = self.terrain.drag.as_mut() {
                for x in minimum.x..=maximum.x {
                    for z in minimum.z..=maximum.z {
                        drag.touched.insert(ChunkCoord::new(x, z));
                    }
                }
            }
        }
        project
            .world
            .set(handle, "Terrain", "data", Value::Terrain(terrain))?;
        if let Some(drag) = self.terrain.drag.as_mut() {
            drag.last_center = Some(point);
        }
        self.terrain.cursor = Some((point, normal));
        Ok(json!({ "changed": changed }))
    }

    pub(crate) fn terrain_overlay(&self, items: &mut Vec<OverlayItem>) {
        let Some((point, normal)) = self.terrain.cursor else {
            return;
        };
        let normal = normal.normalized_or_zero();
        let helper = if normal.y.abs() < 0.9 {
            Vec3::UP
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let side = normal.cross(helper).normalized_or_zero();
        let other = normal.cross(side);
        let lift = point + normal * 0.02;
        let radius = self.terrain.radius;
        let at = |step: usize| {
            let angle = step as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
            lift + (side * angle.cos() + other * angle.sin()) * radius
        };
        for step in 0..RING_SEGMENTS {
            items.push(OverlayItem::Line {
                from: at(step),
                to: at(step + 1),
                color: RING_COLOR,
                width_pixels: 2.0,
            });
        }
        items.push(OverlayItem::Line {
            from: lift,
            to: lift + normal * (radius * 0.5),
            color: RING_COLOR,
            width_pixels: 1.5,
        });
    }
}

fn terrain_failure(error: petunia_terrain::TerrainError) -> Failure {
    Failure::new(
        "TERRAIN_INVALID",
        &error.to_string(),
        "Ajuste o pincel ou a operação.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_terrain::{FractalSettings, TerrainGeneration};

    fn flat_terrain() -> Terrain {
        let mut terrain = Terrain::new(1.0, 8).unwrap();
        terrain
            .generate_region(
                ChunkCoord::new(0, 0),
                ChunkCoord::new(0, 0),
                &TerrainGeneration {
                    fractal: FractalSettings {
                        amplitude: 0.0,
                        ..FractalSettings::default()
                    },
                    base_height: 0.0,
                },
            )
            .unwrap();
        terrain
    }

    #[test]
    fn a_ray_from_above_hits_the_surface() {
        let terrain = flat_terrain();
        let ray = Ray {
            origin: Vec3::new(2.0, 10.0, 2.0),
            direction: Vec3::new(0.0, -1.0, 0.0),
        };
        let hit = raycast_terrain(&ray, &terrain, 7).expect("hit");
        assert_eq!(hit.0, 7);
        assert!(hit.1.y.abs() < 0.01, "{:?}", hit.1);
        assert!((hit.3 - 10.0).abs() < 0.01, "{}", hit.3);
        assert!(hit.2.y > 0.9);
    }

    #[test]
    fn a_ray_that_misses_returns_none() {
        let terrain = flat_terrain();
        let ray = Ray {
            origin: Vec3::new(-20.0, 10.0, -20.0),
            direction: Vec3::new(0.0, -1.0, 0.0),
        };
        assert!(raycast_terrain(&ray, &terrain, 7).is_none());
    }

    #[test]
    fn shape_and_falloff_names_round_trip() {
        assert_eq!(shape_of("sphere", 2.0), BrushShape::Sphere { radius: 2.0 });
        assert_eq!(
            shape_of("box", 2.0),
            BrushShape::Box {
                half_extents: Vec3::splat(2.0)
            }
        );
        assert_eq!(parse_falloff("gaussian"), Some(Falloff::Gaussian));
        assert_eq!(parse_falloff("cubic"), None);
    }
}
