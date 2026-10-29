//! Terrain chunk meshes for the renderer: one grid per chunk, optional distance LOD and skirts
//! to hide the cracks between different levels, and one color per material layer.
//!
//! The mesh is in world coordinates (identity transform); heights come from `petunia_terrain`,
//! so the renderer never owns terrain data.

use petunia_math::{Color, Vec3};
use petunia_terrain::{ChunkCoord, Terrain};

use crate::mesh::MeshData;

/// Default color of each material layer, indexed by the `materials` value in the terrain.
pub const TERRAIN_LAYERS: [Color; 8] = [
    Color::rgb(0.35, 0.55, 0.28), // grass
    Color::rgb(0.45, 0.35, 0.25), // dirt
    Color::rgb(0.48, 0.48, 0.50), // rock
    Color::rgb(0.78, 0.70, 0.50), // sand
    Color::rgb(0.92, 0.94, 0.97), // snow
    Color::rgb(0.32, 0.26, 0.20), // mud
    Color::rgb(0.25, 0.25, 0.27), // asphalt
    Color::rgb(0.22, 0.36, 0.50), // water bed
];

/// Distance thresholds (meters) for the chunk LOD.
pub const LOD_NEAR_DISTANCE: f32 = 96.0;
pub const LOD_MID_DISTANCE: f32 = 256.0;

pub fn layer_color(material: u8) -> Color {
    match TERRAIN_LAYERS.get(material as usize) {
        Some(color) => *color,
        None => TERRAIN_LAYERS[0],
    }
}

/// The world box a chunk's mesh occupies, including the deepest skirt any LOD draws. `None` when
/// the chunk does not exist.
pub fn chunk_bounds(terrain: &Terrain, coord: ChunkCoord) -> Option<(Vec3, Vec3)> {
    let chunk = terrain.chunk(coord)?;
    let cells = terrain.chunk_cells().max(1);
    let cell_size = terrain.cell_size();
    let (minimum_xz, maximum_xz) = chunk.bounds(cell_size);
    let mut lowest = f32::INFINITY;
    let mut highest = f32::NEG_INFINITY;
    // The mesh also samples the shared border, so a tall neighbour must widen the bound (otherwise
    // a visible chunk could be culled at its edge).
    let base_x = coord.x as i64 * cells as i64;
    let base_z = coord.z as i64 * cells as i64;
    for row in 0..=cells {
        for column in 0..=cells {
            let (global_x, global_z) = sample_lattice(terrain, base_x, base_z, cells, column, row);
            let Some(height) = terrain.sample(global_x, global_z) else {
                continue;
            };
            if height.is_finite() {
                lowest = lowest.min(height);
                highest = highest.max(height);
            }
        }
    }
    if lowest > highest {
        return None;
    }
    // A coarse chunk hangs a skirt of up to two coarse cells; take the widest LOD as the bound.
    let skirt = (cell_size * 4.0 * 2.0).max(0.5);
    Some((
        Vec3::new(minimum_xz.x, lowest - skirt, minimum_xz.z),
        Vec3::new(maximum_xz.x, highest, maximum_xz.z),
    ))
}

/// The lattice coordinate a mesh sample reads from: its own, or the closest one inside the chunk
/// when the neighbour does not exist (the mesh clamps to keep the surface closed).
fn sample_lattice(
    terrain: &Terrain,
    base_x: i64,
    base_z: i64,
    cells: usize,
    column: usize,
    row: usize,
) -> (i64, i64) {
    let global_x = base_x.saturating_add(column as i64);
    let global_z = base_z.saturating_add(row as i64);
    if terrain.sample(global_x, global_z).is_some() {
        return (global_x, global_z);
    }
    let clamped_x = column.min(cells - 1);
    let clamped_z = row.min(cells - 1);
    (
        base_x.saturating_add(clamped_x as i64),
        base_z.saturating_add(clamped_z as i64),
    )
}

/// The sphere around a chunk's mesh, for culling. It is conservative: it always contains the mesh.
pub fn chunk_sphere(terrain: &Terrain, coord: ChunkCoord) -> Option<(Vec3, f32)> {
    let (minimum, maximum) = chunk_bounds(terrain, coord)?;
    let center = (minimum + maximum) * 0.5;
    let radius = ((maximum - minimum) * 0.5).length();
    radius.is_finite().then_some((center, radius))
}

/// Sample step for a chunk at `distance` from the camera: 1 (full), 2 or 4.
pub fn lod_step(distance: f32) -> usize {
    if distance <= LOD_NEAR_DISTANCE {
        1
    } else if distance <= LOD_MID_DISTANCE {
        2
    } else {
        4
    }
}

