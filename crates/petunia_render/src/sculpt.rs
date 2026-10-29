//! Sculpted volumes in the frame: what to draw, and the cache that keeps their meshes between
//! frames. A [`Sculpture`] per volume holds the baked field and the chunk meshes; asking for the
//! items again with the same strokes costs a comparison, and a new stroke re-meshes only the
//! chunks around it.

use std::collections::{BTreeMap, HashSet};

use petunia_math::{Color, Transform, Vec3};
use petunia_sculpt::{
    Base, ChunkMesh, Sculpture, Stamp, StampOperation, Stroke, StrokeMode, Volume,
};
use petunia_world::{EntityHandle, Value, World};

use crate::camera::model_matrix;
use crate::pick::{Ray, hit_triangle};

/// A volume as the world describes it, before meshing.
#[derive(Clone, Debug, PartialEq)]
pub struct SculptVolume {
    pub entity: u64,
    pub transform: Transform,
    pub color: Color,
    /// Flat-shaded facets (low poly) instead of smooth shading.
    pub faceted: bool,
    pub volume: Volume,
}

/// One sculpted volume to draw: its chunk meshes, each with an `id` that changes when the chunk
/// is re-meshed (a renderer caches GPU buffers by it).
#[derive(Clone, Debug, PartialEq)]
pub struct SculptItem {
    pub entity: u64,
    pub selected: bool,
    pub hovered: bool,
    pub transform: Transform,
    pub color: Color,
    pub faceted: bool,
    pub chunks: Vec<ChunkMesh>,
}

fn float(world: &World, handle: EntityHandle, table: &str, column: &str) -> Option<f32> {
    match world.get(handle, table, column) {
        Ok(Value::Float(value)) => Some(*value as f32),
        _ => None,
    }
}

fn vector(world: &World, handle: EntityHandle, table: &str, column: &str) -> Option<Vec3> {
    match world.get(handle, table, column) {
        Ok(Value::Vec3(value)) => Some(*value),
        _ => None,
    }
}

/// Every `Sculpt` with its strokes and stamps, in creation order. Strokes that point to no volume,
/// or carry an unknown mode, are left out (the Project Doctor reports them).
pub fn extract_sculpt_volumes(world: &World) -> Vec<SculptVolume> {
    let mut strokes: BTreeMap<u64, Vec<(u64, Stroke)>> = BTreeMap::new();
    for handle in world
        .entities_with_table("SculptStroke")
        .unwrap_or_default()
    {
        let Ok(identity) = world.persistent_id(handle) else {
            continue;
        };
        let Ok(Value::Int(volume)) = world.get(handle, "SculptStroke", "volume") else {
            continue;
        };
        let Ok(Value::Text(mode)) = world.get(handle, "SculptStroke", "mode") else {
            continue;
        };
        let (Some(mode), Some(start), Some(end), Some(radius), Some(smooth)) = (
            StrokeMode::parse(mode),
            vector(world, handle, "SculptStroke", "start"),
            vector(world, handle, "SculptStroke", "end"),
            float(world, handle, "SculptStroke", "radius"),
            float(world, handle, "SculptStroke", "smooth"),
        ) else {
            continue;
        };
        let Ok(volume) = u64::try_from(*volume) else {
            continue;
        };
        strokes.entry(volume).or_default().push((
            identity.0,
            Stroke {
                mode,
                start,
                end,
                radius,
                smooth,
                strength: float(world, handle, "SculptStroke", "strength").unwrap_or(0.0),
            },
        ));
    }
    let mut stamps: BTreeMap<u64, Vec<(u64, Stamp)>> = BTreeMap::new();
    for handle in world.entities_with_table("SculptStamp").unwrap_or_default() {
        let Ok(identity) = world.persistent_id(handle) else {
            continue;
        };
        let Ok(Value::Int(volume)) = world.get(handle, "SculptStamp", "volume") else {
            continue;
        };
        let Ok(volume) = u64::try_from(*volume) else {
            continue;
        };
        let Ok(Value::Text(mode)) = world.get(handle, "SculptStamp", "operation") else {
            continue;
        };
        let Some(operation) = StampOperation::parse(mode) else {
            continue;
        };
        let Ok(Value::Mesh(geometry)) = world.get(handle, "SculptStamp", "geometry") else {
            continue;
        };
        let Some(smooth) = float(world, handle, "SculptStamp", "smooth") else {
            continue;
        };
        let triangles: Vec<[Vec3; 3]> = geometry
            .triangles()
            .iter()
            .filter_map(|face| {
                let point = |slot: usize| {
                    face.get(slot)
                        .and_then(|vertex| geometry.positions.get(*vertex as usize))
                        .copied()
                };
                Some([point(0)?, point(1)?, point(2)?])
            })
            .collect();
        stamps.entry(volume).or_default().push((
            identity.0,
            Stamp {
                operation,
                triangles,
                smooth,
            },
        ));
    }
    let mut volumes = Vec::new();
    for handle in world.entities_with_table("Sculpt").unwrap_or_default() {
        let Ok(identity) = world.persistent_id(handle) else {
            continue;
        };
        let base = match world.get(handle, "Sculpt", "base") {
            Ok(Value::Text(name)) => Base::parse(name).unwrap_or(Base::Solid),
            _ => Base::Solid,
        };
        let mut own = strokes.remove(&identity.0).unwrap_or_default();
        own.sort_by_key(|(order, _)| *order);
        let mut own_stamps = stamps.remove(&identity.0).unwrap_or_default();
        own_stamps.sort_by_key(|(order, _)| *order);
        let color = match world.get(handle, "Sculpt", "color") {
            Ok(Value::Color(color)) => *color,
            _ => Color::WHITE,
        };
        volumes.push(SculptVolume {
            entity: identity.0,
            transform: Transform {
                position: vector(world, handle, "Transform", "position").unwrap_or(Vec3::ZERO),
                rotation_degrees: vector(world, handle, "Transform", "rotation_degrees")
                    .unwrap_or(Vec3::ZERO),
                scale: vector(world, handle, "Transform", "scale").unwrap_or(Vec3::ONE),
            },
            color,
            faceted: matches!(
                world.get(handle, "Sculpt", "faceted"),
                Ok(Value::Bool(true))
            ),
            volume: Volume {
                base,
                size: vector(world, handle, "Sculpt", "size").unwrap_or(Vec3::ONE),
                cell_size: float(world, handle, "Sculpt", "cell_size").unwrap_or(0.5),
                strokes: own.into_iter().map(|(_, stroke)| stroke).collect(),
                stamps: own_stamps.into_iter().map(|(_, stamp)| stamp).collect(),
            },
        });
    }
    volumes
}

