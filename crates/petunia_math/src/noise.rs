//! Deterministic value noise for terrain detail.
//!
//! Own implementation, on purpose: determinism is a requirement (AGENTS.md, section 2), so the
//! sequence must be identical on every machine and on every run. Nothing here depends on a
//! dependency, on hash-map order or on the platform's float formatting.

/// Side of one lattice cell, in the units of the caller's coordinates (meters for terrain).
pub const NOISE_LATTICE_METERS: f32 = 1.5;

/// Octaves summed by [`fractal_noise_2d`]. Each one doubles the frequency and halves the weight.
const OCTAVES: u32 = 3;

/// Lattice indices beyond this are refused: at that distance the noise stops being meaningful and
/// the `i32` cast would stop being exact, so the far field is flat instead of a trap.
const LATTICE_LIMIT: f32 = 1.0e6;

/// A deterministic pseudo-random value in `-1..=1` for one lattice point.
pub fn lattice_value(x: i32, z: i32) -> f32 {
    let mut value = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841);
    value ^= value >> 15;
    value = value.wrapping_mul(0x2c1b_3c6d);
    value ^= value >> 12;
    value = value.wrapping_mul(0x297a_2d39);
    value ^= value >> 15;
    // The distribution only has to be even, not uniform to the last bit.
    (value as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Smooth value noise in `-1..=1` at `(x, z)`, with features one lattice cell wide.
///
/// Non-finite coordinates and coordinates far beyond `LATTICE_LIMIT` cells give `0.0`.
pub fn value_noise_2d(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let scaled_x = x / NOISE_LATTICE_METERS;
    let scaled_z = z / NOISE_LATTICE_METERS;
    let base_x = scaled_x.floor();
    let base_z = scaled_z.floor();
    if base_x.abs() > LATTICE_LIMIT || base_z.abs() > LATTICE_LIMIT {
        return 0.0;
    }
    let cell_x = base_x as i32;
    let cell_z = base_z as i32;
    let blend_x = smoothstep(scaled_x - base_x);
    let blend_z = smoothstep(scaled_z - base_z);
    let corner_first = lattice_value(cell_x, cell_z);
    let corner_second = lattice_value(cell_x + 1, cell_z);
    let corner_third = lattice_value(cell_x, cell_z + 1);
    let corner_fourth = lattice_value(cell_x + 1, cell_z + 1);
    let lower = corner_first + (corner_second - corner_first) * blend_x;
    let upper = corner_third + (corner_fourth - corner_third) * blend_x;
    lower + (upper - lower) * blend_z
}

/// Fractal noise in `-1..=1`: three octaves of [`value_noise_2d`], weighted 1, 1/2 and 1/4.
pub fn fractal_noise_2d(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let mut total = 0.0;
    let mut weight = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    for _ in 0..OCTAVES {
        total += value_noise_2d(x * frequency, z * frequency) * amplitude;
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    if weight <= 0.0 {
        return 0.0;
    }
    total / weight
}

/// Hermite ease between two lattice corners, so the noise has no visible grid seams.
fn smoothstep(value: f32) -> f32 {
    let clamped = value.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_lattice_point_always_gives_the_same_value() {
        assert_eq!(lattice_value(3, -7), lattice_value(3, -7));
        assert_ne!(lattice_value(3, -7), lattice_value(4, -7));
    }

    #[test]
    fn lattice_values_are_bounded_and_use_both_signs() {
        let mut lowest = f32::INFINITY;
        let mut highest = f32::NEG_INFINITY;
        for x in -40..40 {
            for z in -40..40 {
                let value = lattice_value(x, z);
                assert!((-1.0..=1.0).contains(&value), "{value} at {x},{z}");
                lowest = lowest.min(value);
                highest = highest.max(value);
            }
        }
        assert!(lowest < -0.5, "the noise must go below zero: {lowest}");
        assert!(highest > 0.5, "the noise must go above zero: {highest}");
    }

    #[test]
    fn value_noise_is_continuous_between_neighbours() {
        let mut previous = value_noise_2d(0.0, 0.0);
        let mut step = 0.05;
        while step < 20.0 {
            let value = value_noise_2d(step, step * 0.5);
            assert!(
                (value - previous).abs() < 0.4,
                "jump at {step}: {previous} -> {value}"
            );
            previous = value;
            step += 0.05;
        }
    }

    #[test]
    fn fractal_noise_has_detail_and_stays_bounded() {
        let mut lowest = f32::INFINITY;
        let mut highest = f32::NEG_INFINITY;
        let mut sum = 0.0;
        let mut sum_of_squares = 0.0;
        let mut count = 0.0;
        for sample in 0..400 {
            let position = sample as f32 * 0.25;
            let value = fractal_noise_2d(position, position * 0.7);
            assert!((-1.0..=1.0).contains(&value), "{value} at {position}");
            lowest = lowest.min(value);
            highest = highest.max(value);
            sum += value;
            sum_of_squares += value * value;
            count += 1.0;
        }
        assert!(highest - lowest > 0.4, "range {} is flat", highest - lowest);
        let mean = sum / count;
        let variance = sum_of_squares / count - mean * mean;
        assert!(variance > 0.005, "variance {variance} is too flat");
    }

    #[test]
    fn hostile_coordinates_are_flat() {
        assert_eq!(value_noise_2d(f32::NAN, 0.0), 0.0);
        assert_eq!(value_noise_2d(0.0, f32::INFINITY), 0.0);
        assert_eq!(value_noise_2d(1.0e12, -1.0e12), 0.0);
        assert_eq!(fractal_noise_2d(f32::NEG_INFINITY, 0.0), 0.0);
        assert_eq!(fractal_noise_2d(1.0e12, 0.0), 0.0);
    }
}
