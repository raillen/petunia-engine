//! Heightfield/chunks for terrain. Simple grid with deterministic operations.
//!
//! Data plus small free operations. Higher-level sculpt logic lives in `petunia_sculpt`.

use petunia_math::{Vec3, fractal_noise_2d};
use serde::{Deserialize, Serialize};

use crate::stamp::StampShape;

/// Cells per chunk side. The world origin of a chunk is `position * HEIGHT_CHUNK_SIZE * cell_size`.
pub const HEIGHT_CHUNK_SIZE: usize = 64;

/// A world-space brush never touches more chunks than this in one call.
const MAX_BRUSH_CHUNKS: usize = 1024;

/// A single heightfield chunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeightChunk {
    pub position: [i32; 2],
    pub size: usize,
    pub cell_size: f32,
    pub heights: Vec<f32>,
}

impl Default for HeightChunk {
    fn default() -> Self {
        Self {
            position: [0, 0],
            size: HEIGHT_CHUNK_SIZE,
            cell_size: 1.0,
            heights: vec![0.0; HEIGHT_CHUNK_SIZE * HEIGHT_CHUNK_SIZE],
        }
    }
}

impl HeightChunk {
    pub fn new(position: [i32; 2], size: usize, cell_size: f32) -> Self {
        let size_clamped = if size == 0 {
            HEIGHT_CHUNK_SIZE
        } else {
            size.min(256)
        };
        let cell = if cell_size.is_finite() && cell_size > 0.0 {
            cell_size
        } else {
            1.0
        };
        Self {
            position,
            size: size_clamped,
            cell_size: cell,
            heights: vec![0.0; size_clamped * size_clamped],
        }
    }

    /// World-space side length of the chunk in meters.
    pub fn world_size(&self) -> f32 {
        self.size as f32 * self.cell_size
    }

    /// World-space origin (minimum corner) of the chunk.
    pub fn world_origin(&self) -> Vec3 {
        Vec3::new(
            self.position[0] as f32 * self.world_size(),
            0.0,
            self.position[1] as f32 * self.world_size(),
        )
    }

    pub fn index(&self, x: usize, z: usize) -> Option<usize> {
        if x >= self.size || z >= self.size {
            return None;
        }
        Some(z * self.size + x)
    }

    pub fn get(&self, x: usize, z: usize) -> f32 {
        self.index(x, z)
            .and_then(|index| self.heights.get(index).copied())
            .unwrap_or(0.0)
    }

    pub fn set(&mut self, x: usize, z: usize, value: f32) {
        if !value.is_finite() {
            return;
        }
        if let Some(height) = self
            .index(x, z)
            .and_then(|index| self.heights.get_mut(index))
        {
            *height = value;
        }
    }

    /// Sample height at a world position (bilinear, clamped to the chunk).
    pub fn sample_world(&self, world_x: f32, world_z: f32) -> f32 {
        if !world_x.is_finite() || !world_z.is_finite() {
            return 0.0;
        }
        let origin = self.world_origin();
        let maximum = (self.size - 1) as f32;
        let local_x = ((world_x - origin.x) / self.cell_size).clamp(0.0, maximum);
        let local_z = ((world_z - origin.z) / self.cell_size).clamp(0.0, maximum);
        let x0 = local_x.floor() as usize;
        let z0 = local_z.floor() as usize;
        let x1 = (x0 + 1).min(self.size - 1);
        let z1 = (z0 + 1).min(self.size - 1);
        let blend_x = local_x - x0 as f32;
        let blend_z = local_z - z0 as f32;
        let first_row = self.get(x0, z0) * (1.0 - blend_x) + self.get(x1, z0) * blend_x;
        let second_row = self.get(x0, z1) * (1.0 - blend_x) + self.get(x1, z1) * blend_x;
        first_row * (1.0 - blend_z) + second_row * blend_z
    }

    /// Apply a brush at a world position: add or subtract with a smooth quadratic falloff.
    pub fn apply_brush(&mut self, world_position: Vec3, radius: f32, strength: f32, carve: bool) {
        self.apply_brush_with_falloff(world_position, radius, strength, carve, 0.5);
    }