/// One [`Sculpture`] per volume, kept between frames. Volumes that disappear are forgotten.
#[derive(Default)]
pub struct SculptCache {
    sculptures: BTreeMap<u64, Sculpture>,
}

impl SculptCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Brings every sculpture up to date and returns what to draw. A volume that cannot be
    /// meshed (bad numbers) is drawn empty; the Project Doctor says why.
    pub fn items(&mut self, volumes: &[SculptVolume]) -> Vec<SculptItem> {
        let live: HashSet<u64> = volumes.iter().map(|volume| volume.entity).collect();
        self.sculptures.retain(|entity, _| live.contains(entity));
        volumes
            .iter()
            .map(|volume| {
                let sculpture = self.sculptures.entry(volume.entity).or_default();
                if sculpture.update(&volume.volume).is_err() {
                    *sculpture = Sculpture::new();
                }
                SculptItem {
                    entity: volume.entity,
                    selected: false,
                    hovered: false,
                    transform: volume.transform,
                    color: volume.color,
                    faceted: volume.faceted,
                    chunks: sculpture.chunks().cloned().collect(),
                }
            })
            .collect()
    }
}

/// Where the ray meets the volume's surface: distance along the ray and the normal facing it.
/// The ray is carried into the volume's own space, so each chunk is first rejected by its box and
/// only the triangles of the chunks the ray crosses are tested.
pub fn hit_sculpt(item: &SculptItem, ray: &Ray) -> Option<(f32, Vec3)> {
    let model = model_matrix(&item.transform);
    let column = |index: usize| {
        Vec3::new(
            model.get(index * 4).copied().unwrap_or(0.0),
            model.get(index * 4 + 1).copied().unwrap_or(0.0),
            model.get(index * 4 + 2).copied().unwrap_or(0.0),
        )
    };
    let (axis_x, axis_y, axis_z) = (column(0), column(1), column(2));
    let (scale_x, scale_y, scale_z) = (axis_x.length(), axis_y.length(), axis_z.length());
    if scale_x < 1.0e-6 || scale_y < 1.0e-6 || scale_z < 1.0e-6 {
        return None;
    }
    // The linear part is R * S: its inverse is S^-1 * R^T. The direction is not renormalized, so
    // distances along the local ray equal distances along the world ray.
    let (rotation_x, rotation_y, rotation_z) = (
        axis_x * (1.0 / scale_x),
        axis_y * (1.0 / scale_y),
        axis_z * (1.0 / scale_z),
    );
    let to_local = |vector: Vec3| {
        Vec3::new(
            vector.dot(rotation_x) / scale_x,
            vector.dot(rotation_y) / scale_y,
            vector.dot(rotation_z) / scale_z,
        )
    };
    let local = Ray {
        origin: to_local(ray.origin - item.transform.position),
        direction: to_local(ray.direction),
    };
    let mut best: Option<(f32, Vec3)> = None;
    for chunk in &item.chunks {
        if !ray_meets_box(&local, chunk.min, chunk.max) {
            continue;
        }
        let mesh = &chunk.mesh;
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let corner = |slot: usize| {
                triangle
                    .get(slot)
                    .and_then(|index| mesh.positions.get(*index as usize))
                    .map(|[x, y, z]| Vec3::new(*x, *y, *z))
            };
            let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
                continue;
            };
            if let Some(distance) = hit_triangle(&local, a, b, c)
                && best.is_none_or(|(nearest, _)| distance < nearest)
            {
                best = Some((distance, (b - a).cross(c - a)));
            }
        }
    }
    best.map(|(distance, local_normal)| {
        // Normals transform by the inverse transpose: R * S^-1 applied to the local normal.
        let scaled = Vec3::new(
            local_normal.x / scale_x,
            local_normal.y / scale_y,
            local_normal.z / scale_z,
        );
        let mut normal = (rotation_x * scaled.x + rotation_y * scaled.y + rotation_z * scaled.z)
            .normalized_or_zero();
        if normal.dot(ray.direction) > 0.0 {
            normal = -normal;
        }
        (distance, normal)
    })
}

