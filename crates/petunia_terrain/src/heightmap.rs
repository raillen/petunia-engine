//! A heightmap as a file: export a patch of a terrain, edit it elsewhere, stamp it back.
//!
//! The format is the engine's own, so no third-party library and no ambiguity about units: a
//! 28-byte header (`PTHM`, version, size, cell size, origin) followed by one little-endian `f32`
//! per sample, row by row. Heights are meters, exactly as the terrain stores them.
//!
//! Everything here is data and bytes: no file system. The editor or the CLI reads the file and
//! hands the bytes over, so the format can be tested without touching a disk.

use crate::chunk::MAX_HEIGHT;
use crate::error::TerrainError;
use crate::terrain::Terrain;

/// First four bytes of every file: `PTHM`.
pub const HEIGHTMAP_MAGIC: [u8; 4] = *b"PTHM";
/// Version this crate reads and writes.
pub const HEIGHTMAP_VERSION: u32 = 1;
/// Header: magic (4), version (4), width (4), depth (4), cell size (4), origin x (4), origin z (4).
pub const HEIGHTMAP_HEADER_BYTES: usize = 28;
/// Largest side accepted, in samples.
pub const MAX_HEIGHTMAP_SIDE: usize = 2048;
/// Largest sample count accepted (4 Mi samples: a 2048 × 2048 map, 16 MB).
pub const MAX_HEIGHTMAP_SAMPLES: usize = 4_194_304;

/// A rectangular piece of heightfield with the placement it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Heightmap {
    /// Meters between neighbouring samples.
    pub cell_size: f32,
    /// World x of the first sample (column 0).
    pub origin_x: f32,
    /// World z of the first sample (row 0).
    pub origin_z: f32,
    /// Samples along x.
    pub width: usize,
    /// Samples along z.
    pub depth: usize,
    /// `width * depth` heights in meters, row by row.
    pub heights: Vec<f32>,
}

impl Heightmap {
    /// Validates and builds a heightmap. The same checks run when decoding a file.
    pub fn new(
        cell_size: f32,
        origin_x: f32,
        origin_z: f32,
        width: usize,
        depth: usize,
        heights: Vec<f32>,
    ) -> Result<Self, TerrainError> {
        if !cell_size.is_finite() || cell_size < crate::chunk::MIN_CELL_SIZE {
            return Err(TerrainError::HeightmapInvalid(
                "o tamanho da célula precisa ser um número finito a partir do mínimo",
            ));
        }
        if cell_size > crate::chunk::MAX_CELL_SIZE {
            return Err(TerrainError::HeightmapInvalid(
                "o tamanho da célula passou do máximo",
            ));
        }
        if !origin_x.is_finite() || !origin_z.is_finite() {
            return Err(TerrainError::HeightmapInvalid(
                "a origem precisa ser um par de números finitos",
            ));
        }
        if width == 0 || depth == 0 {
            return Err(TerrainError::HeightmapInvalid(
                "a largura e a profundidade precisam ser pelo menos 1",
            ));
        }
        if width > MAX_HEIGHTMAP_SIDE || depth > MAX_HEIGHTMAP_SIDE {
            return Err(TerrainError::HeightmapInvalid(
                "um lado passou de 2048 amostras",
            ));
        }
        let samples = width.saturating_mul(depth);
        if samples > MAX_HEIGHTMAP_SAMPLES {
            return Err(TerrainError::HeightmapInvalid(
                "o mapa passou de 4 milhões de amostras",
            ));
        }
        if heights.len() != samples {
            return Err(TerrainError::HeightmapInvalid(
                "a quantidade de alturas não bate com largura × profundidade",
            ));
        }
        if heights.iter().any(|height| !height.is_finite()) {
            return Err(TerrainError::HeightmapInvalid(
                "toda altura precisa ser um número finito",
            ));
        }
        if heights.iter().any(|height| height.abs() > MAX_HEIGHT) {
            return Err(TerrainError::HeightmapInvalid(
                "uma altura passou do limite de ±10000 m",
            ));
        }
        Ok(Self {
            cell_size,
            origin_x,
            origin_z,
            width,
            depth,
            heights,
        })
    }