    /// Apply a brush. `falloff` in `0..=1` controls edge sharpness: `0` is linear, `1` is sharp.
    pub fn apply_brush_with_falloff(
        &mut self,
        world_position: Vec3,
        radius: f32,
        strength: f32,
        carve: bool,
        falloff: f32,
    ) {
        if !world_position.is_finite() || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        if !strength.is_finite() || strength <= 0.0 {
            return;
        }
        let falloff_exponent = 1.0 + falloff.clamp(0.0, 1.0) * 4.0;
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                let delta_x = (x as f32 - center_x) * self.cell_size;
                let delta_z = (z as f32 - center_z) * self.cell_size;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                if distance > radius {
                    continue;
                }
                let weighted =
                    ((1.0 - (distance / radius).clamp(0.0, 1.0)).powf(falloff_exponent)) * strength;
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                let Some(height) = self.heights.get_mut(index) else {
                    continue;
                };
                if carve {
                    *height -= weighted;
                } else {
                    *height += weighted;
                }
            }
        }
    }

    /// Smooth a region towards the average of its neighbours, with falloff.
    pub fn smooth_region(&mut self, world_position: Vec3, radius: f32, strength: f32) {
        if !world_position.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !strength.is_finite()
        {
            return;
        }
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        let original = self.heights.clone();
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                let delta_x = (x as f32 - center_x) * self.cell_size;
                let delta_z = (z as f32 - center_z) * self.cell_size;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                if distance > radius {
                    continue;
                }
                let weight = ((1.0 - (distance / radius).clamp(0.0, 1.0)).powi(2)) * strength;
                let mut sum = 0.0;
                let mut count = 0.0f32;
                for offset_z in -1..=1 {
                    for offset_x in -1..=1 {
                        let sample_x = x + offset_x;
                        let sample_z = z + offset_z;
                        if sample_x < 0
                            || sample_z < 0
                            || sample_x >= self.size as i32
                            || sample_z >= self.size as i32
                        {
                            continue;
                        }
                        if let Some(index) = self
                            .index(sample_x as usize, sample_z as usize)
                            .and_then(|index| original.get(index))
                        {
                            sum += *index;
                            count += 1.0;
                        }
                    }
                }
                if count <= 0.0 {
                    continue;
                }
                let average = sum / count;
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                let Some(height) = self.heights.get_mut(index) else {
                    continue;
                };
                *height = *height * (1.0 - weight) + average * weight;
            }
        }
    }

    /// Erode a region: each cell hands material to its lowest neighbour, with falloff.
    ///
    /// One pass of thermal erosion. Slopes lose material and their foot gains it, like scree.
    /// Every cell reads the copy taken at the start, so the result does not depend on the
    /// iteration order and the same stamp always gives the same terrain.
    pub fn erode_region(&mut self, world_position: Vec3, radius: f32, strength: f32) {
        if !world_position.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !strength.is_finite()
        {
            return;
        }
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        let original = self.heights.clone();
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                let delta_x = (x as f32 - center_x) * self.cell_size;
                let delta_z = (z as f32 - center_z) * self.cell_size;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                if distance > radius {
                    continue;
                }
                let weight = ((1.0 - (distance / radius).clamp(0.0, 1.0)).powi(2)) * strength;
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                let Some(&height) = original.get(index) else {
                    continue;
                };
                let mut lowest = height;
                let mut lowest_index = None;
                for (offset_x, offset_z) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let sample_x = x + offset_x;
                    let sample_z = z + offset_z;
                    if sample_x < 0
                        || sample_z < 0
                        || sample_x >= self.size as i32
                        || sample_z >= self.size as i32
                    {
                        continue;
                    }
                    let neighbour = self
                        .index(sample_x as usize, sample_z as usize)
                        .and_then(|index| original.get(index).copied());
                    if let Some(neighbour) = neighbour
                        && neighbour < lowest
                    {
                        lowest = neighbour;
                        lowest_index = self.index(sample_x as usize, sample_z as usize);
                    }
                }
                let Some(lowest_index) = lowest_index else {
                    continue;
                };
                let moved = (height - lowest) * 0.25 * weight;
                if moved <= 0.0 {
                    continue;
                }
                if let Some(value) = self.heights.get_mut(index) {
                    *value -= moved;
                }
                if let Some(value) = self.heights.get_mut(lowest_index) {
                    *value += moved;
                }
            }
        }
    }

    /// Add fractal detail inside a brush, without changing the average height.
    ///
    /// The noise is anchored in **world space**, so the same place always gets the same ridges and
    /// neighbouring strokes join seamlessly. The detail is centred before it is applied, so a flat
    /// patch stays at the same average height: the brush roughens the ground, it does not raise it.
    pub fn roughen_region(&mut self, world_position: Vec3, radius: f32, strength: f32) {
        if !world_position.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !strength.is_finite()
            || strength <= 0.0
        {
            return;
        }
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let mut deltas: Vec<(usize, f32)> = Vec::new();
        let mut total = 0.0f32;
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                let delta_x = (x as f32 - center_x) * self.cell_size;
                let delta_z = (z as f32 - center_z) * self.cell_size;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                if distance > radius {
                    continue;
                }
                let weight = ((1.0 - (distance / radius).clamp(0.0, 1.0)).powi(2)) * strength;
                let world_x = origin.x + x as f32 * self.cell_size;
                let world_z = origin.z + z as f32 * self.cell_size;
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                let detail = fractal_noise_2d(world_x, world_z) * weight;
                total += detail;
                deltas.push((index, detail));
            }
        }
        if deltas.is_empty() {
            return;
        }
        // Centred, so the brush does not move the patch up or down as a whole.
        let mean = total / deltas.len() as f32;
        for (index, detail) in deltas {
            if let Some(height) = self.heights.get_mut(index) {
                *height += detail - mean;
            }
        }
    }

    /// Add a named shape ([`StampShape`]) inside the brush disc.
    ///
    /// `strength` is how many meters the shape reaches at its strongest point, and every shape
    /// returns to zero at the edge of the disc, so a stamp never leaves a step for the next one.
    pub fn stamp_region(
        &mut self,
        world_position: Vec3,
        radius: f32,
        strength: f32,
        shape: StampShape,
    ) {
        if !world_position.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !strength.is_finite()
            || strength <= 0.0
        {
            return;
        }
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                // The shape works in units of the radius, so the same stamp fits any brush size.
                let offset_x = (x as f32 - center_x) * self.cell_size / radius;
                let offset_z = (z as f32 - center_z) * self.cell_size / radius;
                let weight = shape.height_at(offset_x, offset_z);
                if weight == 0.0 {
                    continue;
                }
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                if let Some(height) = self.heights.get_mut(index) {
                    let stamped = *height + weight * strength;
                    if stamped.is_finite() {
                        *height = stamped;
                    }
                }
            }
        }
    }

    /// Flatten a region to `target_height` with falloff.
    pub fn flatten_region(
        &mut self,
        world_position: Vec3,
        radius: f32,
        target_height: f32,
        strength: f32,
    ) {
        if !world_position.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !target_height.is_finite()
            || !strength.is_finite()
        {
            return;
        }
        let strength = strength.clamp(0.0, 1.0);
        let origin = self.world_origin();
        let center_x = (world_position.x - origin.x) / self.cell_size;
        let center_z = (world_position.z - origin.z) / self.cell_size;
        let radius_cells = radius / self.cell_size;
        // The inner part snaps fully to the target; the rest blends out to the radius. Without
        // the plateau, a road bed would depend on how many stamps happen to overlap.
        let inner_radius = radius * 0.6;
        let minimum_x = ((center_x - radius_cells).floor() as i32).max(0);
        let maximum_x = ((center_x + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        let minimum_z = ((center_z - radius_cells).floor() as i32).max(0);
        let maximum_z = ((center_z + radius_cells).ceil() as i32).min(self.size as i32 - 1);
        for z in minimum_z..=maximum_z {
            for x in minimum_x..=maximum_x {
                let delta_x = (x as f32 - center_x) * self.cell_size;
                let delta_z = (z as f32 - center_z) * self.cell_size;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                if distance > radius {
                    continue;
                }
                let edge_blend = if distance <= inner_radius {
                    1.0
                } else {
                    let outer_span = radius - inner_radius;
                    let remaining = if outer_span > 1.0e-6 {
                        1.0 - (distance - inner_radius) / outer_span
                    } else {
                        0.0
                    };
                    remaining.clamp(0.0, 1.0).powi(2)
                };
                let weight = edge_blend * strength;
                let Some(index) = self.index(x as usize, z as usize) else {
                    continue;
                };
                let Some(height) = self.heights.get_mut(index) else {
                    continue;
                };
                *height = *height * (1.0 - weight) + target_height * weight;
            }
        }
    }
}

/// Heightfield containing multiple chunks.
#[derive(Clone, Debug, Default)]
pub struct Heightfield {
    pub chunks: Vec<HeightChunk>,
    pub default_cell_size: f32,
}

impl Heightfield {
    pub fn new(cell_size: f32) -> Self {
        Self {
            chunks: Vec::new(),
            default_cell_size: if cell_size.is_finite() && cell_size > 0.0 {
                cell_size
            } else {
                1.0
            },
        }
    }

    /// World-space side length of a chunk created by this heightfield.
    pub fn chunk_world_size(&self) -> f32 {
        HEIGHT_CHUNK_SIZE as f32 * self.default_cell_size
    }

    /// The chunk at `position`, created empty when missing. `None` only if the chunk vector is
    /// inconsistent, which callers treat as "nothing to do".
    pub fn get_or_create_chunk(&mut self, position: [i32; 2]) -> Option<&mut HeightChunk> {
        let index = if let Some(index) = self
            .chunks
            .iter()
            .position(|chunk| chunk.position == position)
        {
            index
        } else {
            self.chunks.push(HeightChunk::new(
                position,
                HEIGHT_CHUNK_SIZE,
                self.default_cell_size,
            ));
            self.chunks.len().saturating_sub(1)
        };
        self.chunks.get_mut(index)
    }

    pub fn chunk(&self, position: [i32; 2]) -> Option<&HeightChunk> {
        self.chunks.iter().find(|chunk| chunk.position == position)
    }

    /// Chunk coordinates that overlap a circle in the XZ plane, in deterministic order.
    pub fn chunks_touching(&self, center: Vec3, radius: f32) -> Vec<[i32; 2]> {
        let mut positions = Vec::new();
        if !center.is_finite() || !radius.is_finite() || radius < 0.0 {
            return positions;
        }
        let span = self.chunk_world_size();
        if span <= 0.0 {
            return positions;
        }
        let minimum_x = ((center.x - radius) / span).floor() as i32;
        let maximum_x = ((center.x + radius) / span).floor() as i32;
        let minimum_z = ((center.z - radius) / span).floor() as i32;
        let maximum_z = ((center.z + radius) / span).floor() as i32;
        for chunk_z in minimum_z..=maximum_z {
            for chunk_x in minimum_x..=maximum_x {
                if positions.len() >= MAX_BRUSH_CHUNKS {
                    return positions;
                }
                positions.push([chunk_x, chunk_z]);
            }
        }
        positions
    }

    /// Sampled terrain height under a world position, from the chunk that contains it.
    pub fn sample_world(&self, world_x: f32, world_z: f32) -> Option<f32> {
        self.chunks
            .iter()
            .find(|chunk| {
                let origin = chunk.world_origin();
                let size = chunk.world_size();
                world_x >= origin.x
                    && world_x < origin.x + size
                    && world_z >= origin.z
                    && world_z < origin.z + size
            })
            .map(|chunk| chunk.sample_world(world_x, world_z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_carve_lowers_center_more_than_edge() {
        let mut chunk = HeightChunk::new([0, 0], 16, 1.0);
        chunk.set(8, 8, 10.0);
        chunk.set(10, 8, 10.0);
        chunk.apply_brush(Vec3::new(8.0, 0.0, 8.0), 4.0, 1.0, true);
        let center = chunk.get(8, 8);
        let edge = chunk.get(10, 8);
        assert!(center < edge, "center {center} should be below edge {edge}");
        assert!(edge < 10.0);
    }

    #[test]
    fn flatten_moves_towards_the_target_height() {
        let mut chunk = HeightChunk::new([0, 0], 16, 1.0);
        chunk.set(8, 8, 10.0);
        chunk.flatten_region(Vec3::new(8.0, 0.0, 8.0), 3.0, 2.0, 1.0);
        assert!((chunk.get(8, 8) - 2.0).abs() < 1.0e-4);
    }

    #[test]
    fn erosion_moves_material_from_the_peak_to_the_foot() {
        let mut chunk = HeightChunk::new([0, 0], 16, 1.0);
        // A single peak with flat ground around it.
        chunk.set(8, 8, 4.0);
        let before = chunk.heights.iter().sum::<f32>();
        chunk.erode_region(Vec3::new(8.0, 0.0, 8.0), 3.0, 1.0);
        assert!(chunk.get(8, 8) < 4.0, "the peak lost material");
        let foot: f32 = [(7usize, 8usize), (9, 8), (8, 7), (8, 9)]
            .iter()
            .map(|(x, z)| chunk.get(*x, *z))
            .sum();
        assert!(foot > 0.0, "the foot gained material");
        let after = chunk.heights.iter().sum::<f32>();
        assert!(
            (before - after).abs() < 1.0e-3,
            "material is moved, not created: {before} -> {after}"
        );
    }

    #[test]
    fn erosion_leaves_flat_ground_and_hostile_arguments_alone() {
        let mut chunk = HeightChunk::new([0, 0], 8, 1.0);
        let flat = chunk.heights.clone();
        chunk.erode_region(Vec3::new(4.0, 0.0, 4.0), 3.0, 1.0);
        assert_eq!(chunk.heights, flat);
        chunk.set(4, 4, 5.0);
        chunk.erode_region(Vec3::new(4.0, 0.0, 4.0), 0.0, 1.0);
        chunk.erode_region(Vec3::new(f32::NAN, 0.0, 4.0), 2.0, 1.0);
        chunk.erode_region(Vec3::new(4.0, 0.0, 4.0), 2.0, f32::NAN);
        assert_eq!(chunk.get(4, 4), 5.0);
    }

    #[test]
    fn the_same_erosion_stamp_gives_the_same_terrain() {
        let mut first = HeightChunk::new([0, 0], 16, 1.0);
        let mut second = HeightChunk::new([0, 0], 16, 1.0);
        for chunk in [&mut first, &mut second] {
            for x in 0..16 {
                chunk.set(x, 8, x as f32 * 0.5);
            }
        }
        first.erode_region(Vec3::new(8.0, 0.0, 8.0), 5.0, 0.8);
        second.erode_region(Vec3::new(8.0, 0.0, 8.0), 5.0, 0.8);
        assert_eq!(first.heights, second.heights);
    }

    #[test]
    fn noise_brush_adds_detail_and_keeps_the_volume() {
        let mut chunk = HeightChunk::new([0, 0], 32, 1.0);
        let before = chunk.heights.iter().sum::<f32>();
        chunk.roughen_region(Vec3::new(16.0, 0.0, 16.0), 6.0, 2.0);
        let after = chunk.heights.iter().sum::<f32>();
        assert!(
            (before - after).abs() < 1.0e-2,
            "the brush roughens the ground instead of raising it: {before} -> {after}"
        );
        let mut detailed = 0;
        let mut changed_outside = 0;
        for z in 0..32 {
            for x in 0..32 {
                let height = chunk.get(x, z);
                let inside = {
                    let delta_x = x as f32 - 16.0;
                    let delta_z = z as f32 - 16.0;
                    (delta_x * delta_x + delta_z * delta_z).sqrt() <= 6.0
                };
                if inside && height.abs() > 1.0e-3 {
                    detailed += 1;
                }
                if !inside && height != 0.0 {
                    changed_outside += 1;
                }
            }
        }
        assert!(detailed > 20, "only {detailed} cells gained detail");
        assert_eq!(
            changed_outside, 0,
            "{changed_outside} cells outside the brush changed"
        );
    }

    #[test]
    fn the_same_noise_stamp_gives_the_same_terrain() {
        let mut first = HeightChunk::new([0, 0], 32, 1.0);
        let mut second = HeightChunk::new([0, 0], 32, 1.0);
        first.roughen_region(Vec3::new(16.0, 0.0, 16.0), 5.0, 1.5);
        second.roughen_region(Vec3::new(16.0, 0.0, 16.0), 5.0, 1.5);
        assert_eq!(first.heights, second.heights);
    }

    #[test]
    fn a_second_stroke_deepens_the_same_ridges() {
        let mut chunk = HeightChunk::new([0, 0], 32, 1.0);
        chunk.roughen_region(Vec3::new(16.0, 0.0, 16.0), 6.0, 1.0);
        let after_first = chunk.heights.clone();
        chunk.roughen_region(Vec3::new(16.0, 0.0, 16.0), 6.0, 1.0);
        // The detail is a field of the world, so painting the same place again reinforces the
        // ridges that are already there instead of inventing a new pattern.
        let reinforced = chunk
            .heights
            .iter()
            .zip(&after_first)
            .filter(|(now, before)| now.abs() > before.abs() + 1.0e-4)
            .count();
        assert!(reinforced > 20, "only {reinforced} cells were deepened");
    }

    #[test]
    fn hostile_brush_arguments_are_ignored() {
        let mut chunk = HeightChunk::new([0, 0], 8, 1.0);
        chunk.set(4, 4, 5.0);
        chunk.apply_brush(Vec3::new(4.0, 0.0, 4.0), 0.0, 1.0, true);
        chunk.apply_brush(Vec3::new(f32::NAN, 0.0, 4.0), 2.0, 1.0, true);
        chunk.flatten_region(Vec3::new(4.0, 0.0, 4.0), 2.0, f32::INFINITY, 1.0);
        chunk.roughen_region(Vec3::new(4.0, 0.0, 4.0), 0.0, 1.0);
        chunk.roughen_region(Vec3::new(f32::NAN, 0.0, 4.0), 2.0, 1.0);
        chunk.roughen_region(Vec3::new(4.0, 0.0, 4.0), 2.0, f32::NAN);
        chunk.roughen_region(Vec3::new(4.0, 0.0, 4.0), 2.0, -3.0);
        assert_eq!(chunk.get(4, 4), 5.0);
    }

    #[test]
    fn a_crater_stamp_lowers_the_middle_and_banks_the_rim() {
        let mut chunk = HeightChunk::new([0, 0], 16, 1.0);
        let flat = chunk.heights.clone();
        chunk.stamp_region(Vec3::new(8.0, 0.0, 8.0), 6.0, 2.0, StampShape::Crater);
        assert!(
            chunk.get(8, 8) < -1.5,
            "the middle is a bowl: {}",
            chunk.get(8, 8)
        );
        assert!(
            chunk.get(12, 8) > 0.1,
            "the rim is a bank: {}",
            chunk.get(12, 8)
        );
        assert_eq!(chunk.get(0, 0), flat[0], "the corner is outside the brush");
    }

    #[test]
    fn a_mound_stamp_never_lowers_the_ground() {
        let mut chunk = HeightChunk::new([0, 0], 16, 1.0);
        chunk.stamp_region(Vec3::new(8.0, 0.0, 8.0), 5.0, 1.5, StampShape::Mound);
        assert!((chunk.get(8, 8) - 1.5).abs() < 1.0e-3);
        assert!(chunk.heights.iter().all(|height| *height >= 0.0));
    }

    #[test]
    fn the_same_stamp_gives_the_same_terrain() {
        let mut first = HeightChunk::new([0, 0], 16, 1.0);
        let mut second = HeightChunk::new([0, 0], 16, 1.0);
        for chunk in [&mut first, &mut second] {
            chunk.stamp_region(Vec3::new(8.0, 0.0, 8.0), 6.0, 1.0, StampShape::Ridge);
        }
        assert_eq!(first.heights, second.heights);
    }

    #[test]
    fn hostile_stamp_arguments_are_ignored() {
        let mut chunk = HeightChunk::new([0, 0], 8, 1.0);
        chunk.set(4, 4, 5.0);
        let flat = chunk.heights.clone();
        chunk.stamp_region(Vec3::new(4.0, 0.0, 4.0), 0.0, 1.0, StampShape::Crater);
        chunk.stamp_region(Vec3::new(f32::NAN, 0.0, 4.0), 2.0, 1.0, StampShape::Crater);
        chunk.stamp_region(Vec3::new(4.0, 0.0, 4.0), 2.0, f32::NAN, StampShape::Crater);
        chunk.stamp_region(Vec3::new(4.0, 0.0, 4.0), 2.0, -3.0, StampShape::Crater);
        assert_eq!(chunk.heights, flat);
    }

    #[test]
    fn chunks_touching_covers_neighbours_and_is_bounded() {
        let heightfield = Heightfield::new(1.0);
        let span = heightfield.chunk_world_size();
        let positions = heightfield.chunks_touching(Vec3::new(span - 1.0, 0.0, 0.0), 2.0);
        assert!(positions.contains(&[0, 0]));
        assert!(positions.contains(&[1, 0]));
        let huge = heightfield.chunks_touching(Vec3::new(0.0, 0.0, 0.0), f32::MAX);
        assert!(huge.len() <= MAX_BRUSH_CHUNKS);
    }

    #[test]
    fn sample_world_is_bilinear_inside_the_chunk() {
        let mut chunk = HeightChunk::new([0, 0], 4, 1.0);
        chunk.set(0, 0, 0.0);
        chunk.set(1, 0, 4.0);
        chunk.set(0, 1, 2.0);
        chunk.set(1, 1, 6.0);
        assert!((chunk.sample_world(0.5, 0.5) - 3.0).abs() < 1.0e-5);
        assert!((chunk.sample_world(-5.0, -5.0) - 0.0).abs() < 1.0e-5);
    }
}