/// Builds the triangle mesh of one chunk. `step` is the LOD (1 = every sample). The mesh includes
/// downward skirts on the four borders so a coarse chunk next to a fine one has no visible gap.
pub fn chunk_mesh(terrain: &Terrain, coord: ChunkCoord, step: usize) -> MeshData {
    let cells = terrain.chunk_cells().max(1);
    let step = step.clamp(1, cells);
    let cell_size = terrain.cell_size();
    let mut mesh = MeshData::default();

    let mut along = Vec::new();
    let mut local = 0;
    while local < cells {
        along.push(local);
        local += step;
    }
    along.push(cells);
    let count = along.len();

    for row in 0..count {
        for column in 0..count {
            let (Some(&local_x), Some(&local_z)) = (along.get(column), along.get(row)) else {
                continue;
            };
            let (world_x, world_z, height, material) =
                sample_world(terrain, coord, cells, cell_size, local_x, local_z);
            let normal = terrain.normal_at(world_x, world_z).unwrap_or(Vec3::UP);
            let color = layer_color(material);
            mesh.positions.push([world_x, height, world_z]);
            mesh.normals.push([normal.x, normal.y, normal.z]);
            mesh.colors.push([color.r, color.g, color.b]);
        }
    }

    for row in 0..count.saturating_sub(1) {
        for column in 0..count.saturating_sub(1) {
            let a = (row * count + column) as u32;
            let b = a + 1;
            let c = a + count as u32;
            let d = c + 1;
            // A hole in the terrain (usually a cave entrance) skips the cell.
            let (Some(first), Some(last)) = (
                mesh.positions.get(a as usize),
                mesh.positions.get(d as usize),
            ) else {
                continue;
            };
            let center_x = (first[0] + last[0]) * 0.5;
            let center_z = (first[2] + last[2]) * 0.5;
            if terrain.is_hole(center_x, center_z) {
                continue;
            }
            // Upward winding (y up, +x right, +z forward).
            mesh.indices.extend([a, c, b, b, c, d]);
        }
    }

    let skirt_depth = (cell_size * step as f32 * 2.0).max(0.5);
    add_skirts(&mut mesh, count, skirt_depth);
    mesh
}

/// Fingerprint of everything the chunk mesh depends on: the chunk, the LOD and the neighbour
/// samples on the shared border. A change in any of them produces a new mesh.
pub fn chunk_fingerprint(terrain: &Terrain, coord: ChunkCoord, step: usize) -> u64 {
    let cells = terrain.chunk_cells().max(1);
    let step = step.clamp(1, cells);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(coord.x as u32 as u64);
    mix(coord.z as u32 as u64);
    mix(cells as u64);
    mix(step as u64);
    mix(terrain.cell_size().to_bits() as u64);
    for hole in terrain.holes() {
        mix(hole.x.to_bits() as u64);
        mix(hole.z.to_bits() as u64);
        mix(hole.radius.to_bits() as u64);
    }
    for local_z in 0..=cells {
        for local_x in 0..=cells {
            let (_, _, height, material) =
                sample_world(terrain, coord, cells, terrain.cell_size(), local_x, local_z);
            mix(height.to_bits() as u64);
            mix(u64::from(material));
        }
    }
    hash
}

fn sample_world(
    terrain: &Terrain,
    coord: ChunkCoord,
    cells: usize,
    cell_size: f32,
    local_x: usize,
    local_z: usize,
) -> (f32, f32, f32, u8) {
    let global_x = coord.x as i64 * cells as i64 + local_x as i64;
    let global_z = coord.z as i64 * cells as i64 + local_z as i64;
    let height = terrain.sample(global_x, global_z).unwrap_or_else(|| {
        // No neighbour chunk: clamp to this chunk's border so the mesh stays closed.
        let clamped_x = local_x.min(cells - 1);
        let clamped_z = local_z.min(cells - 1);
        terrain
            .sample(
                coord.x as i64 * cells as i64 + clamped_x as i64,
                coord.z as i64 * cells as i64 + clamped_z as i64,
            )
            .unwrap_or(0.0)
    });
    let material = terrain.sample_material(global_x, global_z).unwrap_or(0);
    (
        global_x as f32 * cell_size,
        global_z as f32 * cell_size,
        height,
        material,
    )
}

