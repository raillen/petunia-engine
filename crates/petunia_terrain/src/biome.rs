//! Biomes: the same generator, five different worlds.
//!
//! A biome is plain data: the shape of the relief (amplitude, feature size, octaves) and the
//! material of each height band. Nothing is random except the choice itself, which is derived
//! from the seed, so a seed always gives the same biome on every machine.
//!
//! Materials are the eight terrain layers the renderer knows: 0 grass, 1 dirt, 2 rock, 3 sand,
//! 4 snow, 5 mud, 6 asphalt, 7 water bed.

use crate::generate::TerrainGeneration;
use crate::noise::FractalSettings;

/// Terrain layers the renderer paints, in order.
pub const LAYER_GRASS: u8 = 0;
pub const LAYER_DIRT: u8 = 1;
pub const LAYER_ROCK: u8 = 2;
pub const LAYER_SAND: u8 = 3;
pub const LAYER_SNOW: u8 = 4;
pub const LAYER_MUD: u8 = 5;

/// A world's overall look and relief.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    /// Soft green hills.
    Plains,
    /// Steeper green hills with bare soil on the slopes.
    Forest,
    /// Low dunes of sand with rock on the peaks.
    Desert,
    /// High, mostly white ground, with bare rock on the summits.
    Snow,
    /// Rough dark rock with ash on top.
    Volcano,
}

/// Every biome, in the order the seed picks them.
pub const BIOMES: [Biome; 5] = [
    Biome::Plains,
    Biome::Forest,
    Biome::Desert,
    Biome::Snow,
    Biome::Volcano,
];