    /// Reads a rectangle out of a terrain, starting at the sample nearest to `x` and `z`. The
    /// origin is snapped to the terrain lattice so an exported map stamped back lands on the same
    /// samples; samples with no chunk are 0 m.
    pub fn from_terrain(
        terrain: &Terrain,
        x: f32,
        z: f32,
        width: usize,
        depth: usize,
    ) -> Result<Self, TerrainError> {
        if !x.is_finite() || !z.is_finite() {
            return Err(TerrainError::HeightmapInvalid(
                "a origem precisa ser um par de números finitos",
            ));
        }
        if width == 0 || depth == 0 {
            return Err(TerrainError::HeightmapInvalid(
                "a largura e a profundidade precisam ser pelo menos 1",
            ));
        }
        let cell_size = terrain.cell_size();
        let first_x = (x / cell_size).round() as i64;
        let first_z = (z / cell_size).round() as i64;
        let mut heights = Vec::with_capacity(width.saturating_mul(depth));
        for row in 0..depth {
            for column in 0..width {
                let global_x = first_x.saturating_add(column as i64);
                let global_z = first_z.saturating_add(row as i64);
                heights.push(terrain.sample(global_x, global_z).unwrap_or(0.0));
            }
        }
        Self::new(
            cell_size,
            first_x as f32 * cell_size,
            first_z as f32 * cell_size,
            width,
            depth,
            heights,
        )
    }

    /// World position of the middle of the samples: what `terrain.stamp` expects as its center.
    pub fn stamp_center(&self) -> (f32, f32) {
        (
            self.origin_x + (self.width as f32 - 1.0) * self.cell_size * 0.5,
            self.origin_z + (self.depth as f32 - 1.0) * self.cell_size * 0.5,
        )
    }

    /// Height at a column and a row, `None` outside the map.
    pub fn height_at(&self, column: usize, row: usize) -> Option<f32> {
        if column >= self.width || row >= self.depth {
            return None;
        }
        self.heights.get(row * self.width + column).copied()
    }

