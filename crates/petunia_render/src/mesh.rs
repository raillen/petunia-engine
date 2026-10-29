use std::collections::HashMap;
use std::sync::Arc;

use petunia_math::{PolygonMesh, Vec3};

use crate::frame::{DrawItem, Shape};

/// Segments around a round shape when the brush does not say.
pub const DEFAULT_SEGMENTS: u32 = 16;
const MIN_SEGMENTS: u32 = 3;
const MAX_SEGMENTS: u32 = 64;
const MAX_STEPS: u32 = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// The size must be finite and greater than zero on every axis.
    InvalidSize,
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("o tamanho do brush precisa ser finito e maior que zero nos três eixos")
    }
}

impl std::error::Error for MeshError {}

/// Triangle mesh with one normal per vertex. Counter-clockwise triangles face outward.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Optional per-vertex colors (terrain layers). Empty for built-in shapes and editable
    /// meshes, which use the item's uniform color.
    pub colors: Vec<[f32; 3]>,
}

impl MeshData {
    /// Interleaved `position(3) + normal(3)` floats, the layout the GL adapter uploads.
    pub fn interleaved(&self) -> Vec<f32> {
        let mut data = Vec::with_capacity(self.positions.len() * 6);
        for (position, normal) in self.positions.iter().zip(&self.normals) {
            data.extend_from_slice(position);
            data.extend_from_slice(normal);
        }
        data
    }

    fn push_vertex(&mut self, position: Vec3, normal: Vec3) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push([position.x, position.y, position.z]);
        self.normals.push([normal.x, normal.y, normal.z]);
        index
    }

    /// Adds a convex flat polygon whose normal faces away from `interior`.
    fn push_polygon(&mut self, corners: &[Vec3], interior: Vec3) {
        let [a, b, c, ..] = corners else { return };
        let mut normal = (*b - *a).cross(*c - *a).normalized_or_zero();
        let centroid = corners.iter().fold(Vec3::ZERO, |sum, corner| sum + *corner)
            * (1.0 / corners.len() as f32);
        let flipped = normal.dot(centroid - interior) < 0.0;
        if flipped {
            normal = -normal;
        }
        let base = self.positions.len() as u32;
        for corner in corners {
            self.push_vertex(*corner, normal);
        }
        for step in 1..(corners.len() as u32 - 1) {
            if flipped {
                self.indices.extend([base, base + step + 1, base + step]);
            } else {
                self.indices.extend([base, base + step, base + step + 1]);
            }
        }
    }
}

/// The drawing mesh of an editable polygon mesh: flat shaded (one normal per face, so every
/// face reads as a facet), concave faces cut into triangles. Degenerate faces are skipped.
pub fn polygon_mesh_data(mesh: &PolygonMesh) -> MeshData {
    let mut data = MeshData::default();
    let mut triangles = Vec::new();
    for face in &mesh.faces {
        let normal = mesh.face_normal(face);
        if normal.length_squared() == 0.0 {
            continue;
        }
        triangles.clear();
        mesh.triangulate_face(face, &mut triangles);
        for triangle in &triangles {
            for vertex in triangle {
                let position = mesh
                    .positions
                    .get(*vertex as usize)
                    .copied()
                    .unwrap_or(Vec3::ZERO);
                let index = data.push_vertex(position, normal);
                data.indices.push(index);
            }
        }
    }
    data
}

/// Drawing meshes of editable meshes, by fingerprint: the editor extracts the scene many times
/// per mouse move, and a mesh is cut into triangles only when it changes. Meshes not asked for
/// since the previous [`MeshCache::sweep`] are forgotten.
#[derive(Default)]
pub struct MeshCache {
    meshes: HashMap<u64, Arc<MeshData>>,
    used: std::collections::HashSet<u64>,
}

