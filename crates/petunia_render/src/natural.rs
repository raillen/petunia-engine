//! Meshes for the natural world: terrain chunks, water ribbons and road ribbons.
//!
//! Pure functions; the editor and the renderer share them. Heights are row-major and the chunk
//! origin is the minimum world corner.

use petunia_math::{CubicBezierSpline, Vec3};
use petunia_sculpt::{
    PathMesh, ProfilePoint, river_profile, road_profile, split_spline_at, sweep_profile,
};

use crate::mesh::MeshData;

/// FNV-1a mixing used by the cache fingerprints (deterministic across machines).
pub fn hash_bits(mut hash: u64, value: u64) -> u64 {
    hash ^= value;
    hash.wrapping_mul(0x0000_0100_0000_01b3)
}

fn hash_f32(hash: u64, value: f32) -> u64 {
    hash_bits(hash, u64::from(value.to_bits()))
}

/// Fingerprint of a spline and its per-point parameters.
pub fn hash_spline(spline: &CubicBezierSpline) -> u64 {
    let mut hash = hash_bits(0xcbf2_9ce4_8422_2325, spline.points.len() as u64);
    hash = hash_bits(hash, u64::from(spline.closed));
    for point in &spline.points {
        for value in [
            point.position.x,
            point.position.y,
            point.position.z,
            point.handle_in.x,
            point.handle_in.y,
            point.handle_in.z,
            point.handle_out.x,
            point.handle_out.y,
            point.handle_out.z,
            point.width,
            point.depth,
            point.extra,
        ] {
            hash = hash_f32(hash, value);
        }
    }
    hash
}

