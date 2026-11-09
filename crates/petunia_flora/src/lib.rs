//! Vegetation by rules (phase 5, "vegetação por regras com impostores").
//!
//! A person paints rules, not plants: "where the slope is gentle and the ground is above 2 m, put
//! grass, one every 1.5 m, between 0.8 and 1.2 scale". [`scatter`] turns that into instances.
//!
//! Two things make it usable on a real map:
//!
//! - **Deterministic.** Every placement is a function of the rule's seed and the cell, never of
//!   `rand()`; the same map and the same rules give the same forest on every machine
//!   (`AGENTS.md` §2).
//! - **Impostor-ready.** An instance carries a facing and a size and no geometry: the renderer draws
//!   a camera-facing quad with the texture of the species, which is what makes a forest of ten
//!   thousand plants affordable.
//!
//! The rules read the **heightfield**, which already exists in `petunia_world`, so this crate does
//! not care how the ground was made (sculpted, stamped, imported).

use petunia_math::{Color, Vec3};
use petunia_world::HeightChunk;
use serde::{Deserialize, Serialize};

/// Most instances one scatter may produce. A rule that matches everywhere on a large map must not
/// silently allocate millions of quads.
pub const MAX_INSTANCES: usize = 120_000;

/// What one species is placed by.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// Species name; the renderer looks up its texture by it.
    pub species: String,
    /// How far apart plants are placed, in meters.
    pub spacing: f32,
    /// Smallest and largest multiplier of the species base size.
    pub scale_range: (f32, f32),
    /// Ground must be inside this band, in meters.
    pub height_range: (f32, f32),
    /// Ground slope must be at most this, in degrees (0 is flat, 90 is a wall).
    pub maximum_slope_degrees: f32,
    /// How many plants of this species may share a cell (0 and 1 are the same: one per cell).
    pub density: u32,
    /// Seed for the per-cell variation; changing it changes the forest, nothing else.
    pub seed: u64,
    /// Tint variance: how far the colour may drift from the species colour.
    pub color_variance: f32,
    pub color: Color,
}

impl Rule {
    /// A rule that matches nothing, so a half-filled rule is harmless.
    #[must_use]
    pub fn empty(species: &str) -> Self {
        Self {
            species: species.to_string(),
            spacing: 2.0,
            scale_range: (1.0, 1.0),
            height_range: (f32::MIN, f32::MAX),
            maximum_slope_degrees: 90.0,
            density: 1,
            seed: 0,
            color_variance: 0.0,
            color: Color::rgb(0.3, 0.5, 0.25),
        }
    }

    fn validate(&self) -> Result<(), FloraError> {
        if self.species.trim().is_empty() {
            return Err(FloraError::BadRule("a espécie não tem nome".to_string()));
        }
        if !self.spacing.is_finite() || self.spacing <= 0.05 || self.spacing > 200.0 {
            return Err(FloraError::BadRule(format!(
                "o espaçamento de `{}` precisa ficar entre 0,05 e 200 m",
                self.species
            )));
        }
        let (low, high) = self.scale_range;
        if !low.is_finite() || !high.is_finite() || low <= 0.0 || high < low {
            return Err(FloraError::BadRule(format!(
                "a faixa de escala de `{}` está invertida ou não é positiva",
                self.species
            )));
        }
        if !self.maximum_slope_degrees.is_finite()
            || self.maximum_slope_degrees < 0.0
            || self.maximum_slope_degrees > 90.0
        {
            return Err(FloraError::BadRule(format!(
                "a inclinação máxima de `{}` precisa ficar entre 0 e 90 graus",
                self.species
            )));
        }
        Ok(())
    }
}

/// One plant: where, how big, which way it faces, which species.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub position: Vec3,
    /// Facing around Y, in degrees: an impostor turns toward the camera, but the shadow and the
    /// tint follow this, so a forest does not look like a single stamp.
    pub yaw_degrees: f32,
    pub scale: f32,
    pub color: Color,
    /// Index into the rule list, so the renderer knows which texture to use.
    pub species: u16,
}