impl MeshCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The drawing mesh for `mesh` (whose fingerprint is `fingerprint`).
    pub fn drawing(&mut self, fingerprint: u64, mesh: &PolygonMesh) -> Arc<MeshData> {
        self.used.insert(fingerprint);
        self.meshes
            .entry(fingerprint)
            .or_insert_with(|| Arc::new(polygon_mesh_data(mesh)))
            .clone()
    }

    /// A mesh built by the caller, cached by fingerprint (terrain chunks, for example).
    pub fn cached<F>(&mut self, fingerprint: u64, build: F) -> Arc<MeshData>
    where
        F: FnOnce() -> MeshData,
    {
        self.used.insert(fingerprint);
        self.meshes
            .entry(fingerprint)
            .or_insert_with(|| Arc::new(build()))
            .clone()
    }

    /// Drops the meshes not used since the last sweep.
    pub fn sweep(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.meshes
            .retain(|fingerprint, _| used.contains(fingerprint));
    }

    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }
}

/// The triangles an item draws: its own mesh for an editable mesh, else the shape's.
pub fn item_mesh(item: &DrawItem) -> Result<Arc<MeshData>, MeshError> {
    match (&item.shape, &item.mesh) {
        (Shape::Mesh | Shape::Terrain, Some(mesh)) => Ok(mesh.clone()),
        (Shape::Mesh | Shape::Terrain, None) => Ok(Arc::new(MeshData::default())),
        _ => build_mesh(item.shape, item.size, item.detail).map(Arc::new),
    }
}

/// Builds the mesh for a shape. `size` is width, height and depth in meters (for round shapes,
/// x and z are the diameters). `detail` is the number of segments around a round shape
/// (cylinder, cone, sphere) or the number of steps of a staircase; it is clamped to a sane range.
/// The mesh is centered on the origin.
pub fn build_mesh(shape: Shape, size: Vec3, detail: u32) -> Result<MeshData, MeshError> {
    if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
        return Err(MeshError::InvalidSize);
    }
    let half = size * 0.5;
    let segments = if detail == 0 {
        DEFAULT_SEGMENTS
    } else {
        detail.clamp(MIN_SEGMENTS, MAX_SEGMENTS)
    };
    Ok(match shape {
        Shape::Box => box_mesh(half),
        Shape::Ramp => ramp_mesh(half),
        Shape::Cylinder => cylinder_mesh(half, segments),
        Shape::Cone => cone_mesh(half, segments),
        Shape::Sphere => sphere_mesh(half, segments),
        Shape::Stairs => stairs_mesh(half, detail.clamp(1, MAX_STEPS)),
        // An editable mesh or a terrain chunk carries its own triangles ([`item_mesh`]).
        Shape::Mesh | Shape::Terrain => MeshData::default(),
    })
}

