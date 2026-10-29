use std::collections::BTreeMap;

use petunia_math::Vec3;

use crate::brush::{Brush, TerrainOperation};
use crate::chunk::{ChunkCoord, MAX_CHUNKS, MAX_HEIGHT, TerrainChunk, validate_cell_size};
use crate::error::TerrainError;
use crate::generate::{TerrainGeneration, generate_chunk};
use crate::noise::{FractalSettings, fractal};

/// Most samples one brush may touch. It keeps a typo (a 10 km brush) from freezing the editor.
pub const MAX_BRUSH_SAMPLES: i64 = 4_000_000;
/// Most holes one terrain may have.
pub const MAX_HOLES: usize = 4096;

/// How a stamped height patch combines with the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampMode {
    Replace,
    Add,
    Subtract,
    Min,
    Max,
}

/// A height patch to stamp on the terrain.
pub struct Stamp<'a> {
    pub center_x: f32,
    pub center_z: f32,
    pub width: usize,
    pub depth: usize,
    pub heights: &'a [f32],
    pub mode: StampMode,
    pub strength: f32,
}

/// A circular opening in the terrain surface, usually where a cave reaches it. Rendering skips
/// the cells inside it and physics stops colliding there, so the cave mesh can take over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainHole {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
}

/// A heightmap split into square chunks. Samples are owned by exactly one chunk, so there are no
/// duplicated edges and neighbours never disagree.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    cell_size: f32,
    chunk_cells: usize,
    chunks: BTreeMap<ChunkCoord, TerrainChunk>,
    holes: Vec<TerrainHole>,
}

impl Default for Terrain {
    /// An empty terrain with 1 m cells and 16×16-cell chunks.
    fn default() -> Self {
        Self {
            cell_size: 1.0,
            chunk_cells: 16,
            chunks: BTreeMap::new(),
            holes: Vec::new(),
        }
    }
}