fn add_skirts(mesh: &mut MeshData, count: usize, depth: f32) {
    if count < 2 {
        return;
    }
    let mut edges: Vec<Vec<u32>> = Vec::with_capacity(4);
    // Bottom (z = 0), right (x = count-1), top (z = count-1) and left (x = 0) borders.
    edges.push((0..count).map(|column| column as u32).collect());
    edges.push(
        (0..count)
            .map(|row| (row * count + count - 1) as u32)
            .collect(),
    );
    edges.push(
        (0..count)
            .rev()
            .map(|column| ((count - 1) * count + column) as u32)
            .collect(),
    );
    edges.push((0..count).rev().map(|row| (row * count) as u32).collect());

    for edge in edges {
        let mut lowered = Vec::with_capacity(edge.len());
        for index in &edge {
            let Some(position) = mesh.positions.get(*index as usize) else {
                continue;
            };
            let Some(normal) = mesh.normals.get(*index as usize) else {
                continue;
            };
            let Some(color) = mesh.colors.get(*index as usize) else {
                continue;
            };
            let new_index = mesh.positions.len() as u32;
            mesh.positions
                .push([position[0], position[1] - depth, position[2]]);
            mesh.normals.push(*normal);
            mesh.colors.push(*color);
            lowered.push(new_index);
        }
        for pair in 0..edge.len().saturating_sub(1) {
            let Some(&top_a) = edge.get(pair) else {
                continue;
            };
            let Some(&top_b) = edge.get(pair + 1) else {
                continue;
            };
            let Some(&bottom_a) = lowered.get(pair) else {
                continue;
            };
            let Some(&bottom_b) = lowered.get(pair + 1) else {
                continue;
            };
            mesh.indices
                .extend([top_a, bottom_a, top_b, top_b, bottom_a, bottom_b]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_terrain::{ChunkCoord, Terrain};

    fn terrain() -> Terrain {
        let mut terrain = Terrain::new(1.0, 4).unwrap();
        terrain
            .generate_region(
                ChunkCoord::new(0, 0),
                ChunkCoord::new(0, 0),
                &petunia_terrain::TerrainGeneration::default(),
            )
            .unwrap();
        terrain
    }

    #[test]
    fn lod_step_grows_with_distance() {
        assert_eq!(lod_step(0.0), 1);
        assert_eq!(lod_step(LOD_NEAR_DISTANCE + 1.0), 2);
        assert_eq!(lod_step(LOD_MID_DISTANCE + 1.0), 4);
    }

    #[test]
    fn a_chunk_mesh_has_positions_normals_colors_and_triangles() {
        let terrain = terrain();
        let mesh = chunk_mesh(&terrain, ChunkCoord::new(0, 0), 1);
        assert!(!mesh.positions.is_empty());
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.positions.len(), mesh.colors.len());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(
            mesh.positions
                .iter()
                .all(|position| position[1].is_finite())
        );
    }

    #[test]
    fn a_coarse_mesh_has_fewer_vertices_than_a_fine_one() {
        let terrain = terrain();
        let fine = chunk_mesh(&terrain, ChunkCoord::new(0, 0), 1);
        let coarse = chunk_mesh(&terrain, ChunkCoord::new(0, 0), 2);
        assert!(coarse.positions.len() < fine.positions.len());
    }

    #[test]
    fn the_fingerprint_changes_when_a_sample_changes() {
        let mut terrain = terrain();
        let before = chunk_fingerprint(&terrain, ChunkCoord::new(0, 0), 1);
        terrain.set_sample(2, 2, 9.0).unwrap();
        let after = chunk_fingerprint(&terrain, ChunkCoord::new(0, 0), 1);
        assert_ne!(before, after);
    }

    #[test]
    fn a_hole_removes_triangles_from_the_mesh() {
        let mut terrain = terrain();
        let full = chunk_mesh(&terrain, ChunkCoord::new(0, 0), 1).indices.len();
        terrain
            .set_holes(vec![petunia_terrain::TerrainHole {
                x: 2.0,
                z: 2.0,
                radius: 1.5,
            }])
            .unwrap();
        let holed = chunk_mesh(&terrain, ChunkCoord::new(0, 0), 1).indices.len();
        assert!(holed < full, "full {full}, holed {holed}");
    }

    #[test]
    fn a_chunk_sphere_contains_its_deepest_mesh() {
        // Two chunks side by side with different relief: the shared border must be inside the
        // bound too, or a visible chunk could be culled at its edge.
        let mut terrain = Terrain::new(1.0, 4).expect("terreno");
        terrain
            .generate_region(
                ChunkCoord::new(0, 0),
                ChunkCoord::new(1, 0),
                &petunia_terrain::TerrainGeneration {
                    fractal: petunia_terrain::FractalSettings {
                        amplitude: 6.0,
                        ..petunia_terrain::FractalSettings::default()
                    },
                    ..petunia_terrain::TerrainGeneration::default()
                },
            )
            .expect("gera");
        for coord in [ChunkCoord::new(0, 0), ChunkCoord::new(1, 0)] {
            let (center, radius) = chunk_sphere(&terrain, coord).expect("esfera");
            // The coarsest LOD hangs the deepest skirt: if the sphere holds that one, it holds all.
            let mesh = chunk_mesh(&terrain, coord, 4);
            for position in &mesh.positions {
                let point = Vec3::new(position[0], position[1], position[2]);
                let distance = (point - center).length();
                assert!(
                    distance <= radius + 1.0e-3,
                    "{point:?} do bloco {coord:?} ficou a {distance} da esfera (raio {radius})"
                );
            }
        }
        assert!(chunk_bounds(&terrain, ChunkCoord::new(0, 0)).is_some());
        assert!(chunk_bounds(&terrain, ChunkCoord::new(9, 9)).is_none());
    }

    #[test]
    fn layer_colors_fall_back_to_grass() {
        assert_eq!(Some(layer_color(0)), TERRAIN_LAYERS.first().copied());
        assert_eq!(Some(layer_color(7)), TERRAIN_LAYERS.get(7).copied());
        assert_eq!(Some(layer_color(200)), TERRAIN_LAYERS.first().copied());
    }
}