fn box_mesh(half: Vec3) -> MeshData {
    let corner = |sx: f32, sy: f32, sz: f32| Vec3::new(sx * half.x, sy * half.y, sz * half.z);
    let faces: [[Vec3; 4]; 6] = [
        [
            corner(1.0, -1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(1.0, 1.0, 1.0),
            corner(1.0, -1.0, 1.0),
        ],
        [
            corner(-1.0, -1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, 1.0, 1.0),
        ],
        [
            corner(-1.0, -1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(1.0, -1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
        ],
        [
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, 1.0, 1.0),
        ],
        [
            corner(-1.0, -1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
        ],
    ];
    let mut mesh = MeshData::default();
    for face in &faces {
        mesh.push_polygon(face, Vec3::ZERO);
    }
    mesh
}

/// Wedge: low edge at the front (+z), high edge at the back (-z).
fn ramp_mesh(half: Vec3) -> MeshData {
    let front_bottom_left = Vec3::new(-half.x, -half.y, half.z);
    let front_bottom_right = Vec3::new(half.x, -half.y, half.z);
    let back_bottom_left = Vec3::new(-half.x, -half.y, -half.z);
    let back_bottom_right = Vec3::new(half.x, -half.y, -half.z);
    let back_top_left = Vec3::new(-half.x, half.y, -half.z);
    let back_top_right = Vec3::new(half.x, half.y, -half.z);
    // Centroid of the triangular prism; the origin lies on the slope, so it cannot be used.
    let interior = Vec3::new(0.0, -half.y / 3.0, -half.z / 3.0);

    let mut mesh = MeshData::default();
    mesh.push_polygon(
        &[
            front_bottom_left,
            front_bottom_right,
            back_bottom_right,
            back_bottom_left,
        ],
        interior,
    );
    mesh.push_polygon(
        &[
            back_bottom_left,
            back_bottom_right,
            back_top_right,
            back_top_left,
        ],
        interior,
    );
    mesh.push_polygon(
        &[
            front_bottom_left,
            front_bottom_right,
            back_top_right,
            back_top_left,
        ],
        interior,
    );
    mesh.push_polygon(
        &[front_bottom_left, back_bottom_left, back_top_left],
        interior,
    );
    mesh.push_polygon(
        &[front_bottom_right, back_bottom_right, back_top_right],
        interior,
    );
    mesh
}

fn cylinder_mesh(half: Vec3, segments: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let ring = |angle: f32, y: f32| Vec3::new(half.x * angle.cos(), y, half.z * angle.sin());
    let side_normal = |angle: f32| {
        Vec3::new(angle.cos() / half.x, 0.0, angle.sin() / half.z).normalized_or_zero()
    };
    let step = std::f32::consts::TAU / segments as f32;

    for segment in 0..segments {
        let (angle_a, angle_b) = (segment as f32 * step, (segment + 1) as f32 * step);
        let bottom_a = mesh.push_vertex(ring(angle_a, -half.y), side_normal(angle_a));
        let bottom_b = mesh.push_vertex(ring(angle_b, -half.y), side_normal(angle_b));
        let top_b = mesh.push_vertex(ring(angle_b, half.y), side_normal(angle_b));
        let top_a = mesh.push_vertex(ring(angle_a, half.y), side_normal(angle_a));
        mesh.indices
            .extend([bottom_a, top_b, bottom_b, bottom_a, top_a, top_b]);
    }

    let top_center = mesh.push_vertex(Vec3::new(0.0, half.y, 0.0), Vec3::UP);
    let bottom_center = mesh.push_vertex(Vec3::new(0.0, -half.y, 0.0), -Vec3::UP);
    for segment in 0..segments {
        let (angle_a, angle_b) = (segment as f32 * step, (segment + 1) as f32 * step);
        let top_a = mesh.push_vertex(ring(angle_a, half.y), Vec3::UP);
        let top_b = mesh.push_vertex(ring(angle_b, half.y), Vec3::UP);
        mesh.indices.extend([top_center, top_b, top_a]);
        let bottom_a = mesh.push_vertex(ring(angle_a, -half.y), -Vec3::UP);
        let bottom_b = mesh.push_vertex(ring(angle_b, -half.y), -Vec3::UP);
        mesh.indices.extend([bottom_center, bottom_a, bottom_b]);
    }
    mesh
}

fn cone_mesh(half: Vec3, segments: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let apex = Vec3::new(0.0, half.y, 0.0);
    let ring = |angle: f32| Vec3::new(half.x * angle.cos(), -half.y, half.z * angle.sin());
    // Gradient of the cone's implicit surface: smooth shading along the slope.
    let slope_normal = |angle: f32| {
        Vec3::new(
            2.0 * angle.cos() / half.x,
            1.0 / half.y,
            2.0 * angle.sin() / half.z,
        )
        .normalized_or_zero()
    };
    let step = std::f32::consts::TAU / segments as f32;
    let bottom_center = mesh.push_vertex(Vec3::new(0.0, -half.y, 0.0), -Vec3::UP);
    for segment in 0..segments {
        let (angle_a, angle_b) = (segment as f32 * step, (segment + 1) as f32 * step);
        let middle = (angle_a + angle_b) * 0.5;
        let base_a = mesh.push_vertex(ring(angle_a), slope_normal(angle_a));
        let tip = mesh.push_vertex(apex, slope_normal(middle));
        let base_b = mesh.push_vertex(ring(angle_b), slope_normal(angle_b));
        mesh.indices.extend([base_a, tip, base_b]);
        let cap_a = mesh.push_vertex(ring(angle_a), -Vec3::UP);
        let cap_b = mesh.push_vertex(ring(angle_b), -Vec3::UP);
        mesh.indices.extend([bottom_center, cap_a, cap_b]);
    }
    mesh
}

fn sphere_mesh(half: Vec3, segments: u32) -> MeshData {
    let rings = (segments / 2).max(2);
    let mut mesh = MeshData::default();
    // Ellipsoid normal: gradient of x^2/a^2 + y^2/b^2 + z^2/c^2.
    let point = |ring: u32, segment: u32| {
        let latitude = std::f32::consts::PI * ring as f32 / rings as f32;
        let longitude = std::f32::consts::TAU * segment as f32 / segments as f32;
        let (sin_lat, cos_lat) = latitude.sin_cos();
        let position = Vec3::new(
            half.x * sin_lat * longitude.cos(),
            half.y * cos_lat,
            half.z * sin_lat * longitude.sin(),
        );
        let normal = Vec3::new(
            position.x / (half.x * half.x),
            position.y / (half.y * half.y),
            position.z / (half.z * half.z),
        )
        .normalized_or_zero();
        (position, normal)
    };
    let mut grid = Vec::new();
    for ring in 0..=rings {
        for segment in 0..=segments {
            let (position, normal) = point(ring, segment);
            grid.push(mesh.push_vertex(position, normal));
        }
    }
    let at = |ring: u32, segment: u32| {
        grid.get((ring * (segments + 1) + segment) as usize)
            .copied()
            .unwrap_or(0)
    };
    for ring in 0..rings {
        for segment in 0..segments {
            let top_a = at(ring, segment);
            let top_b = at(ring, segment + 1);
            let bottom_a = at(ring + 1, segment);
            let bottom_b = at(ring + 1, segment + 1);
            // The poles collapse a row to a point: skip the triangle that would have no area.
            if ring + 1 != rings {
                mesh.indices.extend([bottom_a, top_b, bottom_b]);
            }
            if ring != 0 {
                mesh.indices.extend([bottom_a, top_a, top_b]);
            }
        }
    }
    mesh
}

/// Adds the six faces of an axis-aligned box.
fn push_box(mesh: &mut MeshData, minimum: Vec3, maximum: Vec3) {
    let center = (minimum + maximum) * 0.5;
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { maximum.x } else { minimum.x },
            if y { maximum.y } else { minimum.y },
            if z { maximum.z } else { minimum.z },
        )
    };
    let faces: [[Vec3; 4]; 6] = [
        [
            corner(true, false, false),
            corner(true, true, false),
            corner(true, true, true),
            corner(true, false, true),
        ],
        [
            corner(false, false, false),
            corner(false, true, false),
            corner(false, true, true),
            corner(false, false, true),
        ],
        [
            corner(false, true, false),
            corner(true, true, false),
            corner(true, true, true),
            corner(false, true, true),
        ],
        [
            corner(false, false, false),
            corner(true, false, false),
            corner(true, false, true),
            corner(false, false, true),
        ],
        [
            corner(false, false, true),
            corner(true, false, true),
            corner(true, true, true),
            corner(false, true, true),
        ],
        [
            corner(false, false, false),
            corner(true, false, false),
            corner(true, true, false),
            corner(false, true, false),
        ],
    ];
    for face in &faces {
        mesh.push_polygon(face, center);
    }
}

/// Staircase rising toward -z (low at the front, +z), like the ramp. Each step is its own box
/// from the ground up, so every face is convex and faces outward.
fn stairs_mesh(half: Vec3, steps: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let tread = half.z * 2.0 / steps as f32;
    let rise = half.y * 2.0 / steps as f32;
    for step in 0..steps {
        let front = half.z - step as f32 * tread;
        let back = front - tread;
        let top = -half.y + (step + 1) as f32 * rise;
        push_box(
            &mut mesh,
            Vec3::new(-half.x, -half.y, back),
            Vec3::new(half.x, top, front),
        );
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(mesh: &MeshData, index: u32) -> Vec3 {
        let [x, y, z] = mesh.positions[index as usize];
        Vec3::new(x, y, z)
    }

    /// We cannot look at the meshes in a window here, so check the winding numerically:
    /// every triangle must face away from the solid's interior, and the stored vertex normal
    /// must agree with the triangle's own geometric normal.
    fn assert_faces_outward(mesh: &MeshData, interior: Vec3) {
        assert_faces_outward_with(mesh, interior, 0.5);
    }

    /// `agreement` is the least allowed dot product between a smooth vertex normal and the flat
    /// face normal (very coarse round shapes legitimately differ a lot).
    fn assert_faces_outward_with(mesh: &MeshData, interior: Vec3, agreement: f32) {
        assert!(!mesh.indices.is_empty());
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = [
                at(mesh, triangle[0]),
                at(mesh, triangle[1]),
                at(mesh, triangle[2]),
            ];
            let geometric = (b - a).cross(c - a).normalized_or_zero();
            let centroid = (a + b + c) * (1.0 / 3.0);
            assert!(
                geometric.dot(centroid - interior) > 0.0,
                "triangle faces inward"
            );
            let [nx, ny, nz] = mesh.normals[triangle[0] as usize];
            let dot = geometric.dot(Vec3::new(nx, ny, nz));
            assert!(
                dot > agreement,
                "stored normal disagrees: dot {dot}, needs {agreement}, triangle {triangle:?}, vertices {a:?} {b:?} {c:?}"
            );
        }
    }

    #[test]
    fn box_ramp_and_cylinder_face_outward() {
        let size = Vec3::new(2.0, 3.0, 4.0);
        assert_faces_outward(&build_mesh(Shape::Box, size, 0).unwrap(), Vec3::ZERO);
        assert_faces_outward(&build_mesh(Shape::Cylinder, size, 0).unwrap(), Vec3::ZERO);
        let interior = Vec3::new(0.0, -size.y / 6.0, -size.z / 6.0);
        assert_faces_outward(&build_mesh(Shape::Ramp, size, 0).unwrap(), interior);
    }

    #[test]
    fn cone_sphere_and_stairs_face_outward_at_any_detail() {
        let size = Vec3::new(2.0, 3.0, 4.0);
        for detail in [0, 3, 8, 64, 1000] {
            // With 3 segments a "sphere" is a few big facets: smooth normals legitimately differ.
            let agreement = if detail == 3 { -1.0 } else { 0.5 };
            let sphere = build_mesh(Shape::Sphere, size, detail).unwrap();
            assert_faces_outward_with(&sphere, Vec3::ZERO, agreement);
            // The cone's interior point sits low: the origin is on the slope's inner side.
            let interior = Vec3::new(0.0, -size.y / 4.0, 0.0);
            let cone = build_mesh(Shape::Cone, size, detail).unwrap();
            assert_faces_outward_with(&cone, interior, agreement);
        }
        // One step is a plain box. More steps are checked per step: every triangle of step k
        // must face away from that step's own center.
        assert_faces_outward(&build_mesh(Shape::Stairs, size, 1).unwrap(), Vec3::ZERO);
        let stairs = build_mesh(Shape::Stairs, size, 4).unwrap();
        assert_eq!(
            stairs.indices.len(),
            4 * 12 * 3,
            "four boxes of twelve triangles"
        );
        for triangle in stairs.indices.as_chunks::<3>().0 {
            let [a, b, c] = [
                at(&stairs, triangle[0]),
                at(&stairs, triangle[1]),
                at(&stairs, triangle[2]),
            ];
            let geometric = (b - a).cross(c - a).normalized_or_zero();
            let [nx, ny, nz] = stairs.normals[triangle[0] as usize];
            assert!(
                geometric.dot(Vec3::new(nx, ny, nz)) > 0.5,
                "stored normal disagrees"
            );
        }
    }

    #[test]
    fn invalid_sizes_are_rejected() {
        assert_eq!(
            build_mesh(Shape::Box, Vec3::new(0.0, 1.0, 1.0), 0),
            Err(MeshError::InvalidSize)
        );
        assert_eq!(
            build_mesh(Shape::Box, Vec3::new(f32::NAN, 1.0, 1.0), 0),
            Err(MeshError::InvalidSize)
        );
    }
}