impl Terrain {
    pub fn new(cell_size: f32, chunk_cells: usize) -> Result<Self, TerrainError> {
        validate_cell_size(cell_size)?;
        if chunk_cells == 0 || chunk_cells > crate::chunk::MAX_CHUNK_CELLS {
            return Err(TerrainError::InvalidChunkCells(chunk_cells));
        }
        Ok(Self {
            cell_size,
            chunk_cells,
            chunks: BTreeMap::new(),
            holes: Vec::new(),
        })
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn chunk_cells(&self) -> usize {
        self.chunk_cells
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunk(&self, coord: ChunkCoord) -> Option<&TerrainChunk> {
        self.chunks.get(&coord)
    }

    pub fn chunks(&self) -> impl Iterator<Item = &TerrainChunk> {
        self.chunks.values()
    }

    pub fn chunk_coords(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        self.chunks.keys().copied()
    }

    /// Creates the chunk when it does not exist yet.
    pub fn ensure_chunk(
        &mut self,
        coord: ChunkCoord,
        default_height: f32,
    ) -> Result<&mut TerrainChunk, TerrainError> {
        if !self.chunks.contains_key(&coord) {
            if self.chunks.len() >= MAX_CHUNKS {
                return Err(TerrainError::TooManyChunks(self.chunks.len()));
            }
            let chunk = TerrainChunk::new(coord, self.chunk_cells, default_height)?;
            self.chunks.insert(coord, chunk);
        }
        self.chunks
            .get_mut(&coord)
            .ok_or(TerrainError::UnknownChunk)
    }

    pub fn remove_chunk(&mut self, coord: ChunkCoord) -> Option<TerrainChunk> {
        self.chunks.remove(&coord)
    }

    /// Inserts a chunk built elsewhere (loading a file, an undo patch). The chunk must match
    /// this terrain's cell count and pass its own validation.
    pub fn insert_chunk(&mut self, chunk: TerrainChunk) -> Result<(), TerrainError> {
        chunk.validate()?;
        if chunk.cells() != self.chunk_cells {
            return Err(TerrainError::InvalidChunkCells(chunk.cells()));
        }
        if !self.chunks.contains_key(&chunk.coord()) && self.chunks.len() >= MAX_CHUNKS {
            return Err(TerrainError::TooManyChunks(self.chunks.len()));
        }
        self.chunks.insert(chunk.coord(), chunk);
        Ok(())
    }

    /// Checks the whole terrain against the crate limits.
    pub fn validate(&self) -> Result<(), TerrainError> {
        validate_cell_size(self.cell_size)?;
        if self.chunk_cells == 0 || self.chunk_cells > crate::chunk::MAX_CHUNK_CELLS {
            return Err(TerrainError::InvalidChunkCells(self.chunk_cells));
        }
        if self.chunks.len() > MAX_CHUNKS {
            return Err(TerrainError::TooManyChunks(self.chunks.len()));
        }
        for (coord, chunk) in &self.chunks {
            if chunk.coord() != *coord || chunk.cells() != self.chunk_cells {
                return Err(TerrainError::InvalidChunkCells(chunk.cells()));
            }
            chunk.validate()?;
        }
        if self.holes.len() > MAX_HOLES {
            return Err(TerrainError::TooManyChunks(self.holes.len()));
        }
        for hole in &self.holes {
            if !hole.x.is_finite() || !hole.z.is_finite() || !hole.radius.is_finite() {
                return Err(TerrainError::NonFinite("o furo do terreno"));
            }
            if hole.radius <= 0.0 {
                return Err(TerrainError::InvalidBrush(
                    "o raio do furo precisa ser positivo",
                ));
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn holes(&self) -> &[TerrainHole] {
        &self.holes
    }

    /// Replaces the holes. Every hole must be finite and have a positive radius.
    pub fn set_holes(&mut self, holes: Vec<TerrainHole>) -> Result<(), TerrainError> {
        if holes.len() > MAX_HOLES {
            return Err(TerrainError::TooManyChunks(holes.len()));
        }
        for hole in &holes {
            if !hole.x.is_finite() || !hole.z.is_finite() || !hole.radius.is_finite() {
                return Err(TerrainError::NonFinite("o furo do terreno"));
            }
            if hole.radius <= 0.0 || hole.radius > MAX_HEIGHT {
                return Err(TerrainError::InvalidBrush(
                    "o raio do furo precisa ser positivo",
                ));
            }
        }
        self.holes = holes;
        Ok(())
    }

    /// True when the world position is inside a hole.
    pub fn is_hole(&self, world_x: f32, world_z: f32) -> bool {
        self.holes.iter().any(|hole| {
            let offset_x = world_x - hole.x;
            let offset_z = world_z - hole.z;
            offset_x * offset_x + offset_z * offset_z <= hole.radius * hole.radius
        })
    }

    pub fn sample_count(&self) -> usize {
        self.chunks
            .values()
            .map(|chunk| chunk.cells() * chunk.cells())
            .sum()
    }

    /// Copies one chunk for undo. `None` means the chunk did not exist.
    pub fn capture_chunk(&self, coord: ChunkCoord) -> Option<TerrainChunk> {
        self.chunks.get(&coord).cloned()
    }

    /// Restores a chunk captured with [`Terrain::capture_chunk`]. `None` removes it.
    pub fn restore_chunk(
        &mut self,
        coord: ChunkCoord,
        chunk: Option<TerrainChunk>,
    ) -> Result<(), TerrainError> {
        match chunk {
            Some(chunk) => {
                if chunk.coord() != coord || chunk.cells() != self.chunk_cells {
                    return Err(TerrainError::InvalidChunkCells(chunk.cells()));
                }
                if !self.chunks.contains_key(&coord) && self.chunks.len() >= MAX_CHUNKS {
                    return Err(TerrainError::TooManyChunks(self.chunks.len()));
                }
                self.chunks.insert(coord, chunk);
            }
            None => {
                self.chunks.remove(&coord);
            }
        }
        Ok(())
    }

    /// Copies every chunk that overlaps `minimum..maximum` (inclusive) for one undo step.
    pub fn capture_region(
        &self,
        minimum: ChunkCoord,
        maximum: ChunkCoord,
    ) -> Vec<(ChunkCoord, Option<TerrainChunk>)> {
        let mut captured = Vec::new();
        for coord_x in minimum.x..=maximum.x {
            for coord_z in minimum.z..=maximum.z {
                let coord = ChunkCoord::new(coord_x, coord_z);
                captured.push((coord, self.capture_chunk(coord)));
            }
        }
        captured
    }

    pub fn restore_region(
        &mut self,
        captured: Vec<(ChunkCoord, Option<TerrainChunk>)>,
    ) -> Result<(), TerrainError> {
        for (coord, chunk) in captured {
            self.restore_chunk(coord, chunk)?;
        }
        Ok(())
    }

    /// Replaces one chunk with generated terrain.
    pub fn generate(
        &mut self,
        coord: ChunkCoord,
        settings: &TerrainGeneration,
    ) -> Result<(), TerrainError> {
        let chunk = generate_chunk(coord, self.chunk_cells, self.cell_size, settings)?;
        if !self.chunks.contains_key(&coord) && self.chunks.len() >= MAX_CHUNKS {
            return Err(TerrainError::TooManyChunks(self.chunks.len()));
        }
        self.chunks.insert(coord, chunk);
        Ok(())
    }

    /// Generates every chunk in the inclusive rectangle. All-or-nothing: the chunks are built
    /// before any of them is inserted.
    pub fn generate_region(
        &mut self,
        minimum: ChunkCoord,
        maximum: ChunkCoord,
        settings: &TerrainGeneration,
    ) -> Result<(), TerrainError> {
        let width = i64::from(maximum.x) - i64::from(minimum.x) + 1;
        let depth = i64::from(maximum.z) - i64::from(minimum.z) + 1;
        if width <= 0 || depth <= 0 {
            return Err(TerrainError::InvalidBrush(
                "o retângulo de geração está invertido",
            ));
        }
        let count = width.saturating_mul(depth);
        if count > MAX_CHUNKS as i64 {
            return Err(TerrainError::TooManyChunks(count as usize));
        }
        let mut built = Vec::with_capacity(count as usize);
        for coord_x in minimum.x..=maximum.x {
            for coord_z in minimum.z..=maximum.z {
                built.push(generate_chunk(
                    ChunkCoord::new(coord_x, coord_z),
                    self.chunk_cells,
                    self.cell_size,
                    settings,
                )?);
            }
        }
        for chunk in built {
            self.chunks.insert(chunk.coord(), chunk);
        }
        Ok(())
    }

    /// Height at a global sample. `None` when that chunk does not exist.
    pub fn sample(&self, global_x: i64, global_z: i64) -> Option<f32> {
        let (coord, local_x, local_z) = self.split_sample(global_x, global_z).ok()?;
        self.chunks.get(&coord)?.height(local_x, local_z)
    }

    pub fn sample_material(&self, global_x: i64, global_z: i64) -> Option<u8> {
        let (coord, local_x, local_z) = self.split_sample(global_x, global_z).ok()?;
        self.chunks.get(&coord)?.material(local_x, local_z)
    }

    /// Writes one global sample, creating its chunk when needed. Out-of-range heights are
    /// clamped, never wrapped.
    pub fn set_sample(
        &mut self,
        global_x: i64,
        global_z: i64,
        height: f32,
    ) -> Result<(), TerrainError> {
        if !height.is_finite() {
            return Err(TerrainError::NonFinite("a altura"));
        }
        let clamped = height.clamp(-MAX_HEIGHT, MAX_HEIGHT);
        let (coord, local_x, local_z) = self.split_sample(global_x, global_z)?;
        if !self.chunks.contains_key(&coord) {
            if self.chunks.len() >= MAX_CHUNKS {
                return Err(TerrainError::TooManyChunks(self.chunks.len()));
            }
            let chunk = TerrainChunk::new(coord, self.chunk_cells, 0.0)?;
            self.chunks.insert(coord, chunk);
        }
        let chunk = self
            .chunks
            .get_mut(&coord)
            .ok_or(TerrainError::UnknownChunk)?;
        chunk.set_height(local_x, local_z, clamped)
    }

    pub fn set_sample_material(
        &mut self,
        global_x: i64,
        global_z: i64,
        material: u8,
    ) -> Result<(), TerrainError> {
        let (coord, local_x, local_z) = self.split_sample(global_x, global_z)?;
        let chunk = self
            .chunks
            .get_mut(&coord)
            .ok_or(TerrainError::UnknownChunk)?;
        chunk.set_material(local_x, local_z, material)
    }

    /// Bilinear height at a world position. `None` when any of the four samples is missing.
    pub fn height_at(&self, world_x: f32, world_z: f32) -> Option<f32> {
        if !world_x.is_finite() || !world_z.is_finite() {
            return None;
        }
        let grid_x = world_x / self.cell_size;
        let grid_z = world_z / self.cell_size;
        let base_x = grid_x.floor();
        let base_z = grid_z.floor();
        let fraction_x = grid_x - base_x;
        let fraction_z = grid_z - base_z;
        let left = base_x as i64;
        let back = base_z as i64;
        let right = left.checked_add(1)?;
        let front = back.checked_add(1)?;
        let back_left = self.sample(left, back)?;
        let back_right = self.sample(right, back)?;
        let front_left = self.sample(left, front)?;
        let front_right = self.sample(right, front)?;
        let bottom = back_left + (back_right - back_left) * fraction_x;
        let top = front_left + (front_right - front_left) * fraction_x;
        Some(bottom + (top - bottom) * fraction_z)
    }

    /// Unit normal from central differences. `None` when the neighbourhood is missing.
    pub fn normal_at(&self, world_x: f32, world_z: f32) -> Option<Vec3> {
        let step = self.cell_size;
        let left = self.height_at(world_x - step, world_z)?;
        let right = self.height_at(world_x + step, world_z)?;
        let back = self.height_at(world_x, world_z - step)?;
        let front = self.height_at(world_x, world_z + step)?;
        let normal = Vec3::new(-(right - left), 2.0 * step, -(front - back));
        let unit = normal.normalized_or_zero();
        if unit.length_squared() > 0.0 {
            Some(unit)
        } else {
            None
        }
    }

    /// Applies one brush operation. Returns how many samples changed. The brush is a volume in
    /// world space, so it works from any camera angle (including from outside a cave).
    pub fn apply(
        &mut self,
        brush: &Brush,
        operation: TerrainOperation,
    ) -> Result<usize, TerrainError> {
        brush.validate()?;
        let (minimum, maximum) = brush.bounds();
        let cell = self.cell_size;
        let min_x = (minimum.x / cell).floor() as i64;
        let max_x = (maximum.x / cell).ceil() as i64;
        let min_z = (minimum.z / cell).floor() as i64;
        let max_z = (maximum.z / cell).ceil() as i64;
        let width = max_x.saturating_sub(min_x).saturating_add(1);
        let depth = max_z.saturating_sub(min_z).saturating_add(1);
        if width.saturating_mul(depth) > MAX_BRUSH_SAMPLES {
            return Err(TerrainError::InvalidBrush(
                "o pincel cobre amostras demais; reduza o raio",
            ));
        }

        let target_height = match operation {
            TerrainOperation::Flatten {
                target_height: Some(height),
            } => {
                if !height.is_finite() {
                    return Err(TerrainError::NonFinite("a altura do plano"));
                }
                height
            }
            TerrainOperation::Flatten {
                target_height: None,
            } => self
                .height_at(brush.center.x, brush.center.z)
                .unwrap_or(0.0),
            _ => 0.0,
        };
        let noise_settings = match operation {
            TerrainOperation::Noise {
                seed,
                feature_size,
                amplitude,
            } => {
                if !feature_size.is_finite() || feature_size <= 0.0 {
                    return Err(TerrainError::NonFinite("o tamanho da feição do ruído"));
                }
                if !amplitude.is_finite() {
                    return Err(TerrainError::NonFinite("a amplitude do ruído"));
                }
                Some(FractalSettings {
                    seed,
                    feature_size,
                    amplitude,
                    ..FractalSettings::default()
                })
            }
            _ => None,
        };
        let terrace_step = match operation {
            TerrainOperation::Terrace { step } => {
                if !step.is_finite() || step <= 0.0 {
                    return Err(TerrainError::NonFinite("o passo do terraço"));
                }
                step
            }
            _ => 0.0,
        };

        // Phase 1: read the samples and their weights. Nothing is written yet, so smoothing sees
        // the terrain as it was before the stroke (one pass, no order dependence).
        let mut touched: Vec<(i64, i64, f32, f32)> = Vec::new();
        for grid_z in min_z..=max_z {
            for grid_x in min_x..=max_x {
                let world_x = grid_x as f32 * cell;
                let world_z = grid_z as f32 * cell;
                let current = self.sample(grid_x, grid_z).unwrap_or(0.0);
                let point = Vec3::new(world_x, current, world_z);
                let weight = brush.weight(point);
                if weight <= 0.0 {
                    continue;
                }
                touched.push((grid_x, grid_z, current, weight));
            }
        }
        if touched.is_empty() {
            return Ok(0);
        }

        // Phase 2: compute the new heights.
        let mut updated: Vec<(i64, i64, f32)> = Vec::with_capacity(touched.len());
        for (grid_x, grid_z, current, weight) in touched {
            let blend = (weight * brush.strength).clamp(0.0, 1.0);
            let next = match operation {
                TerrainOperation::Raise => current + weight * brush.strength,
                TerrainOperation::Lower => current - weight * brush.strength,
                TerrainOperation::Smooth => {
                    let average = self.neighbour_average(grid_x, grid_z, current);
                    current + (average - current) * blend
                }
                TerrainOperation::Flatten { .. } => current + (target_height - current) * blend,
                TerrainOperation::Noise { .. } => {
                    let settings = noise_settings
                        .as_ref()
                        .ok_or(TerrainError::InvalidBrush("o ruído não foi configurado"))?;
                    let world_x = grid_x as f32 * cell;
                    let world_z = grid_z as f32 * cell;
                    current
                        + fractal(settings, world_x, world_z)
                            * settings.amplitude
                            * weight
                            * brush.strength
                }
                TerrainOperation::Terrace { .. } => {
                    let step = terrace_step.max(0.001);
                    let target = (current / step).round() * step;
                    current + (target - current) * blend
                }
            };
            if !next.is_finite() {
                continue;
            }
            updated.push((grid_x, grid_z, next.clamp(-MAX_HEIGHT, MAX_HEIGHT)));
        }

        // Phase 3: write.
        let mut changed = 0;
        for (grid_x, grid_z, next) in updated {
            let previous = self.sample(grid_x, grid_z);
            self.set_sample(grid_x, grid_z, next)?;
            if previous != Some(next) {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Stamps a rectangular height patch centered at `(center_x, center_z)`. `heights` is
    /// row-major with `width * depth` entries; each entry covers one cell. The patch never
    /// creates terrain outside the existing samples.
    pub fn stamp(&mut self, stamp: Stamp<'_>) -> Result<usize, TerrainError> {
        let Stamp {
            center_x,
            center_z,
            width,
            depth,
            heights,
            mode,
            strength,
        } = stamp;
        if width == 0 || depth == 0 || width > 4096 || depth > 4096 {
            return Err(TerrainError::InvalidBrush(
                "o carimbo aceita de 1 a 4096 células por lado",
            ));
        }
        let expected = width
            .checked_mul(depth)
            .ok_or(TerrainError::InvalidBrush("carimbo grande demais"))?;
        if heights.len() != expected {
            return Err(TerrainError::InvalidBrush(
                "o carimbo não tem uma altura por célula",
            ));
        }
        if !center_x.is_finite() || !center_z.is_finite() {
            return Err(TerrainError::NonFinite("o centro do carimbo"));
        }
        if !strength.is_finite() || strength < 0.0 {
            return Err(TerrainError::InvalidBrush(
                "a força do carimbo precisa ser positiva",
            ));
        }
        let cell = self.cell_size;
        let span_x = width as f32 * cell;
        let span_z = depth as f32 * cell;
        let origin_x = center_x - span_x * 0.5;
        let origin_z = center_z - span_z * 0.5;
        let mut changed = 0;
        for row in 0..depth {
            for column in 0..width {
                let Some(value) = heights.get(row * width + column).copied() else {
                    continue;
                };
                if !value.is_finite() {
                    return Err(TerrainError::NonFinite("a altura do carimbo"));
                }
                let world_x = origin_x + (column as f32 + 0.5) * cell;
                let world_z = origin_z + (row as f32 + 0.5) * cell;
                let grid_x = (world_x / cell).round() as i64;
                let grid_z = (world_z / cell).round() as i64;
                let Some(current) = self.sample(grid_x, grid_z) else {
                    continue;
                };
                let next = match mode {
                    StampMode::Replace => value,
                    StampMode::Add => current + value * strength,
                    StampMode::Subtract => current - value * strength,
                    StampMode::Min => current.min(value),
                    StampMode::Max => current.max(value),
                };
                if (next - current).abs() > 1.0e-6 {
                    self.set_sample(grid_x, grid_z, next)?;
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }

    /// Thermal erosion: material above the stable slope (`talus`, meters of height per cell)
    /// slides to the lowest neighbour. The deltas of one iteration are computed first and then
    /// applied together, so the result does not depend on iteration order. Returns how many
    /// samples changed.
    pub fn erode(
        &mut self,
        iterations: u32,
        talus: f32,
        strength: f32,
    ) -> Result<usize, TerrainError> {
        if iterations == 0 || iterations > 64 {
            return Err(TerrainError::InvalidBrush(
                "a erosão aceita de 1 a 64 iterações",
            ));
        }
        if !talus.is_finite() || talus < 0.0 {
            return Err(TerrainError::NonFinite("o ângulo de repouso (talus)"));
        }
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return Err(TerrainError::InvalidBrush(
                "a força da erosão precisa estar entre 0 e 1",
            ));
        }
        let cells = self.chunk_cells as i64;
        let coords: Vec<ChunkCoord> = self.chunks.keys().copied().collect();
        let mut changed = 0;
        for _ in 0..iterations {
            let mut deltas: BTreeMap<(i64, i64), f32> = BTreeMap::new();
            for coord in &coords {
                let origin_x = coord.x as i64 * cells;
                let origin_z = coord.z as i64 * cells;
                for local_z in 0..cells {
                    for local_x in 0..cells {
                        let grid_x = origin_x + local_x;
                        let grid_z = origin_z + local_z;
                        let Some(height) = self.sample(grid_x, grid_z) else {
                            continue;
                        };
                        let mut lowest = height;
                        let mut target = None;
                        for offset_z in -1..=1 {
                            for offset_x in -1..=1 {
                                if offset_x == 0 && offset_z == 0 {
                                    continue;
                                }
                                let Some(neighbour) =
                                    self.sample(grid_x + offset_x, grid_z + offset_z)
                                else {
                                    continue;
                                };
                                if neighbour < lowest {
                                    lowest = neighbour;
                                    target = Some((grid_x + offset_x, grid_z + offset_z));
                                }
                            }
                        }
                        let Some((target_x, target_z)) = target else {
                            continue;
                        };
                        let difference = height - lowest;
                        if difference <= talus || talus <= 0.0 && difference <= 0.0 {
                            continue;
                        }
                        let moved = (difference - talus) * strength * 0.5;
                        if moved <= 0.0 {
                            continue;
                        }
                        *deltas.entry((grid_x, grid_z)).or_insert(0.0) -= moved;
                        *deltas.entry((target_x, target_z)).or_insert(0.0) += moved;
                    }
                }
            }
            if deltas.is_empty() {
                break;
            }
            for ((grid_x, grid_z), delta) in deltas {
                if delta.abs() <= 1.0e-6 {
                    continue;
                }
                let height = self.sample(grid_x, grid_z).unwrap_or(0.0);
                self.set_sample(grid_x, grid_z, height + delta)?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Hydraulic erosion with deterministic droplets. Each droplet starts at a seeded sample,
    /// runs downhill carrying sediment, erodes where it accelerates and deposits where it slows.
    /// The droplets run in a fixed order with a fixed seed, so the same input gives the same
    /// terrain. Returns how many samples changed.
    #[allow(clippy::too_many_arguments)]
    pub fn hydraulic_erode(
        &mut self,
        droplets: u32,
        strength: f32,
        deposition: f32,
        evaporation: f32,
        seed: u64,
    ) -> Result<usize, TerrainError> {
        if droplets == 0 || droplets > 100_000 {
            return Err(TerrainError::InvalidBrush(
                "a erosão hidráulica aceita de 1 a 100000 gotas",
            ));
        }
        for (name, value) in [
            ("a força", strength),
            ("a deposição", deposition),
            ("a evaporação", evaporation),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(TerrainError::InvalidBrush(name));
            }
        }
        let cells = self.chunk_cells as i64;
        let mut minimum: Option<(i64, i64)> = None;
        let mut maximum: Option<(i64, i64)> = None;
        for coord in self.chunks.keys() {
            let min_x = coord.x as i64 * cells;
            let min_z = coord.z as i64 * cells;
            let max_x = min_x + cells - 1;
            let max_z = min_z + cells - 1;
            minimum = Some(match minimum {
                Some((x, z)) => (x.min(min_x), z.min(min_z)),
                None => (min_x, min_z),
            });
            maximum = Some(match maximum {
                Some((x, z)) => (x.max(max_x), z.max(max_z)),
                None => (max_x, max_z),
            });
        }
        let (Some((min_x, min_z)), Some((max_x, max_z))) = (minimum, maximum) else {
            return Ok(0);
        };
        if max_x < min_x || max_z < min_z {
            return Ok(0);
        }
        let width = (max_x - min_x + 1) as u64;
        let depth = (max_z - min_z + 1) as u64;
        let mut state = seed | 1;
        let mut changed = std::collections::BTreeSet::new();
        for _ in 0..droplets {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let start_x = min_x + (state % width) as i64;
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let start_z = min_z + (state % depth) as i64;
            let mut grid_x = start_x;
            let mut grid_z = start_z;
            let mut sediment = 0.0f32;
            let mut water = 1.0f32;
            for _ in 0..32 {
                let Some(height) = self.sample(grid_x, grid_z) else {
                    break;
                };
                let mut lowest = height;
                let mut target: Option<(i64, i64)> = None;
                for offset_z in -1..=1 {
                    for offset_x in -1..=1 {
                        if offset_x == 0 && offset_z == 0 {
                            continue;
                        }
                        let Some(neighbour) = self.sample(grid_x + offset_x, grid_z + offset_z)
                        else {
                            continue;
                        };
                        if neighbour < lowest {
                            lowest = neighbour;
                            target = Some((grid_x + offset_x, grid_z + offset_z));
                        }
                    }
                }
                let Some((target_x, target_z)) = target else {
                    break;
                };
                let slope = height - lowest;
                if slope <= 0.0 {
                    break;
                }
                let capacity = slope * water * 4.0;
                if sediment > capacity {
                    let deposit = (sediment - capacity) * deposition;
                    if deposit > 0.0 {
                        let next = (height + deposit).clamp(-MAX_HEIGHT, MAX_HEIGHT);
                        self.set_sample(grid_x, grid_z, next)?;
                        changed.insert((grid_x, grid_z));
                        sediment -= deposit;
                    }
                } else {
                    let erode = ((capacity - sediment) * strength).min(slope * 0.5);
                    if erode > 0.0 {
                        let next = (height - erode).clamp(-MAX_HEIGHT, MAX_HEIGHT);
                        self.set_sample(grid_x, grid_z, next)?;
                        changed.insert((grid_x, grid_z));
                        sediment += erode;
                    }
                }
                grid_x = target_x;
                grid_z = target_z;
                water *= 1.0 - evaporation;
                if water < 0.01 {
                    break;
                }
            }
        }
        Ok(changed.len())
    }

    /// Paints a material layer on every existing sample inside the brush. Samples that do not
    /// exist are skipped (painting never creates terrain). Returns how many samples changed.
    pub fn paint(&mut self, brush: &Brush, material: u8) -> Result<usize, TerrainError> {
        brush.validate()?;
        let (minimum, maximum) = brush.bounds();
        let cell = self.cell_size;
        let min_x = (minimum.x / cell).floor() as i64;
        let max_x = (maximum.x / cell).ceil() as i64;
        let min_z = (minimum.z / cell).floor() as i64;
        let max_z = (maximum.z / cell).ceil() as i64;
        let width = max_x.saturating_sub(min_x).saturating_add(1);
        let depth = max_z.saturating_sub(min_z).saturating_add(1);
        if width.saturating_mul(depth) > MAX_BRUSH_SAMPLES {
            return Err(TerrainError::InvalidBrush(
                "o pincel cobre amostras demais; reduza o raio",
            ));
        }
        let mut painted = 0;
        for grid_z in min_z..=max_z {
            for grid_x in min_x..=max_x {
                let Some(current) = self.sample(grid_x, grid_z) else {
                    continue;
                };
                let point = Vec3::new(grid_x as f32 * cell, current, grid_z as f32 * cell);
                if brush.weight(point) <= 0.0 {
                    continue;
                }
                if self.sample_material(grid_x, grid_z) != Some(material) {
                    self.set_sample_material(grid_x, grid_z, material)?;
                    painted += 1;
                }
            }
        }
        Ok(painted)
    }

    /// Axis-aligned bounds of every sample, including height. `None` when the terrain is empty.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut chunks = self.chunks.values();
        let first = chunks.next()?;
        let (first_minimum, first_maximum) = first.bounds(self.cell_size);
        let mut minimum = Vec3::new(first_minimum.x, f32::INFINITY, first_minimum.z);
        let mut maximum = Vec3::new(first_maximum.x, f32::NEG_INFINITY, first_maximum.z);
        for chunk in std::iter::once(first).chain(chunks) {
            let (chunk_minimum, chunk_maximum) = chunk.bounds(self.cell_size);
            minimum.x = minimum.x.min(chunk_minimum.x);
            minimum.z = minimum.z.min(chunk_minimum.z);
            maximum.x = maximum.x.max(chunk_maximum.x);
            maximum.z = maximum.z.max(chunk_maximum.z);
            for height in chunk.heights() {
                minimum.y = minimum.y.min(*height);
                maximum.y = maximum.y.max(*height);
            }
        }
        Some((minimum, maximum))
    }

    /// Chunk coordinates that overlap the inclusive brush rectangle.
    pub fn brush_chunk_region(&self, brush: &Brush) -> (ChunkCoord, ChunkCoord) {
        let (minimum, maximum) = brush.bounds();
        let side = self.chunk_cells as f32 * self.cell_size;
        let min_x = (minimum.x / side).floor() as i32;
        let max_x = (maximum.x / side).floor() as i32;
        let min_z = (minimum.z / side).floor() as i32;
        let max_z = (maximum.z / side).floor() as i32;
        (ChunkCoord::new(min_x, min_z), ChunkCoord::new(max_x, max_z))
    }

    fn split_sample(
        &self,
        global_x: i64,
        global_z: i64,
    ) -> Result<(ChunkCoord, usize, usize), TerrainError> {
        let cells = self.chunk_cells as i64;
        let chunk_x = global_x.div_euclid(cells);
        let chunk_z = global_z.div_euclid(cells);
        let local_x = global_x.rem_euclid(cells) as usize;
        let local_z = global_z.rem_euclid(cells) as usize;
        let chunk_x =
            i32::try_from(chunk_x).map_err(|_| TerrainError::CoordinateOutOfRange(global_x))?;
        let chunk_z =
            i32::try_from(chunk_z).map_err(|_| TerrainError::CoordinateOutOfRange(global_z))?;
        Ok((ChunkCoord::new(chunk_x, chunk_z), local_x, local_z))
    }

    fn neighbour_average(&self, grid_x: i64, grid_z: i64, fallback: f32) -> f32 {
        let mut total = 0.0;
        let mut count = 0.0;
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                if offset_x == 0 && offset_z == 0 {
                    continue;
                }
                if let Some(height) = self.sample(grid_x + offset_x, grid_z + offset_z) {
                    total += height;
                    count += 1.0;
                }
            }
        }
        if count > 0.0 { total / count } else { fallback }
    }
}
