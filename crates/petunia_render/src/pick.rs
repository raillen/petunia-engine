//! Picking: which entity is under a point of the viewport. Pure math, no GPU: the ray is tested
//! against the real triangles of every item, so ramps and cylinders pick as exactly as boxes.

use std::collections::HashMap;
use std::sync::Arc;

use petunia_math::Vec3;

use crate::camera::{Camera, Mat4, model_matrix};
use crate::frame::{DrawItem, Shape, SpriteItem};
use crate::mesh::{MeshData, item_mesh};

/// A ray in world space. `direction` has length one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// The ray that leaves the camera through a point of the viewport. `ndc_x` and `ndc_y` are in
/// `-1..=1` (left to right, bottom to top). Returns `None` for non-finite input.
pub fn ray_through(camera: &Camera, aspect_ratio: f32, ndc_x: f32, ndc_y: f32) -> Option<Ray> {
    if !(ndc_x.is_finite() && ndc_y.is_finite() && aspect_ratio.is_finite() && aspect_ratio > 0.0) {
        return None;
    }
    let forward = camera.forward();
    let side = {
        let cross = forward.cross(Vec3::UP).normalized_or_zero();
        if cross.length_squared() > 0.0 {
            cross
        } else {
            camera.right()
        }
    };
    let up = side.cross(forward).normalized_or_zero();
    match camera.projection_mode {
        crate::camera::ProjectionMode::Perspective => {
            let half_height = (camera.fov_degrees.to_radians() * 0.5).tan();
            let direction =
                forward + side * (ndc_x * half_height * aspect_ratio) + up * (ndc_y * half_height);
            let direction = direction.normalized_or_zero();
            if direction.length_squared() == 0.0 {
                return None;
            }
            Some(Ray {
                origin: camera.position,
                direction,
            })
        }
        crate::camera::ProjectionMode::Orthographic => {
            let half_height = (camera.orthographic_height * 0.5).max(0.05);
            let half_width = half_height * aspect_ratio.max(0.01);
            let origin = camera.position + side * (ndc_x * half_width) + up * (ndc_y * half_height);
            Some(Ray {
                origin,
                direction: forward,
            })
        }
    }
}

fn transform_point(matrix: &Mat4, point: [f32; 3]) -> Vec3 {
    let [x, y, z] = point;
    let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
    Vec3::new(
        entry(0) * x + entry(4) * y + entry(8) * z + entry(12),
        entry(1) * x + entry(5) * y + entry(9) * z + entry(13),
        entry(2) * x + entry(6) * y + entry(10) * z + entry(14),
    )
}

/// Moller-Trumbore, two-sided. Returns the distance along the ray.
pub(crate) fn hit_triangle(ray: &Ray, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    const EPSILON: f32 = 1.0e-7;
    let edge_ab = b - a;
    let edge_ac = c - a;
    let pivot = ray.direction.cross(edge_ac);
    let determinant = edge_ab.dot(pivot);
    if determinant.abs() < EPSILON {
        return None;
    }
    let inverse = 1.0 / determinant;
    let to_origin = ray.origin - a;
    let u = to_origin.dot(pivot) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let cross = to_origin.cross(edge_ab);
    let v = ray.direction.dot(cross) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge_ac.dot(cross) * inverse;
    (distance > 0.0).then_some(distance)
}

/// Where a ray met a surface: the entity, the distance along the ray and the surface normal
/// (unit, facing the ray).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceHit {
    pub entity: u64,
    pub distance: f32,
    pub normal: Vec3,
}

fn hit_mesh(ray: &Ray, mesh: &MeshData, model: &Mat4) -> Option<f32> {
    hit_mesh_with_normal(ray, mesh, model).map(|(distance, _)| distance)
}

pub(crate) fn hit_mesh_with_normal(
    ray: &Ray,
    mesh: &MeshData,
    model: &Mat4,
) -> Option<(f32, Vec3)> {
    let world: Vec<Vec3> = mesh
        .positions
        .iter()
        .map(|position| transform_point(model, *position))
        .collect();
    let mut nearest: Option<(f32, Vec3)> = None;
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let corner = |slot: usize| {
            triangle
                .get(slot)
                .and_then(|index| world.get(*index as usize))
                .copied()
        };
        let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
            continue;
        };
        if let Some(distance) = hit_triangle(ray, a, b, c)
            && nearest.is_none_or(|(best, _)| distance < best)
        {
            let mut normal = (b - a).cross(c - a).normalized_or_zero();
            if normal.dot(ray.direction) > 0.0 {
                normal = -normal;
            }
            nearest = Some((distance, normal));
        }
    }
    nearest
}

