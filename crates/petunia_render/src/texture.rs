//! The detail texture of the terrain: one tile per material layer, in a single atlas.
//!
//! The surface keeps its color per layer; the texture adds the grain that a flat color cannot
//! give (grass speckle, rock bands, sand ripples). It is generated here, from our own hash, so
//! a project does not depend on an image file, an image decoder or a file path, and the same
//! build always draws the same grain (AGENTS.md §2: determinism is ours).
//!
//! Layout: [`ATLAS_TILES`] square tiles side by side, tile `n` for layer `n`. Values are gray and
//! the shader multiplies the layer color by them; 0.5 is "no change".

/// Tiles in the atlas, one per terrain layer the renderer knows.
pub const ATLAS_TILES: usize = 8;
/// Pixels along one side of a tile.
pub const ATLAS_TILE_SIZE: usize = 64;
/// How many meters one tile covers on the ground.
pub const DETAIL_METERS_PER_TILE: f32 = 4.0;

/// Pixels of a texture, row-major, RGBA8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl TextureData {
    /// Pixel at `(x, y)`, or `(0, 0, 0, 0)` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let index = ((y * self.width + x) * 4) as usize;
        let mut pixel = [0u8; 4];
        if let Some(slice) = self.rgba.get(index..index + 4) {
            pixel.copy_from_slice(slice);
        }
        pixel
    }
}

/// Strength and shape of the grain of each layer: `(amplitude, kind)`.
/// Kind 0 is fine noise, 1 is wide blobs, 2 is bands, 3 is ripples.
const GRAIN: [(f32, u8); ATLAS_TILES] = [
    (0.16, 0), // grass: fine speckle
    (0.18, 1), // dirt: clods
    (0.14, 2), // rock: strata
    (0.12, 3), // sand: ripples
    (0.07, 0), // snow: very fine grain
    (0.17, 1), // mud: patches
    (0.15, 0), // asphalt: fine grain
    (0.10, 3), // water bed: silt ripples
];

/// The terrain atlas: every layer's grain, side by side.
pub fn terrain_atlas() -> TextureData {
    let tile = ATLAS_TILE_SIZE;
    let width = (tile * ATLAS_TILES) as u32;
    let height = tile as u32;
    let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    for layer in 0..ATLAS_TILES {
        let (amplitude, kind) = GRAIN.get(layer).copied().unwrap_or((0.1, 0));
        for y in 0..tile {
            for x in 0..tile {
                let value = grain_value(layer as u64, kind, amplitude, x, y);
                let byte = ((value.clamp(0.0, 1.0) * 255.0).round() as u32).min(255) as u8;
                let index = ((y as u32) * width + (layer * tile + x) as u32) * 4;
                if let Some(pixel) = rgba.get_mut(index as usize..index as usize + 4) {
                    pixel.copy_from_slice(&[byte, byte, byte, 255]);
                }
            }
        }
    }
    TextureData {
        width,
        height,
        rgba,
    }
}

/// One gray sample in `0..=1` around the middle (0.5 means "leave the color alone").
fn grain_value(layer: u64, kind: u8, amplitude: f32, x: usize, y: usize) -> f32 {
    let fx = x as f32 / ATLAS_TILE_SIZE as f32;
    let fy = y as f32 / ATLAS_TILE_SIZE as f32;
    let noise = match kind {
        // Fine noise: a different value per pixel.
        0 => hash_unit(layer, x as u64, y as u64),
        // Wide blobs: noise on a coarse grid, interpolated by nearest neighbour on 8x8 cells.
        1 => hash_unit(layer, (x / 8) as u64, (y / 8) as u64),
        // Bands: strata running across the tile, wobbled by the fine noise.
        2 => {
            let wobble = (hash_unit(layer, (x / 8) as u64, 3) - 0.5) * 0.5;
            let wave = ((fy * 9.0 + wobble) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            wave * 0.7 + hash_unit(layer, x as u64, y as u64) * 0.3
        }
        // Ripples: waves across the tile plus a little noise.
        _ => {
            let wave = ((fx * 6.0 + fy * 1.5) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            wave * 0.65 + hash_unit(layer, x as u64, y as u64) * 0.35
        }
    };
    (0.5 + (noise - 0.5) * 2.0 * amplitude).clamp(0.0, 1.0)
}

/// Deterministic value in `0..=1` for a tile and a pixel.
fn hash_unit(layer: u64, x: u64, y: u64) -> f32 {
    let mut mixed = layer.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ x.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ y.wrapping_mul(0x1656_67B1_9E37_79F9);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    mixed ^= mixed >> 33;
    let unit = (mixed >> 11) as f32 / (1u64 << 53) as f32;
    unit.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_has_one_tile_per_layer() {
        let atlas = terrain_atlas();
        assert_eq!(atlas.width, (ATLAS_TILES * ATLAS_TILE_SIZE) as u32);
        assert_eq!(atlas.height, ATLAS_TILE_SIZE as u32);
        assert_eq!(atlas.rgba.len(), (atlas.width * atlas.height * 4) as usize);
        // Every pixel is opaque.
        assert!(atlas.rgba.chunks(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn the_atlas_is_deterministic() {
        assert_eq!(terrain_atlas(), terrain_atlas());
    }

    #[test]
    fn the_tiles_do_not_look_alike() {
        let atlas = terrain_atlas();
        let tile = ATLAS_TILE_SIZE as u32;
        let first: Vec<u8> = (0..tile)
            .flat_map(|y| (0..tile).map(move |x| (x, y)))
            .map(|(x, y)| atlas.pixel(x, y)[0])
            .collect();
        for layer in 1..ATLAS_TILES {
            let other: Vec<u8> = (0..tile)
                .flat_map(|y| (0..tile).map(move |x| (x, y)))
                .map(|(x, y)| atlas.pixel(layer as u32 * tile + x, y)[0])
                .collect();
            assert_ne!(first, other, "as camadas 0 e {layer} têm o mesmo desenho");
        }
    }

    #[test]
    fn every_tile_has_grain_and_stays_near_the_middle() {
        let atlas = terrain_atlas();
        let tile = ATLAS_TILE_SIZE as u32;
        for layer in 0..ATLAS_TILES as u32 {
            let values: Vec<u8> = (0..tile)
                .flat_map(|y| (0..tile).map(move |x| (x, y)))
                .map(|(x, y)| atlas.pixel(layer * tile + x, y)[0])
                .collect();
            let lowest = *values.iter().min().unwrap_or(&0);
            let highest = *values.iter().max().unwrap_or(&0);
            assert!(highest > lowest, "a camada {layer} é lisa demais");
            // The grain never reaches black or white: it modulates the color, it does not replace it.
            assert!(lowest > 60, "camada {layer} escura demais: {lowest}");
            assert!(highest < 200, "camada {layer} clara demais: {highest}");
        }
    }

    #[test]
    fn a_pixel_outside_the_image_is_zero() {
        let atlas = terrain_atlas();
        assert_eq!(atlas.pixel(atlas.width, 0), [0, 0, 0, 0]);
        assert_eq!(atlas.pixel(0, atlas.height), [0, 0, 0, 0]);
    }
}
