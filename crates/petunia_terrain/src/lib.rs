//! Chunked heightmap terrain (DR-33).
//!
//! The exterior of a world is a heightmap split into square chunks. Samples are owned by exactly
//! one chunk, so neighbouring chunks never disagree and there are no duplicated edges. Caves and
//! other organic volumes stay in `petunia_sculpt` as sparse 3D SDF chunks; this crate is the
//! heightfield side of the hybrid model.
//!
//! Brushes are **volumes in world space** (sphere, box, cylinder, capsule), not screen-space
//! stamps. That is what makes it possible to carve from outside a cave. Falloffs are data:
//! constant, linear, smooth and Gaussian.
//!
//! Generation uses the crate's own deterministic value noise and fractal Brownian motion, so the
//! same seed gives the same terrain on every machine. No GPU, no third-party noise library.

mod brush;
mod chunk;
mod error;
mod generate;
mod heightmap;
mod noise;
mod terrain;

pub use brush::{Brush, BrushShape, Falloff, TerrainOperation};
pub use chunk::{
    ChunkCoord, MAX_CELL_SIZE, MAX_CHUNK_CELLS, MAX_CHUNKS, MAX_HEIGHT, MIN_CELL_SIZE, TerrainChunk,
};
pub use error::TerrainError;
pub use generate::{TerrainGeneration, generate_chunk};
pub use heightmap::{
    HEIGHTMAP_HEADER_BYTES, HEIGHTMAP_MAGIC, HEIGHTMAP_VERSION, Heightmap, MAX_HEIGHTMAP_SAMPLES,
    MAX_HEIGHTMAP_SIDE,
};
pub use noise::{FractalSettings, fractal, lattice_value, value_noise};
pub use terrain::{MAX_BRUSH_SAMPLES, MAX_HOLES, Stamp, StampMode, Terrain, TerrainHole};

#[cfg(test)]
mod tests;