/// The first surface the ray meets, with its normal, skipping the entities in `ignore`.
pub fn pick_surface(items: &[DrawItem], ray: &Ray, ignore: &[u64]) -> Option<SurfaceHit> {
    let mut best: Option<SurfaceHit> = None;
    for item in items
        .iter()
        .filter(|item| item.entity != 0 && !ignore.contains(&item.entity))
    {
        let Ok(mesh) = item_mesh(item) else {
            continue;
        };
        let model = model_matrix(&item.transform);
        if let Some((distance, normal)) = hit_mesh_with_normal(ray, &mesh, &model)
            && best.is_none_or(|hit| distance < hit.distance)
        {
            best = Some(SurfaceHit {
                entity: item.entity,
                distance,
                normal,
            });
        }
    }
    best
}

/// World-space corners of every item's mesh except `ignore`: the targets of point snapping.
pub fn mesh_points(items: &[DrawItem], ignore: &[u64]) -> Vec<Vec3> {
    let mut points = Vec::new();
    for item in items
        .iter()
        .filter(|item| item.entity != 0 && !ignore.contains(&item.entity))
    {
        let Ok(mesh) = item_mesh(item) else {
            continue;
        };
        let model = model_matrix(&item.transform);
        for position in &mesh.positions {
            let point = transform_point(&model, *position);
            // Meshes repeat corners per face; keep each once.
            if !points
                .iter()
                .any(|known: &Vec3| (*known - point).length_squared() < 1.0e-8)
            {
                points.push(point);
            }
        }
    }
    points
}

/// Shape, size bits, detail and (for editable meshes) the fingerprint: equal keys share a mesh.
type MeshKey = (Shape, [u32; 3], u32, u64);

/// The entity whose surface the ray hits first, with the distance. Items with entity `0` and
/// items with an invalid size are skipped.
pub fn pick_entity(items: &[DrawItem], ray: &Ray) -> Option<(u64, f32)> {
    let mut meshes: HashMap<MeshKey, Option<Arc<MeshData>>> = HashMap::new();
    let mut best: Option<(u64, f32)> = None;
    for item in items.iter().filter(|item| item.entity != 0) {
        let key = (
            item.shape,
            [
                item.size.x.to_bits(),
                item.size.y.to_bits(),
                item.size.z.to_bits(),
            ],
            item.detail,
            item.mesh_fingerprint,
        );
        let mesh = meshes.entry(key).or_insert_with(|| item_mesh(item).ok());
        let Some(mesh) = mesh else {
            continue;
        };
        let model = model_matrix(&item.transform);
        if let Some(distance) = hit_mesh(ray, mesh, &model)
            && best.is_none_or(|(_, nearest)| distance < nearest)
        {
            best = Some((item.entity, distance));
        }
    }
    best
}