/// Why a scatter was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FloraError {
    /// A rule is not usable; the sentence says which and why.
    BadRule(String),
    /// The rules produced more plants than the budget allows.
    TooMany,
}

impl std::fmt::Display for FloraError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRule(reason) => formatter.write_str(reason),
            Self::TooMany => formatter.write_str("as regras geram plantas demais"),
        }
    }
}

/// Places every plant the rules ask for, over the area the field covers.
///
/// The walk is by cell, in a fixed order, so the result is deterministic before any randomness is
/// involved; the per-cell variation comes from a hash of the cell and the rule's seed.
pub fn scatter(field: &HeightChunk, rules: &[Rule]) -> Result<Vec<Instance>, FloraError> {
    for rule in rules {
        rule.validate()?;
    }
    let mut instances = Vec::new();
    let origin = field.world_origin();
    let side = field.world_size();
    for (index, rule) in rules.iter().enumerate() {
        let step = rule.spacing;
        let per_side = (side / step).ceil().max(1.0);
        let count = (per_side * per_side) as u32;
        let density = rule.density.max(1);
        for cell in 0..count.saturating_mul(density) {
            if instances.len() >= MAX_INSTANCES {
                return Err(FloraError::TooMany);
            }
            let hash = hash_of(rule.seed, u64::from(cell));
            let column = (cell % per_side as u32) as f32;
            let row = (cell / per_side as u32) as f32;
            // Jitter inside the cell, so the grid does not show.
            let jitter = |bits: u64| ((bits & 0xFFFF) as f32 / 65_535.0) - 0.5;
            let x = origin.x + (column + 0.5 + jitter(hash) * 0.8) * step;
            let z = origin.z + (row + 0.5 + jitter(hash >> 16) * 0.8) * step;
            let height = field.sample_world(x, z);
            if height < rule.height_range.0 || height > rule.height_range.1 {
                continue;
            }
            if slope_degrees(field, x, z) > rule.maximum_slope_degrees {
                continue;
            }
            let scale_mix = (hash >> 32) as f32 / u32::MAX as f32;
            let (low, high) = rule.scale_range;
            let scale = low + (high - low) * scale_mix;
            let yaw = ((hash >> 40) & 0xFFFF) as f32 / 65_535.0 * 360.0;
            let drift = (hash >> 48) as f32 / 65_535.0 * 2.0 - 1.0;
            let color = Color::new(
                (rule.color.r + drift * rule.color_variance).clamp(0.0, 1.0),
                (rule.color.g + drift * rule.color_variance).clamp(0.0, 1.0),
                (rule.color.b + drift * rule.color_variance).clamp(0.0, 1.0),
                rule.color.a,
            );
            instances.push(Instance {
                position: Vec3::new(x, height, z),
                yaw_degrees: yaw,
                scale,
                color,
                species: u16::try_from(index).unwrap_or(0),
            });
        }
    }
    Ok(instances)
}

/// Places plants over every chunk the caller hands in.
///
/// The caller knows which chunks exist (the terrain keeps them by grid position), so this function
/// does not guess: a chunk with no data is simply not passed.
pub fn scatter_world(chunks: &[&HeightChunk], rules: &[Rule]) -> Result<Vec<Instance>, FloraError> {
    let mut instances = Vec::new();
    for chunk in chunks {
        let mut local = scatter(chunk, rules)?;
        instances.append(&mut local);
        if instances.len() >= MAX_INSTANCES {
            return Err(FloraError::TooMany);
        }
    }
    Ok(instances)
}

/// The slope of the ground at a world point, in degrees, from the field's own samples.
fn slope_degrees(field: &HeightChunk, x: f32, z: f32) -> f32 {
    // Half a meter is fine for a 16 m tile and stays cheap on a large one.
    let step = 0.5f32;
    let dx = field.sample_world(x + step, z) - field.sample_world(x - step, z);
    let dz = field.sample_world(x, z + step) - field.sample_world(x, z - step);
    let horizontal = 2.0 * step;
    let slope = (dx * dx + dz * dz).sqrt() / horizontal;
    slope.atan().to_degrees()
}

