use petunia_math::Vec3;

use crate::error::TerrainError;

/// Most cells a chunk may have along one side.
pub const MAX_CHUNK_CELLS: usize = 256;
/// Most chunks one terrain may hold.
pub const MAX_CHUNKS: usize = 16_384;
/// Tallest (and deepest) height a sample may store, in meters.
pub const MAX_HEIGHT: f32 = 10_000.0;
/// Smallest cell size, in meters.
pub const MIN_CELL_SIZE: f32 = 0.05;
/// Largest cell size, in meters.
pub const MAX_CELL_SIZE: f32 = 64.0;

/// Position of a chunk in the chunk grid. Chunk `(0, 0)` starts at the world origin and grows
/// toward positive `x` and `z`; negative coordinates are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// A square block of height samples. `heights` has `cells * cells` entries in row-major order
/// (`local_z * cells + local_x`). The sample at a chunk border is owned by exactly one chunk:
/// there is no duplicated edge, so neighbouring chunks cannot disagree.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainChunk {
    coord: ChunkCoord,
    cells: usize,
    heights: Vec<f32>,
    materials: Vec<u8>,
}

impl TerrainChunk {
    pub fn new(coord: ChunkCoord, cells: usize, default_height: f32) -> Result<Self, TerrainError> {
        if cells == 0 || cells > MAX_CHUNK_CELLS {
            return Err(TerrainError::InvalidChunkCells(cells));
        }
        if !default_height.is_finite() {
            return Err(TerrainError::NonFinite("a altura inicial"));
        }
        let count = cells
            .checked_mul(cells)
            .ok_or(TerrainError::InvalidChunkCells(cells))?;
        Ok(Self {
            coord,
            cells,
            heights: vec![default_height; count],
            materials: vec![0; count],
        })
    }

    /// Builds a chunk from saved data. `materials` may be empty (all zeros). Everything is
    /// validated before the chunk exists.
    pub fn from_parts(
        coord: ChunkCoord,
        cells: usize,
        heights: Vec<f32>,
        materials: Vec<u8>,
    ) -> Result<Self, TerrainError> {
        if cells == 0 || cells > MAX_CHUNK_CELLS {
            return Err(TerrainError::InvalidChunkCells(cells));
        }
        let expected = cells
            .checked_mul(cells)
            .ok_or(TerrainError::InvalidChunkCells(cells))?;
        if heights.len() != expected {
            return Err(TerrainError::InvalidBrush(
                "a lista de alturas não tem uma entrada por célula",
            ));
        }
        if !materials.is_empty() && materials.len() != expected {
            return Err(TerrainError::InvalidBrush(
                "a lista de materiais não tem uma entrada por célula",
            ));
        }
        for height in &heights {
            if !height.is_finite() {
                return Err(TerrainError::NonFinite("a altura"));
            }
            if height.abs() > MAX_HEIGHT {
                return Err(TerrainError::HeightOutsideLimit(*height));
            }
        }
        let materials = if materials.is_empty() {
            vec![0; expected]
        } else {
            materials
        };
        Ok(Self {
            coord,
            cells,
            heights,
            materials,
        })
    }

    /// Checks the chunk against the crate limits. Returns the first problem found.
    pub fn validate(&self) -> Result<(), TerrainError> {
        if self.cells == 0 || self.cells > MAX_CHUNK_CELLS {
            return Err(TerrainError::InvalidChunkCells(self.cells));
        }
        let expected = self
            .cells
            .checked_mul(self.cells)
            .ok_or(TerrainError::InvalidChunkCells(self.cells))?;
        if self.heights.len() != expected || self.materials.len() != expected {
            return Err(TerrainError::InvalidBrush(
                "o bloco não tem uma entrada por célula",
            ));
        }
        for height in &self.heights {
            if !height.is_finite() {
                return Err(TerrainError::NonFinite("a altura"));
            }
            if height.abs() > MAX_HEIGHT {
                return Err(TerrainError::HeightOutsideLimit(*height));
            }
        }
        Ok(())
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub fn cells(&self) -> usize {
        self.cells
    }

    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    pub fn materials(&self) -> &[u8] {
        &self.materials
    }

    pub fn height(&self, local_x: usize, local_z: usize) -> Option<f32> {
        self.index(local_x, local_z)
            .and_then(|index| self.heights.get(index).copied())
    }

    pub fn material(&self, local_x: usize, local_z: usize) -> Option<u8> {
        self.index(local_x, local_z)
            .and_then(|index| self.materials.get(index).copied())
    }

    pub fn set_height(
        &mut self,
        local_x: usize,
        local_z: usize,
        height: f32,
    ) -> Result<(), TerrainError> {
        if !height.is_finite() {
            return Err(TerrainError::NonFinite("a altura"));
        }
        if height.abs() > MAX_HEIGHT {
            return Err(TerrainError::HeightOutsideLimit(height));
        }
        let index = self
            .index(local_x, local_z)
            .ok_or(TerrainError::UnknownChunk)?;
        if let Some(slot) = self.heights.get_mut(index) {
            *slot = height;
            Ok(())
        } else {
            Err(TerrainError::UnknownChunk)
        }
    }

    pub fn set_material(
        &mut self,
        local_x: usize,
        local_z: usize,
        material: u8,
    ) -> Result<(), TerrainError> {
        let index = self
            .index(local_x, local_z)
            .ok_or(TerrainError::UnknownChunk)?;
        if let Some(slot) = self.materials.get_mut(index) {
            *slot = material;
            Ok(())
        } else {
            Err(TerrainError::UnknownChunk)
        }
    }

    /// Axis-aligned bounds of the chunk in world space, including the last sample's cell.
    pub fn bounds(&self, cell_size: f32) -> (Vec3, Vec3) {
        let side = self.cells as f32 * cell_size;
        let minimum = Vec3::new(self.coord.x as f32 * side, 0.0, self.coord.z as f32 * side);
        let maximum = Vec3::new(minimum.x + side, 0.0, minimum.z + side);
        (minimum, maximum)
    }

    fn index(&self, local_x: usize, local_z: usize) -> Option<usize> {
        if local_x >= self.cells || local_z >= self.cells {
            return None;
        }
        local_z
            .checked_mul(self.cells)
            .and_then(|row| row.checked_add(local_x))
    }
}

pub(crate) fn validate_cell_size(cell_size: f32) -> Result<(), TerrainError> {
    if !cell_size.is_finite() || cell_size < MIN_CELL_SIZE || cell_size > MAX_CELL_SIZE {
        return Err(TerrainError::InvalidCellSize(cell_size));
    }
    Ok(())
}
