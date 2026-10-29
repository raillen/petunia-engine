//! Terrain stamps: named height shapes applied with the brush.
//!
//! A stamp is data, not a painting routine: the shape is a closed-form function of the offset
//! inside the brush disc. The same stamp on the same place always gives the same ground, and two
//! neighbouring stamps blend without a step because every shape is `0` at the edge of the disc.
//!
//! The height is `±1` at the strongest point and `0` at the edge, so the brush `strength` is
//! simply how many meters the shape moves the ground. A crater is a bowl with a banked rim; a mound
//! is the same idea with the sign flipped, which is why the shapes are not "raise" and "lower"
//! variants of each other but independent silhouettes.

use serde::{Deserialize, Serialize};

/// A named shape the terrain brush can stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StampShape {
    /// A bowl with a raised rim: bomb crater, pond bed, fire pit.
    Crater,
    /// A rounded hill that meets the ground at its edge.
    Mound,
    /// A flat top with a steep shoulder: terrace, shelf, pad for a building.
    Plateau,
    /// A crest line along the local x axis: embankment, dike, river bank.
    Ridge,
}

impl StampShape {
    /// Every shape, in the order the editor lists them.
    pub const ALL: [Self; 4] = [Self::Crater, Self::Mound, Self::Plateau, Self::Ridge];

    /// The canonical name, as it travels in files and commands.
    pub fn name(self) -> &'static str {
        match self {
            Self::Crater => "crater",
            Self::Mound => "mound",
            Self::Plateau => "plateau",
            Self::Ridge => "ridge",
        }
    }

    /// Parses the canonical name, plus the two synonyms a person is likely to type.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "crater" => Some(Self::Crater),
            "mound" | "hill" => Some(Self::Mound),
            "plateau" | "terrace" => Some(Self::Plateau),
            "ridge" | "dike" => Some(Self::Ridge),
            _ => None,
        }
    }

    /// The height multiplier at an offset from the centre, in units of the brush radius.
    ///
    /// `0` outside the disc, `±1` at the strongest point of the shape.
    pub fn height_at(self, offset_x: f32, offset_z: f32) -> f32 {
        if !offset_x.is_finite() || !offset_z.is_finite() {
            return 0.0;
        }
        match self {
            Self::Crater => crater(offset_x, offset_z),
            Self::Mound => mound(offset_x, offset_z),
            Self::Plateau => plateau(offset_x, offset_z),
            Self::Ridge => ridge(offset_x, offset_z),
        }
    }
}

/// Raises the ground in the middle and comes back to zero at the edge.
fn mound(offset_x: f32, offset_z: f32) -> f32 {
    let squared_distance = offset_x * offset_x + offset_z * offset_z;
    if squared_distance >= 1.0 {
        return 0.0;
    }
    // The 1.5 exponent makes the slope meet the ground tangentially, so the hill has no crease.
    (1.0 - squared_distance).powf(1.5)
}

/// Lowers a bowl in the middle and banks the material around it.
fn crater(offset_x: f32, offset_z: f32) -> f32 {
    let distance = (offset_x * offset_x + offset_z * offset_z).sqrt();
    if distance >= 1.0 {
        return 0.0;
    }
    // Bowl: -1 in the middle, back to 0 at 0.62 of the radius.
    let bowl = -(1.0 - smoothstep(0.0, 0.62, distance));
    // Rim: a low bank around the bowl, so the crater reads as an edge and not as a dent.
    let rim = (1.0 - smoothstep(0.0, 0.22, (distance - 0.72).abs())) * 0.35;
    // Nothing survives at the very edge: neighbouring stamps then join without a step.
    (bowl + rim) * (1.0 - smoothstep(0.85, 1.0, distance))
}

/// A flat top that drops down a shoulder: the useful shape for a shelf or a building pad.
fn plateau(offset_x: f32, offset_z: f32) -> f32 {
    let distance = (offset_x * offset_x + offset_z * offset_z).sqrt();
    if distance >= 1.0 {
        return 0.0;
    }
    1.0 - smoothstep(0.5, 0.9, distance)
}