/// A stable hash of `(seed, cell)`: the same numbers always, on every machine.
fn hash_of(seed: u64, cell: u64) -> u64 {
    let mut value = seed ^ cell.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_field(size: usize) -> HeightChunk {
        HeightChunk::new([0, 0], size, 1.0)
    }

    #[test]
    fn a_flat_field_gets_plants_at_the_spacing() {
        let field = flat_field(16);
        let rule = Rule {
            spacing: 4.0,
            ..Rule::empty("grama")
        };
        let plants = scatter(&field, &[rule]).expect("scatter");
        assert!(!plants.is_empty(), "a flat field matches");
        // 16 m / 4 m = 4 by 4 cells.
        assert_eq!(plants.len(), 16);
        for plant in &plants {
            assert!(plant.position.y.abs() < 1e-5, "on the ground");
        }
    }

    #[test]
    fn the_same_rules_and_map_give_the_same_forest() {
        let field = flat_field(16);
        let rules = vec![Rule {
            spacing: 2.5,
            scale_range: (0.5, 1.5),
            color_variance: 0.2,
            seed: 42,
            ..Rule::empty("grama")
        }];
        let first = scatter(&field, &rules).expect("scatter");
        for _ in 0..6 {
            assert_eq!(scatter(&field, &rules).expect("scatter"), first);
        }
    }

    #[test]
    fn a_height_band_and_a_slope_limit_are_respected() {
        let mut field = flat_field(16);
        // A hill on one side: only the low half may grow anything.
        for z in 0..16 {
            for x in 8..16 {
                field.set(x, z, 10.0);
            }
        }
        let rule = Rule {
            spacing: 4.0,
            height_range: (-0.5, 0.5),
            ..Rule::empty("grama")
        };
        let plants = scatter(&field, &[rule]).expect("scatter");
        assert!(!plants.is_empty());
        for plant in &plants {
            assert!(
                plant.position.y < 0.5,
                "only the low ground: {:?}",
                plant.position
            );
            assert!(plant.position.x < 8.0, "the hill is excluded");
        }
    }

    #[test]
    fn scale_and_yaw_stay_inside_their_ranges() {
        let field = flat_field(12);
        let rule = Rule {
            spacing: 2.0,
            scale_range: (0.8, 1.2),
            ..Rule::empty("grama")
        };
        for plant in scatter(&field, &[rule]).expect("scatter") {
            assert!((0.8..=1.2).contains(&plant.scale), "{}", plant.scale);
            assert!((0.0..360.0).contains(&plant.yaw_degrees));
        }
    }

    #[test]
    fn a_bad_rule_is_refused_with_the_species_name() {
        let field = flat_field(8);
        let bad = Rule {
            spacing: -1.0,
            ..Rule::empty("samambaia")
        };
        let error = scatter(&field, &[bad]).expect_err("refused");
        let text = error.to_string();
        assert!(text.contains("samambaia"), "{text}");
    }

    #[test]
    fn an_inverted_scale_range_is_refused() {
        let field = flat_field(8);
        let bad = Rule {
            scale_range: (2.0, 1.0),
            ..Rule::empty("grama")
        };
        assert!(matches!(
            scatter(&field, &[bad]),
            Err(FloraError::BadRule(_))
        ));
    }

    #[test]
    fn a_huge_map_stops_at_the_budget_instead_of_allocating() {
        // A 600 m field with plants every 0.5 m would be 1.4 million; the budget must stop it.
        // A 256-cell chunk at 1 m is the largest the type allows; 0.5 m spacing overshoots.
        let field = HeightChunk::new([0, 0], 256, 1.0);
        let rule = Rule {
            spacing: 0.5,
            ..Rule::empty("grama")
        };
        assert_eq!(scatter(&field, &[rule]), Err(FloraError::TooMany));
    }
}