impl Biome {
    /// The name used in files and in the command payload.
    pub fn name(self) -> &'static str {
        match self {
            Self::Plains => "plains",
            Self::Forest => "forest",
            Self::Desert => "desert",
            Self::Snow => "snow",
            Self::Volcano => "volcano",
        }
    }

    /// The Portuguese name the editor shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Plains => "Planície",
            Self::Forest => "Floresta",
            Self::Desert => "Deserto",
            Self::Snow => "Neve",
            Self::Volcano => "Vulcão",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "plains" | "planicie" | "planície" => Some(Self::Plains),
            "forest" | "floresta" => Some(Self::Forest),
            "desert" | "deserto" => Some(Self::Desert),
            "snow" | "neve" => Some(Self::Snow),
            "volcano" | "vulcao" | "vulcão" => Some(Self::Volcano),
            _ => None,
        }
    }

    /// The biome a seed picks. An empty terrain generated with a seed always looks the same, and
    /// the choice is spread over the five (the seed mixes first, so nearby seeds differ).
    pub fn from_seed(seed: u64) -> Self {
        let mixed = crate::noise::lattice_value(seed, 0, 0);
        // `lattice_value` is in -1..=1 and may repeat for close seeds: mix again with the seed.
        let unit = ((mixed + 1.0) * 0.5).clamp(0.0, 1.0);
        let index = ((unit * BIOMES.len() as f32) as usize).min(BIOMES.len() - 1);
        BIOMES.get(index).copied().unwrap_or(Self::Plains)
    }

    /// Relief this biome generates for a seed. Amplitude is the peak height before the seed's own
    /// variation, which keeps two plains worlds from looking identical.
    pub fn settings(self, seed: u64) -> TerrainGeneration {
        let (feature_size, amplitude, octaves, persistence, base_height) = match self {
            Self::Plains => (96.0, 6.0, 4, 0.45, 0.0),
            Self::Forest => (48.0, 14.0, 5, 0.55, 0.0),
            Self::Desert => (72.0, 3.5, 3, 0.40, 0.0),
            Self::Snow => (56.0, 22.0, 6, 0.50, 2.0),
            Self::Volcano => (40.0, 18.0, 6, 0.60, 1.0),
        };
        // A gentle per-seed variation (0.85 .. 1.15) so two worlds of the same biome differ.
        let jitter = 0.85 + 0.30 * ((crate::noise::lattice_value(seed, 7, 11) + 1.0) * 0.5);
        TerrainGeneration {
            fractal: FractalSettings {
                seed,
                feature_size,
                amplitude: amplitude * jitter,
                octaves,
                lacunarity: 2.0,
                persistence,
            },
            base_height,
            biome: Some(self),
        }
    }

    /// The layer a sample gets, from its own height only: no neighbour lookup, so two chunks
    /// painted from the same settings cannot disagree on the border.
    pub fn material_at(self, height: f32, base_height: f32, amplitude: f32) -> u8 {
        if !height.is_finite() {
            return LAYER_ROCK;
        }
        let span = amplitude.abs().max(0.001);
        let normalized = ((height - base_height) / span).clamp(-1.0, 1.0);
        let bands: &[(f32, u8)] = match self {
            // Below the threshold, the top band.
            Self::Plains => &[(-1.0, LAYER_GRASS), (0.55, LAYER_ROCK)],
            Self::Forest => &[(-1.0, LAYER_GRASS), (0.35, LAYER_DIRT), (0.75, LAYER_ROCK)],
            Self::Desert => &[(-1.0, LAYER_SAND), (0.5, LAYER_ROCK)],
            Self::Snow => &[(-1.0, LAYER_SNOW), (0.7, LAYER_ROCK)],
            Self::Volcano => &[(-1.0, LAYER_ROCK), (0.45, LAYER_MUD)],
        };
        let mut layer = bands.first().map_or(LAYER_GRASS, |band| band.1);
        for (threshold, material) in bands {
            if normalized >= *threshold {
                layer = *material;
            }
        }
        layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for biome in BIOMES {
            assert_eq!(Biome::parse(biome.name()), Some(biome));
        }
        assert_eq!(Biome::parse("Floresta"), None);
        assert_eq!(Biome::parse("floresta"), Some(Biome::Forest));
        assert_eq!(Biome::parse("planície"), Some(Biome::Plains));
        assert_eq!(Biome::parse("tundra"), None);
    }

    #[test]
    fn the_same_seed_picks_the_same_biome() {
        for seed in 0..64 {
            assert_eq!(Biome::from_seed(seed), Biome::from_seed(seed));
        }
    }

    #[test]
    fn the_seeds_reach_every_biome() {
        let mut seen: Vec<Biome> = Vec::new();
        for seed in 0..80 {
            let biome = Biome::from_seed(seed);
            if !seen.contains(&biome) {
                seen.push(biome);
            }
        }
        assert_eq!(seen.len(), BIOMES.len(), "biomas vistos: {seen:?}");
    }

    #[test]
    fn the_settings_of_every_biome_are_valid() {
        for biome in BIOMES {
            let settings = biome.settings(9);
            settings.validate().expect("válido");
            assert_eq!(settings.biome, Some(biome));
            assert_eq!(settings.fractal.seed, 9);
            assert!(settings.fractal.amplitude > 0.0);
        }
    }

    #[test]
    fn the_biomes_differ_in_relief() {
        let plains = Biome::Plains.settings(1).fractal;
        let snow = Biome::Snow.settings(1).fractal;
        assert!(
            snow.amplitude > plains.amplitude,
            "a neve tem montanhas mais altas"
        );
        assert!(plains.feature_size > snow.feature_size);
    }

    #[test]
    fn materials_follow_the_height_bands() {
        // Grass below 55 % of the amplitude and rock above it.
        assert_eq!(Biome::Plains.material_at(0.0, 0.0, 10.0), LAYER_GRASS);
        assert_eq!(Biome::Plains.material_at(9.0, 0.0, 10.0), LAYER_ROCK);
        // Sand low, rock high.
        assert_eq!(Biome::Desert.material_at(-5.0, 0.0, 10.0), LAYER_SAND);
        assert_eq!(Biome::Desert.material_at(6.0, 0.0, 10.0), LAYER_ROCK);
        // Snow covers almost everything; bare rock shows on the peaks.
        assert_eq!(Biome::Snow.material_at(1.0, 2.0, 20.0), LAYER_SNOW);
        assert_eq!(Biome::Snow.material_at(-10.0, 2.0, 20.0), LAYER_SNOW);
        assert_eq!(Biome::Snow.material_at(20.0, 2.0, 20.0), LAYER_ROCK);
        // Volcano: rock with ash on the peaks.
        assert_eq!(Biome::Volcano.material_at(0.0, 0.0, 18.0), LAYER_ROCK);
        assert_eq!(Biome::Volcano.material_at(12.0, 0.0, 18.0), LAYER_MUD);
        // A degenerate amplitude cannot divide by zero: the height lands on the base band.
        assert_eq!(Biome::Plains.material_at(0.0, 0.0, 0.0), LAYER_GRASS);
        assert_eq!(Biome::Plains.material_at(f32::NAN, 0.0, 10.0), LAYER_ROCK);
    }
}
