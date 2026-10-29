use crate::biome::{Biome, LAYER_GRASS};
use crate::chunk::{ChunkCoord, TerrainChunk};
use crate::error::TerrainError;
use crate::noise::{FractalSettings, fractal};

/// Settings for the built-in terrain generator. The values are plain data so the editor can show
/// them and a project can save them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainGeneration {
    pub fractal: FractalSettings,
    /// Height of the flat reference plane, in meters.
    pub base_height: f32,
    /// Which world this is: it paints the materials. `None` leaves every sample on the first
    /// layer, which is what a plain heightfield (or an imported map) has.
    pub biome: Option<Biome>,
}

impl Default for TerrainGeneration {
    fn default() -> Self {
        Self {
            fractal: FractalSettings::default(),
            base_height: 0.0,
            biome: None,
        }
    }
}

impl TerrainGeneration {
    pub fn validate(&self) -> Result<(), TerrainError> {
        if !self.base_height.is_finite() {
            return Err(TerrainError::NonFinite("a altura base"));
        }
        if !self.fractal.feature_size.is_finite() || self.fractal.feature_size <= 0.0 {
            return Err(TerrainError::NonFinite("o tamanho da feição"));
        }
        if !self.fractal.amplitude.is_finite() || self.fractal.amplitude < 0.0 {
            return Err(TerrainError::NonFinite("a amplitude"));
        }
        if !self.fractal.lacunarity.is_finite() || !self.fractal.persistence.is_finite() {
            return Err(TerrainError::NonFinite("os parâmetros do ruído"));
        }
        Ok(())
    }

    /// Height of the generated surface at a world position, in meters.
    pub fn height_at(&self, world_x: f32, world_z: f32) -> f32 {
        self.base_height + fractal(&self.fractal, world_x, world_z) * self.fractal.amplitude
    }
}

/// Builds one chunk of generated terrain. Deterministic: the same settings and coordinates give
/// the same heights on every machine.
pub fn generate_chunk(
    coord: ChunkCoord,
    cells: usize,
    cell_size: f32,
    settings: &TerrainGeneration,
) -> Result<TerrainChunk, TerrainError> {
    settings.validate()?;
    let mut chunk = TerrainChunk::new(coord, cells, settings.base_height)?;
    let side = cells as f32 * cell_size;
    let origin_x = coord.x as f32 * side;
    let origin_z = coord.z as f32 * side;
    for local_z in 0..cells {
        for local_x in 0..cells {
            let world_x = origin_x + local_x as f32 * cell_size;
            let world_z = origin_z + local_z as f32 * cell_size;
            let height = settings.height_at(world_x, world_z);
            chunk.set_height(local_x, local_z, height)?;
            // The layer depends on this sample's height alone, so two chunks never disagree on
            // the border they share.
            let layer = settings.biome.map_or(LAYER_GRASS, |biome| {
                biome.material_at(height, settings.base_height, settings.fractal.amplitude)
            });
            chunk.set_material(local_x, local_z, layer)?;
        }
    }
    Ok(chunk)
}