/// Slab test: does the ray cross the box at all?
fn ray_meets_box(ray: &Ray, low: Vec3, high: Vec3) -> bool {
    let mut enter = f32::MIN;
    let mut exit = f32::MAX;
    for (origin, direction, near, far) in [
        (ray.origin.x, ray.direction.x, low.x, high.x),
        (ray.origin.y, ray.direction.y, low.y, high.y),
        (ray.origin.z, ray.direction.z, low.z, high.z),
    ] {
        if direction.abs() < 1.0e-12 {
            if origin < near || origin > far {
                return false;
            }
            continue;
        }
        let (first, second) = ((near - origin) / direction, (far - origin) / direction);
        enter = enter.max(first.min(second));
        exit = exit.min(first.max(second));
    }
    enter <= exit && exit >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(transform: Transform) -> SculptItem {
        let volume = Volume {
            base: Base::Solid,
            size: Vec3::new(6.0, 6.0, 6.0),
            cell_size: 0.5,
            strokes: Vec::new(),
            stamps: Vec::new(),
        };
        let mut cache = SculptCache::new();
        cache
            .items(&[SculptVolume {
                entity: 1,
                transform,
                color: Color::WHITE,
                faceted: false,
                volume,
            }])
            .remove(0)
    }

    #[test]
    fn a_ray_hits_the_block_at_the_right_distance_with_the_normal_facing_it() {
        let plain = item(Transform::IDENTITY);
        let ray = Ray {
            origin: Vec3::new(0.2, 0.3, 10.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
        };
        let (distance, normal) = hit_sculpt(&plain, &ray).expect("hit");
        assert!((distance - 7.0).abs() < 0.2, "{distance}");
        assert!(normal.dot(Vec3::new(0.0, 0.0, 1.0)) > 0.9, "{normal:?}");
    }

    #[test]
    fn moving_and_scaling_the_volume_moves_and_scales_the_hit() {
        let moved = item(Transform {
            position: Vec3::new(5.0, 0.0, 0.0),
            scale: Vec3::new(2.0, 1.0, 2.0),
            ..Transform::IDENTITY
        });
        let ray = Ray {
            origin: Vec3::new(5.0, 0.0, 20.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
        };
        let (distance, _) = hit_sculpt(&moved, &ray).expect("hit");
        assert!(
            (distance - (20.0 - 6.0)).abs() < 0.3,
            "{distance}: front face at z = 6"
        );
        let miss = Ray {
            origin: Vec3::new(-20.0, 0.0, 20.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
        };
        assert!(hit_sculpt(&moved, &miss).is_none());
    }

    #[test]
    fn asking_again_with_the_same_strokes_does_not_re_mesh() {
        let volume = SculptVolume {
            entity: 9,
            transform: Transform::IDENTITY,
            color: Color::WHITE,
            faceted: false,
            volume: Volume {
                base: Base::Solid,
                size: Vec3::new(20.0, 6.0, 20.0),
                cell_size: 0.5,
                strokes: Vec::new(),
                stamps: Vec::new(),
            },
        };
        let mut cache = SculptCache::new();
        let first: Vec<u64> = cache.items(std::slice::from_ref(&volume))[0]
            .chunks
            .iter()
            .map(|chunk| chunk.id)
            .collect();
        let again: Vec<u64> = cache.items(&[volume])[0]
            .chunks
            .iter()
            .map(|chunk| chunk.id)
            .collect();
        assert!(!first.is_empty());
        assert_eq!(first, again, "same mesh ids: nothing was rebuilt");
    }
}