/// A crest line along the local x axis: long in x, narrow across z.
fn ridge(offset_x: f32, offset_z: f32) -> f32 {
    if offset_x.abs() >= 1.0 || offset_z.abs() >= 1.0 {
        return 0.0;
    }
    // Half the radius is enough to give the crest a shoulder a player can walk on.
    const HALF_WIDTH: f32 = 0.45;
    let across = (offset_z / HALF_WIDTH).abs();
    if across >= 1.0 {
        return 0.0;
    }
    let crest = (1.0 - across * across).powf(1.5);
    crest * (1.0 - smoothstep(0.6, 0.95, offset_x.abs()))
}

/// The usual smoothstep, without a dependency and with a defined result when the edges coincide.
fn smoothstep(edge_start: f32, edge_end: f32, value: f32) -> f32 {
    let span = edge_end - edge_start;
    if span.abs() < f32::EPSILON {
        return if value < edge_start { 0.0 } else { 1.0 };
    }
    let position = ((value - edge_start) / span).clamp(0.0, 1.0);
    position * position * (3.0 - 2.0 * position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_round_trips() {
        for shape in StampShape::ALL {
            assert_eq!(
                StampShape::parse(shape.name()),
                Some(shape),
                "{}",
                shape.name()
            );
        }
        assert_eq!(StampShape::parse("cone"), None);
    }

    #[test]
    fn every_shape_is_bounded_and_flat_outside_the_disc() {
        for shape in StampShape::ALL {
            let mut strongest: f32 = 0.0;
            for step in 0..=40 {
                for other in 0..=40 {
                    let offset_x = -2.0 + step as f32 * 0.1;
                    let offset_z = -2.0 + other as f32 * 0.1;
                    let height = shape.height_at(offset_x, offset_z);
                    assert!(height.is_finite(), "{} is not finite", shape.name());
                    assert!(
                        height.abs() <= 1.0 + 1.0e-4,
                        "{} reached {height}",
                        shape.name()
                    );
                    strongest = strongest.max(height.abs());
                    if (offset_x * offset_x + offset_z * offset_z).sqrt() >= 1.0 {
                        assert_eq!(height, 0.0, "{} leaks outside the disc", shape.name());
                    }
                }
            }
            assert!(
                strongest > 0.9,
                "{} is too subtle: {strongest}",
                shape.name()
            );
        }
    }

    #[test]
    fn a_crater_dips_in_the_middle_and_banks_at_the_rim() {
        assert!(StampShape::Crater.height_at(0.0, 0.0) < -0.9);
        assert!(StampShape::Crater.height_at(0.72, 0.0) > 0.2);
        // From the bowl to the bank the ground rises without a flat gap in between.
        assert!(StampShape::Crater.height_at(0.45, 0.0) > -0.4);
        assert!(StampShape::Crater.height_at(0.62, 0.0) > 0.1);
    }

    #[test]
    fn a_mound_peaks_in_the_middle() {
        let shape = StampShape::Mound;
        assert!((shape.height_at(0.0, 0.0) - 1.0).abs() < 1.0e-6);
        assert!(shape.height_at(0.5, 0.0) > shape.height_at(0.9, 0.0));
        assert_eq!(shape.height_at(1.0, 0.0), 0.0);
    }

    #[test]
    fn a_plateau_is_flat_on_top_and_drops_at_the_shoulder() {
        let shape = StampShape::Plateau;
        assert!((shape.height_at(0.0, 0.0) - 1.0).abs() < 1.0e-6);
        assert!((shape.height_at(0.4, 0.0) - 1.0).abs() < 1.0e-6);
        assert!(shape.height_at(0.7, 0.0) < 0.75);
        assert_eq!(shape.height_at(0.95, 0.0), 0.0);
    }

    #[test]
    fn a_ridge_is_long_along_x_and_narrow_across_z() {
        let shape = StampShape::Ridge;
        assert!((shape.height_at(0.0, 0.0) - 1.0).abs() < 1.0e-6);
        assert!(shape.height_at(0.5, 0.0) > 0.5, "the crest runs along x");
        assert!(shape.height_at(0.0, 0.6) < 0.05, "the crest is narrow in z");
    }

    #[test]
    fn hostile_offsets_are_ignored() {
        for shape in StampShape::ALL {
            assert_eq!(shape.height_at(f32::NAN, 0.0), 0.0);
            assert_eq!(shape.height_at(0.0, f32::INFINITY), 0.0);
        }
    }
}
