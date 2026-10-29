//! Deterministic noise used to generate terrain. It is ours on purpose: the same seed must give
//! the same world on every machine, and "game noise" is determinism-critical (AGENTS.md §2).
//!
//! The hash is a 64-bit mix of the seed and the integer lattice coordinates. Value noise is
//! smooth enough for terrain and cheap; fractal Brownian motion sums octaves of it.

/// One lattice value in `-1.0..=1.0`, fully determined by the seed and the cell.
pub fn lattice_value(seed: u64, x: i64, z: i64) -> f32 {
    let mut mixed = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    mixed ^= mixed >> 33;
    let unit = (mixed >> 11) as f32 / (1u64 << 53) as f32;
    unit * 2.0 - 1.0
}

fn smootherstep(t: f32) -> f32 {
    let clamped = t.clamp(0.0, 1.0);
    clamped * clamped * clamped * (clamped * (clamped * 6.0 - 15.0) + 10.0)
}

/// Bilinear value noise in `-1.0..=1.0`.
pub fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = x.floor();
    let z0 = z.floor();
    let tx = smootherstep(x - x0);
    let tz = smootherstep(z - z0);
    let ix = x0 as i64;
    let iz = z0 as i64;
    let v00 = lattice_value(seed, ix, iz);
    let v10 = lattice_value(seed, ix + 1, iz);
    let v01 = lattice_value(seed, ix, iz + 1);
    let v11 = lattice_value(seed, ix + 1, iz + 1);
    let bottom = v00 + (v10 - v00) * tx;
    let top = v01 + (v11 - v01) * tx;
    bottom + (top - bottom) * tz
}

/// Parameters of a fractal terrain. All of them are validated before use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractalSettings {
    pub seed: u64,
    /// World-space size of one noise cell, in meters. Larger means broader hills.
    pub feature_size: f32,
    /// Peak height contributed by the first octave, in meters.
    pub amplitude: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub persistence: f32,
}

impl Default for FractalSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            feature_size: 64.0,
            amplitude: 12.0,
            octaves: 5,
            lacunarity: 2.0,
            persistence: 0.5,
        }
    }
}

/// Fractal Brownian motion in `-1.0..=1.0` (before amplitude). The world coordinates are in
/// meters; `settings.feature_size` controls the broadest octave.
pub fn fractal(settings: &FractalSettings, world_x: f32, world_z: f32) -> f32 {
    let feature = settings.feature_size.max(0.001);
    let mut frequency = 1.0 / feature;
    let mut amplitude = 1.0;
    let mut total = 0.0;
    let mut normalizer = 0.0;
    let octaves = settings.octaves.clamp(1, 16);
    for octave in 0..octaves {
        let octave_seed = settings
            .seed
            .wrapping_add((octave as u64).wrapping_mul(0xA24B_AED4_963E_E407));
        total += value_noise(octave_seed, world_x * frequency, world_z * frequency) * amplitude;
        normalizer += amplitude;
        frequency *= settings.lacunarity.max(0.01);
        amplitude *= settings.persistence.clamp(0.01, 0.99);
    }
    if normalizer > 0.0 {
        (total / normalizer).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
