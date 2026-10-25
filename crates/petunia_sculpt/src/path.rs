//! Tunnels from a path: click a few points, get a smooth passage. Two points give a straight
//! capsule; three or more are joined by a Catmull-Rom spline (it passes through every point) and
//! cut into short capsules, so the strokes are the same kind a brush drag makes.

use petunia_math::Vec3;

use crate::{Stroke, StrokeMode};

/// A path never turns into more strokes than this.
const MAX_PATH_STROKES: usize = 400;

/// The strokes that dig (or build) a passage of `radius` along `points`.
pub fn tunnel_strokes(points: &[Vec3], mode: StrokeMode, radius: f32, smooth: f32) -> Vec<Stroke> {
    let stroke = |start: Vec3, end: Vec3| Stroke::capsule(mode, start, end, radius, smooth);
    match points {
        [] => Vec::new(),
        [only] => vec![stroke(*only, *only)],
        [first, second] => vec![stroke(*first, *second)],
        _ => {
            let spacing = (radius * 1.5).max(0.75);
            let mut samples = vec![*points.first().unwrap_or(&Vec3::ZERO)];
            for index in 0..points.len() - 1 {
                let before = points.get(index.saturating_sub(1));
                let (Some(start), Some(end)) = (points.get(index), points.get(index + 1)) else {
                    continue;
                };
                let after = points.get(index + 2).unwrap_or(end);
                let before = before.unwrap_or(start);
                let pieces = (((*end - *start).length() / spacing).ceil() as usize).clamp(1, 32);
                for piece in 1..=pieces {
                    let t = piece as f32 / pieces as f32;
                    samples.push(catmull_rom(*before, *start, *end, *after, t));
                }
            }
            samples
                .windows(2)
                .take(MAX_PATH_STROKES)
                .filter_map(|pair| Some(stroke(*pair.first()?, *pair.get(1)?)))
                .collect()
        }
    }
}

fn catmull_rom(before: Vec3, start: Vec3, end: Vec3, after: Vec3, t: f32) -> Vec3 {
    let (t2, t3) = (t * t, t * t * t);
    (start * 2.0
        + (end - before) * t
        + (before * 2.0 - start * 5.0 + end * 4.0 - after) * t2
        + (start * 3.0 - before - end * 3.0 + after) * t3)
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_points_make_one_capsule_and_a_bend_makes_a_chain_through_every_point() {
        let straight = tunnel_strokes(
            &[Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)],
            StrokeMode::Carve,
            1.0,
            0.5,
        );
        assert_eq!(straight.len(), 1);
        let corner = Vec3::new(5.0, 0.0, 0.0);
        let bent = tunnel_strokes(
            &[Vec3::ZERO, corner, Vec3::new(5.0, 0.0, 5.0)],
            StrokeMode::Carve,
            1.0,
            0.5,
        );
        assert!(bent.len() > 4);
        assert!(
            bent.iter()
                .any(|stroke| (stroke.end - corner).length() < 1.0e-4),
            "the spline passes through the clicked point"
        );
        for pair in bent.windows(2) {
            assert!(
                (pair[0].end - pair[1].start).length() < 1.0e-5,
                "a continuous chain"
            );
        }
    }
}