/// Mesh of one heightfield chunk, in world coordinates, with a distance LOD.
///
/// `lod_step` skips cells (1 is full detail). The step is adjusted to divide the chunk edge, and
/// a skirt around the chunk hides the cracks where two chunks use different levels.
pub fn terrain_mesh_data(
    heights: &[f32],
    size: usize,
    cell_size: f32,
    origin: Vec3,
    fingerprint: u64,
    lod_step: usize,
) -> Option<(MeshData, u64)> {
    if size < 2 || heights.len() != size * size || !cell_size.is_finite() || cell_size <= 0.0 {
        return None;
    }
    let mut step = lod_step.clamp(1, size - 1);
    while step > 1 && !(size - 1).is_multiple_of(step) {
        step -= 1;
    }
    let grid = (size - 1) / step + 1;
    let mut hash = hash_bits(fingerprint, size as u64);
    hash = hash_bits(hash, step as u64);
    hash = hash_f32(hash, cell_size);
    for value in [origin.x, origin.y, origin.z] {
        hash = hash_f32(hash, value);
    }
    for height in heights {
        hash = hash_f32(hash, *height);
    }
    // Every sample below is clamped to `size - 1`, so the index is always inside the grid.
    let height_at = |x: usize, z: usize| heights.get(z * size + x).copied().unwrap_or(0.0);
    let mut data = MeshData::default();
    data.positions.reserve(grid * grid + grid * 4);
    data.uvs.reserve(grid * grid + grid * 4);
    data.flows.reserve(grid * grid + grid * 4);
    data.margins.reserve(grid * grid + grid * 4);
    for grid_z in 0..grid {
        let z = (grid_z * step).min(size - 1);
        for grid_x in 0..grid {
            let x = (grid_x * step).min(size - 1);
            data.positions.push([
                origin.x + x as f32 * cell_size,
                origin.y + height_at(x, z),
                origin.z + z as f32 * cell_size,
            ]);
            let left = height_at(x.saturating_sub(step), z);
            let right = height_at((x + step).min(size - 1), z);
            let back = height_at(x, z.saturating_sub(step));
            let front = height_at(x, (z + step).min(size - 1));
            let normal = Vec3::new(
                -(right - left) / (2.0 * cell_size * step as f32),
                1.0,
                -(front - back) / (2.0 * cell_size * step as f32),
            )
            .normalized_or_zero();
            data.normals.push([normal.x, normal.y, normal.z]);
            data.uvs
                .push([x as f32 / size as f32, z as f32 / size as f32]);
            data.flows.push([0.0, 0.0, 0.0]);
            data.margins.push(0.0);
        }
    }
    for grid_z in 0..(grid - 1) {
        for grid_x in 0..(grid - 1) {
            let a = (grid_z * grid + grid_x) as u32;
            let b = a + 1;
            let c = ((grid_z + 1) * grid + grid_x) as u32;
            let d = c + 1;
            data.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    // Skirt: duplicate the boundary one step below and close the four sides. Both windings are
    // emitted so the skirt is visible from inside and outside the chunk.
    let skirt_depth = cell_size * step as f32 * 0.5;
    let mut skirts = std::collections::BTreeMap::new();
    let boundary: std::collections::BTreeSet<(usize, usize)> = (0..grid)
        .flat_map(|index| [(index, 0), (index, grid - 1), (0, index), (grid - 1, index)])
        .collect();
    for (grid_x, grid_z) in boundary {
        let source = (grid_z * grid + grid_x) as u32;
        let position = *data.positions.get(source as usize)?;
        let normal = *data.normals.get(source as usize)?;
        let skirt = data.positions.len() as u32;
        data.positions
            .push([position[0], position[1] - skirt_depth, position[2]]);
        data.normals.push(normal);
        data.uvs.push([0.0, 0.0]);
        data.flows.push([0.0, 0.0, 0.0]);
        data.margins.push(0.0);
        skirts.insert((grid_x, grid_z), skirt);
    }
    for grid_z in 0..grid {
        for grid_x in 0..grid {
            if grid_x + 1 < grid && (grid_z == 0 || grid_z == grid - 1) {
                skirt_quad(
                    &mut data,
                    &skirts,
                    grid,
                    (grid_x, grid_z),
                    (grid_x + 1, grid_z),
                );
            }
            if grid_z + 1 < grid && (grid_x == 0 || grid_x == grid - 1) {
                skirt_quad(
                    &mut data,
                    &skirts,
                    grid,
                    (grid_x, grid_z),
                    (grid_x, grid_z + 1),
                );
            }
        }
    }
    Some((data, hash))
}

fn skirt_quad(
    data: &mut MeshData,
    skirts: &std::collections::BTreeMap<(usize, usize), u32>,
    grid: usize,
    first: (usize, usize),
    second: (usize, usize),
) {
    let first_top = first.1 * grid + first.0;
    let second_top = second.1 * grid + second.0;
    let (Some(&first_skirt), Some(&second_skirt)) = (skirts.get(&first), skirts.get(&second))
    else {
        return;
    };
    let first_top = first_top as u32;
    let second_top = second_top as u32;
    data.indices.extend_from_slice(&[
        first_top,
        first_skirt,
        second_top,
        second_top,
        first_skirt,
        second_skirt,
        first_top,
        second_top,
        first_skirt,
        second_top,
        second_skirt,
        first_skirt,
    ]);
}

fn path_mesh_to_data(mesh: &PathMesh) -> MeshData {
    MeshData {
        positions: mesh
            .vertices
            .iter()
            .map(|vertex| [vertex.position.x, vertex.position.y, vertex.position.z])
            .collect(),
        normals: mesh
            .vertices
            .iter()
            .map(|vertex| [vertex.normal.x, vertex.normal.y, vertex.normal.z])
            .collect(),
        indices: mesh.indices.clone(),
        uvs: mesh.vertices.iter().map(|vertex| vertex.uv).collect(),
        flows: mesh
            .vertices
            .iter()
            .map(|vertex| [vertex.flow.x, vertex.flow.y, vertex.flow.z])
            .collect(),
        margins: mesh
            .vertices
            .iter()
            .map(|vertex| vertex.margin_mask)
            .collect(),
    }
}

/// Flat water ribbon along a river spline. The profile is normalized; the spline carries width.
pub fn water_mesh_data(spline: &CubicBezierSpline, spacing: f32) -> Option<(MeshData, u64)> {
    let mesh = sweep_profile(spline, &river_profile(), spacing).ok()?;
    if mesh.is_empty() {
        return None;
    }
    let fingerprint = hash_spline(spline);
    Some((path_mesh_to_data(&mesh), fingerprint))
}

fn sub_spline(spline: &CubicBezierSpline, start: f32, end: f32) -> Option<CubicBezierSpline> {
    let segments = spline.segments() as f32;
    if segments <= 0.0 || !start.is_finite() || !end.is_finite() {
        return None;
    }
    let end = end.clamp(0.001, segments - 0.001);
    let start = start.clamp(0.001, end - 0.001);
    let (left, _) = split_spline_at(spline, end)?;
    let (_, middle) = split_spline_at(&left, start)?;
    Some(middle)
}

fn append_mesh(target: &mut MeshData, source: &MeshData) {
    let offset = target.positions.len() as u32;
    target.positions.extend_from_slice(&source.positions);
    target.normals.extend_from_slice(&source.normals);
    target.uvs.extend_from_slice(&source.uvs);
    target.flows.extend_from_slice(&source.flows);
    target.margins.extend_from_slice(&source.margins);
    target
        .indices
        .extend(source.indices.iter().map(|index| index + offset));
}

/// Two vertical rails at the road edges. Used by bridges and by roads with `guardrail`.
pub fn rail_pair_mesh_data(
    spline: &CubicBezierSpline,
    width: f32,
    spacing: f32,
) -> Option<(MeshData, u64)> {
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    let mut shaped = spline.clone();
    for point in &mut shaped.points {
        point.width = width;
        point.depth = 1.0;
    }
    let left = sweep_profile(
        &shaped,
        &[ProfilePoint::new(-1.0, 0.0), ProfilePoint::new(-1.0, 1.0)],
        spacing,
    )
    .ok()?;
    let right = sweep_profile(
        &shaped,
        &[ProfilePoint::new(1.0, 0.0), ProfilePoint::new(1.0, 1.0)],
        spacing,
    )
    .ok()?;
    if left.is_empty() || right.is_empty() {
        return None;
    }
    let mut mesh = path_mesh_to_data(&left);
    append_mesh(&mut mesh, &path_mesh_to_data(&right));
    let mut fingerprint = hash_spline(&shaped);
    fingerprint = hash_f32(fingerprint, width);
    Some((mesh, fingerprint))
}

/// Deck thickness of a bridge, in meters.
const BRIDGE_DECK_THICKNESS: f32 = 0.6;

/// Rebuilds a mesh with one flat normal per triangle.
///
/// The sweep assigns every vertex the frame's up vector, which is right for flat ribbons and wrong
/// for a closed slab: the sides and the underside would be lit as if they faced the sky.
fn with_flat_triangle_normals(source: MeshData) -> MeshData {
    let mut data = MeshData::default();
    for triangle in source.indices.as_chunks::<3>().0 {
        let [index_a, index_b, index_c] = *triangle;
        // A triangle is emitted whole or not at all: a half triangle would break the winding.
        let vertices = [index_a, index_b, index_c].map(|index| {
            let index = index as usize;
            Some((
                *source.positions.get(index)?,
                source.uvs.get(index).copied()?,
                source.flows.get(index).copied()?,
                source.margins.get(index).copied().unwrap_or(0.0),
            ))
        });
        let [Some(first), Some(second), Some(third)] = vertices else {
            continue;
        };
        let edge_a = Vec3::new(
            second.0[0] - first.0[0],
            second.0[1] - first.0[1],
            second.0[2] - first.0[2],
        );
        let edge_b = Vec3::new(
            third.0[0] - first.0[0],
            third.0[1] - first.0[1],
            third.0[2] - first.0[2],
        );
        let normal = edge_a.cross(edge_b).normalized_or_zero();
        for (position, uv, flow, margin) in [first, second, third] {
            data.positions.push(position);
            data.normals.push([normal.x, normal.y, normal.z]);
            data.uvs.push(uv);
            data.flows.push(flow);
            data.margins.push(margin);
            data.indices.push((data.positions.len() - 1) as u32);
        }
    }
    data
}

/// Closed deck slab for a detected bridge span: top at the road surface, sides and underside below.
///
/// The road ribbon already spans the gap; the deck gives it thickness so the bridge reads as a
/// structure instead of a floating strip.
fn bridge_deck_mesh_data(span: &CubicBezierSpline, width: f32, spacing: f32) -> Option<MeshData> {
    let mut shaped = span.clone();
    for point in &mut shaped.points {
        point.width = width;
        point.depth = BRIDGE_DECK_THICKNESS;
    }
    let outline = [
        ProfilePoint::new(-1.0, 0.0),
        ProfilePoint::new(-1.0, -1.0),
        ProfilePoint::new(1.0, -1.0),
        ProfilePoint::new(1.0, 0.0),
        ProfilePoint::new(-1.0, 0.0),
    ];
    let mesh = sweep_profile(&shaped, &outline, spacing).ok()?;
    if mesh.is_empty() {
        return None;
    }
    Some(with_flat_triangle_normals(path_mesh_to_data(&mesh)))
}

/// Deck slab and rails for a detected bridge span (parameters in segment units).
pub fn bridge_mesh_data(
    spline: &CubicBezierSpline,
    start_parameter: f32,
    end_parameter: f32,
    width: f32,
    spacing: f32,
) -> Option<(MeshData, u64)> {
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    if !start_parameter.is_finite() || !end_parameter.is_finite() {
        return None;
    }
    // A span shorter than this is a point, not a bridge.
    if end_parameter - start_parameter <= 1.0e-3 {
        return None;
    }
    let span = sub_spline(spline, start_parameter, end_parameter)?;
    let mut mesh = bridge_deck_mesh_data(&span, width, spacing)?;
    let (rails, _) = rail_pair_mesh_data(&span, width, spacing)?;
    append_mesh(&mut mesh, &rails);
    let mut fingerprint = hash_spline(&span);
    fingerprint = hash_f32(fingerprint, width);
    Some((mesh, fingerprint))
}

/// Road ribbon with the road's own width and shoulder, independent of the spline point widths.
pub fn road_mesh_data(
    spline: &CubicBezierSpline,
    width: f32,
    shoulder: f32,
    spacing: f32,
) -> Option<(MeshData, u64)> {
    if !width.is_finite() || width <= 0.0 {
        return None;
    }
    let mut shaped = spline.clone();
    for point in &mut shaped.points {
        point.width = width;
    }
    let mesh = sweep_profile(&shaped, &road_profile(shoulder), spacing).ok()?;
    if mesh.is_empty() {
        return None;
    }
    let mut fingerprint = hash_spline(&shaped);
    fingerprint = hash_f32(fingerprint, shoulder);
    Some((path_mesh_to_data(&mesh), fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::SplinePoint;

    fn straight_spline(height: f32) -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::new(0.0, height, 0.0),
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(10.0, 0.0, 0.0),
            width: 6.0,
            depth: 2.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(30.0, height, 0.0),
            handle_in: Vec3::new(-10.0, 0.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 6.0,
            depth: 2.0,
            extra: 0.0,
        });
        spline
    }

    #[test]
    fn bridge_mesh_has_a_deck_below_the_road_and_rails_above() {
        let spline = straight_spline(5.0);
        let (mesh, _) = bridge_mesh_data(&spline, 0.0, 1.0, 6.0, 1.0).unwrap_or_else(|| panic!());
        assert!(!mesh.positions.is_empty());
        assert!(mesh.indices.len().is_multiple_of(3));
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.positions.len())
        );
        // The deck hangs below the road surface and the rails stand above it.
        let lowest = mesh
            .positions
            .iter()
            .map(|position| position[1])
            .fold(f32::INFINITY, f32::min);
        let highest = mesh
            .positions
            .iter()
            .map(|position| position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(lowest < 5.0 - 0.3, "lowest {lowest}");
        assert!(highest > 5.5, "highest {highest}");
    }

    #[test]
    fn bridge_sides_and_underside_are_not_lit_from_above() {
        let spline = straight_spline(5.0);
        let (mesh, _) = bridge_mesh_data(&spline, 0.0, 1.0, 6.0, 1.0).unwrap_or_else(|| panic!());
        let upwards = mesh.normals.iter().filter(|normal| normal[1] > 0.9).count();
        let sideways = mesh
            .normals
            .iter()
            .filter(|normal| normal[1].abs() < 0.1)
            .count();
        let downwards = mesh
            .normals
            .iter()
            .filter(|normal| normal[1] < -0.9)
            .count();
        assert!(upwards > 0);
        assert!(sideways > 0, "deck has no side faces");
        assert!(downwards > 0, "deck has no underside");
    }

    #[test]
    fn hostile_bridge_parameters_are_refused() {
        let spline = straight_spline(5.0);
        assert!(bridge_mesh_data(&spline, 0.0, 1.0, 0.0, 1.0).is_none());
        assert!(bridge_mesh_data(&spline, 0.0, 1.0, f32::NAN, 1.0).is_none());
        assert!(bridge_mesh_data(&spline, 0.5, 0.5, 6.0, 1.0).is_none());
    }

    #[test]
    fn terrain_mesh_is_upward_facing() {
        let heights = vec![0.0f32; 4 * 4];
        let (mesh, _) =
            terrain_mesh_data(&heights, 4, 1.0, Vec3::ZERO, 1, 1).unwrap_or_else(|| panic!("mesh"));
        assert!(mesh.positions.len() >= 16);
        assert!(mesh.indices.len() > 9 * 6);
        assert!(mesh.normals.iter().all(|normal| normal[1] > 0.9));
    }

    #[test]
    fn hostile_terrain_is_refused() {
        assert!(terrain_mesh_data(&[0.0, 1.0], 4, 1.0, Vec3::ZERO, 1, 1).is_none());
        assert!(terrain_mesh_data(&[0.0; 16], 4, 0.0, Vec3::ZERO, 1, 1).is_none());
        assert!(terrain_mesh_data(&[0.0; 16], 4, 1.0, Vec3::ZERO, 1, 1).is_some());
    }

    #[test]
    fn terrain_lod_uses_fewer_vertices_and_closes_the_cracks() {
        let heights = vec![0.0f32; 64 * 64];
        let (full, _) =
            terrain_mesh_data(&heights, 64, 1.0, Vec3::ZERO, 1, 1).unwrap_or_else(|| panic!());
        let (coarse, _) =
            terrain_mesh_data(&heights, 64, 1.0, Vec3::ZERO, 1, 3).unwrap_or_else(|| panic!());
        assert!(coarse.positions.len() < full.positions.len());
        assert!(coarse.indices.len().is_multiple_of(3));
        assert!(coarse.positions.len() < full.positions.len() / 2);
        assert!(coarse.positions.len() > 100);
    }
}