/// The sprite whose quad the ray hits first, with the distance (DR-38). The quad lies on the
/// sprite's own XY plane, so picking works the same under a perspective or an orthographic camera.
pub fn pick_sprite(sprites: &[SpriteItem], ray: &Ray) -> Option<(u64, f32)> {
    let mut best: Option<(u64, f32)> = None;
    for sprite in sprites
        .iter()
        .filter(|sprite| sprite.entity != 0 && sprite.width > 0.0 && sprite.height > 0.0)
    {
        let model = model_matrix(&sprite.transform);
        let half_width = sprite.width * 0.5;
        let half_height = sprite.height * 0.5;
        let corner =
            |x: f32, y: f32| transform_point(&model, [x * half_width, y * half_height, 0.0]);
        let top_left = corner(-1.0, 1.0);
        let top_right = corner(1.0, 1.0);
        let bottom_left = corner(-1.0, -1.0);
        let bottom_right = corner(1.0, -1.0);
        let distance = [
            hit_triangle(ray, top_left, top_right, bottom_left),
            hit_triangle(ray, top_right, bottom_right, bottom_left),
        ]
        .into_iter()
        .flatten()
        .min_by(f32::total_cmp);
        if let Some(distance) = distance
            && best.is_none_or(|(_, nearest)| distance < nearest)
        {
            best = Some((sprite.entity, distance));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::{Color, Transform};

    fn box_at(entity: u64, z: f32) -> DrawItem {
        DrawItem {
            entity,
            selected: false,
            hovered: false,
            shape: Shape::Box,
            size: Vec3::splat(2.0),
            detail: 0,
            transform: Transform {
                position: Vec3::new(0.0, 0.0, z),
                ..Transform::IDENTITY
            },
            color: Color::WHITE,
            mesh: None,
            mesh_fingerprint: 0,
        }
    }

    #[test]
    fn editable_meshes_pick_by_their_own_triangles() {
        // A single upward triangle far to the side of where a 2 m box would be.
        let polygon = petunia_math::PolygonMesh {
            positions: vec![
                Vec3::new(4.0, 0.0, 1.0),
                Vec3::new(6.0, 0.0, 1.0),
                Vec3::new(5.0, 0.0, -1.0),
            ],
            faces: vec![vec![0, 1, 2]],
        };
        let item = DrawItem {
            shape: Shape::Mesh,
            mesh: Some(Arc::new(crate::mesh::polygon_mesh_data(&polygon))),
            mesh_fingerprint: polygon.fingerprint(),
            ..box_at(9, 0.0)
        };
        let down = |x: f32| Ray {
            origin: Vec3::new(x, 5.0, 0.0),
            direction: Vec3::new(0.0, -1.0, 0.0),
        };
        let items = [item.clone()];
        assert_eq!(pick_entity(&items, &down(5.0)).map(|hit| hit.0), Some(9));
        assert_eq!(
            pick_entity(&items, &down(0.0)),
            None,
            "no box where the shape would be"
        );
        let hit = pick_surface(&items, &down(5.0), &[]).expect("surface");
        assert!(hit.normal.y > 0.99 && (hit.distance - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn a_sprite_is_picked_where_its_quad_is_and_not_outside_it() {
        let sprite = |entity: u64, z: f32, width: f32, height: f32| SpriteItem {
            entity,
            selected: false,
            hovered: false,
            width,
            height,
            transform: Transform {
                position: Vec3::new(0.0, 0.0, z),
                ..Transform::IDENTITY
            },
            color: Color::WHITE,
            asset: String::new(),
            texture: None,
            flip_x: false,
            flip_y: false,
            layer: 0,
            animation: crate::frame::SpriteAnimation::single(),
            frame: 0,
        };
        let forward = |x: f32, y: f32| Ray {
            origin: Vec3::new(x, y, 5.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
        };
        // Small quad in front, larger one behind it.
        let sprites = [sprite(4, 0.0, 2.0, 2.0), sprite(8, -3.0, 4.0, 4.0)];
        let center = pick_sprite(&sprites, &forward(0.0, 0.0)).expect("the nearer quad");
        assert_eq!(center.0, 4);
        assert!((center.1 - 5.0).abs() < 1.0e-4, "{}", center.1);
        assert_eq!(
            pick_sprite(&sprites, &forward(1.5, 0.0)).map(|hit| hit.0),
            Some(8),
            "outside the small quad but inside the big one"
        );
        assert_eq!(pick_sprite(&sprites, &forward(3.5, 0.0)), None);
        assert_eq!(
            pick_sprite(
                &[sprite(0, 0.0, 2.0, 2.0), sprite(5, 0.0, 0.0, 2.0)],
                &forward(0.0, 0.0)
            ),
            None,
            "entity 0 and a quad without size are not targets"
        );
    }

    #[test]
    fn picks_the_nearest_item_under_the_point_and_misses_empty_space() {
        let camera = Camera::default();
        let items = [box_at(7, -10.0), box_at(3, -5.0)];
        let center = ray_through(&camera, 1.0, 0.0, 0.0).expect("ray");
        assert_eq!(pick_entity(&items, &center).map(|hit| hit.0), Some(3));
        let corner = ray_through(&camera, 1.0, 0.95, 0.95).expect("ray");
        assert_eq!(pick_entity(&items, &corner), None);
        assert!(ray_through(&camera, 1.0, f32::NAN, 0.0).is_none());

        let ortho_camera = Camera {
            projection_mode: crate::camera::ProjectionMode::Orthographic,
            orthographic_height: 10.0,
            ..Camera::default()
        };
        let ortho_center = ray_through(&ortho_camera, 1.0, 0.0, 0.0).expect("ortho ray");
        assert_eq!(ortho_center.direction, ortho_camera.forward());
        assert_eq!(ortho_center.origin, ortho_camera.position);

        let ortho_offset = ray_through(&ortho_camera, 1.0, 0.5, 0.5).expect("offset ray");
        assert_eq!(ortho_offset.direction, ortho_camera.forward());
        assert!((ortho_offset.origin.x - 2.5).abs() < 1.0e-5);
        assert!((ortho_offset.origin.y - 2.5).abs() < 1.0e-5);
    }
}