    /// The file, ready to write.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEIGHTMAP_HEADER_BYTES + self.heights.len() * 4);
        bytes.extend_from_slice(&HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&HEIGHTMAP_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.width as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.depth as u32).to_le_bytes());
        bytes.extend_from_slice(&self.cell_size.to_le_bytes());
        bytes.extend_from_slice(&self.origin_x.to_le_bytes());
        bytes.extend_from_slice(&self.origin_z.to_le_bytes());
        for height in &self.heights {
            bytes.extend_from_slice(&height.to_le_bytes());
        }
        bytes
    }

    /// Reads a file. Every field is checked before the map exists: a truncated file, a wrong
    /// magic, a size the engine cannot hold or a `NaN` height is an error, not a partial map.
    pub fn decode(bytes: &[u8]) -> Result<Self, TerrainError> {
        if bytes.len() < HEIGHTMAP_HEADER_BYTES {
            return Err(TerrainError::HeightmapInvalid(
                "o arquivo é menor que o cabeçalho de 28 bytes",
            ));
        }
        let magic = bytes.get(0..4).unwrap_or_default();
        if magic != HEIGHTMAP_MAGIC {
            return Err(TerrainError::HeightmapInvalid(
                "o arquivo não começa com PTHM",
            ));
        }
        let version = read_u32(bytes, 4)?;
        if version != HEIGHTMAP_VERSION {
            return Err(TerrainError::HeightmapInvalid(
                "a versão do arquivo não é a 1",
            ));
        }
        let width = read_u32(bytes, 8)? as usize;
        let depth = read_u32(bytes, 12)? as usize;
        let cell_size = read_f32(bytes, 16)?;
        let origin_x = read_f32(bytes, 20)?;
        let origin_z = read_f32(bytes, 24)?;
        let samples = width
            .checked_mul(depth)
            .ok_or(TerrainError::HeightmapInvalid(
                "largura × profundidade estourou o limite",
            ))?;
        let expected = HEIGHTMAP_HEADER_BYTES
            .checked_add(
                samples
                    .checked_mul(4)
                    .ok_or(TerrainError::HeightmapInvalid(
                        "largura × profundidade estourou o limite",
                    ))?,
            )
            .ok_or(TerrainError::HeightmapInvalid(
                "largura × profundidade estourou o limite",
            ))?;
        if bytes.len() != expected {
            return Err(TerrainError::HeightmapInvalid(
                "o arquivo não tem exatamente uma altura por amostra",
            ));
        }
        let mut heights = Vec::with_capacity(samples);
        for index in 0..samples {
            heights.push(read_f32(bytes, HEIGHTMAP_HEADER_BYTES + index * 4)?);
        }
        Self::new(cell_size, origin_x, origin_z, width, depth, heights)
    }

    /// Sample count.
    pub fn sample_count(&self) -> usize {
        self.heights.len()
    }

    /// World x of a column.
    pub fn x_of(&self, column: usize) -> f32 {
        self.origin_x + column as f32 * self.cell_size
    }

    /// World z of a row.
    pub fn z_of(&self, row: usize) -> f32 {
        self.origin_z + row as f32 * self.cell_size
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, TerrainError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(TerrainError::HeightmapInvalid("o arquivo termina no meio"))?;
    let array: [u8; 4] = slice
        .try_into()
        .map_err(|_| TerrainError::HeightmapInvalid("o arquivo termina no meio"))?;
    Ok(u32::from_le_bytes(array))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, TerrainError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(TerrainError::HeightmapInvalid("o arquivo termina no meio"))?;
    let array: [u8; 4] = slice
        .try_into()
        .map_err(|_| TerrainError::HeightmapInvalid("o arquivo termina no meio"))?;
    Ok(f32::from_le_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Heightmap {
        Heightmap::new(1.5, -8.0, 4.0, 3, 2, vec![0.5, -1.0, 2.0, 3.0, 4.5, 0.0]).expect("mapa")
    }

    #[test]
    fn a_heightmap_round_trips_exactly() {
        let original = map();
        let bytes = original.encode();
        assert_eq!(bytes.len(), HEIGHTMAP_HEADER_BYTES + 6 * 4);
        let decoded = Heightmap::decode(&bytes).expect("decodifica");
        assert_eq!(decoded, original);
    }

    #[test]
    fn the_decode_keeps_the_origin_and_the_cell_size() {
        let decoded = Heightmap::decode(&map().encode()).expect("decodifica");
        assert_eq!(decoded.x_of(0), -8.0);
        assert_eq!(decoded.x_of(2), -5.0);
        assert_eq!(decoded.z_of(1), 5.5);
        assert_eq!(decoded.height_at(2, 1), Some(0.0));
        assert_eq!(decoded.height_at(3, 0), None);
        assert_eq!(decoded.sample_count(), 6);
    }

    #[test]
    fn a_file_from_another_program_is_refused() {
        let mut bytes = map().encode();
        bytes[0] = b'X';
        assert!(Heightmap::decode(&bytes).is_err());
    }

    #[test]
    fn a_file_from_another_version_is_refused() {
        let mut bytes = map().encode();
        let version = 2_u32.to_le_bytes();
        bytes[4..8].copy_from_slice(&version);
        assert!(Heightmap::decode(&bytes).is_err());
    }

    #[test]
    fn a_truncated_or_padded_file_is_refused() {
        let bytes = map().encode();
        assert!(Heightmap::decode(&bytes[..bytes.len() - 1]).is_err());
        assert!(Heightmap::decode(&bytes[..10]).is_err());
        let mut padded = bytes.clone();
        padded.extend_from_slice(&[0, 0, 0, 0]);
        assert!(Heightmap::decode(&padded).is_err());
    }

    #[test]
    fn absurd_sizes_are_refused_before_allocating() {
        let mut bytes = map().encode();
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Heightmap::decode(&bytes).is_err());
        let mut wide = map().encode();
        wide[8..12].copy_from_slice(&4096_u32.to_le_bytes());
        assert!(Heightmap::decode(&wide).is_err());
    }

    #[test]
    fn a_non_finite_height_is_refused() {
        let mut bytes = map().encode();
        let nan = f32::NAN.to_le_bytes();
        let start = HEIGHTMAP_HEADER_BYTES;
        bytes[start..start + 4].copy_from_slice(&nan);
        assert!(Heightmap::decode(&bytes).is_err());
    }

    #[test]
    fn a_map_reads_the_terrain_grid_and_stamps_back_to_the_same_place() {
        let mut terrain = Terrain::new(1.0, 4).expect("terreno");
        for (index, height) in [1.0_f32, 2.0, 3.0, 4.0].into_iter().enumerate() {
            terrain
                .set_sample(index as i64 % 2, index as i64 / 2, height)
                .expect("amostra");
        }
        let map = Heightmap::from_terrain(&terrain, 0.4, -0.2, 2, 2).expect("mapa");
        // 0.4 and -0.2 snap to the lattice origin: the map keeps the terrain's own positions.
        assert_eq!(map.origin_x, 0.0);
        assert_eq!(map.origin_z, 0.0);
        assert_eq!(map.heights, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(map.stamp_center(), (0.5, 0.5));
    }

    #[test]
    fn a_map_reads_zero_outside_the_terrain() {
        let terrain = Terrain::new(1.0, 4).expect("terreno");
        let map = Heightmap::from_terrain(&terrain, 100.0, 100.0, 2, 2).expect("mapa");
        assert_eq!(map.heights, vec![0.0; 4]);
    }

    #[test]
    fn building_a_map_rejects_what_the_format_rejects() {
        assert!(Heightmap::new(1.0, 0.0, 0.0, 0, 4, Vec::new()).is_err());
        assert!(Heightmap::new(1.0, 0.0, 0.0, 2, 2, vec![0.0, 0.0, 0.0]).is_err());
        assert!(Heightmap::new(-1.0, 0.0, 0.0, 1, 1, vec![0.0]).is_err());
        assert!(Heightmap::new(1.0, f32::NAN, 0.0, 1, 1, vec![0.0]).is_err());
        assert!(Heightmap::new(1.0, 0.0, 0.0, 1, 1, vec![f32::INFINITY]).is_err());
        assert!(Heightmap::new(1.0, 0.0, 0.0, 1, 1, vec![0.0]).is_ok());
    }
}
