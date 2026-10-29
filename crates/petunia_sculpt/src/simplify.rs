//! Consolidation: a long drag is dozens of tiny capsules end to end. Chains of the same kind
//! (same mode, radius and blend, each starting where the last ended) are simplified into fewer,
//! longer capsules that stay within `tolerance` of the original path. Only **add** and **carve**
//! chains are merged: they are unions, so a longer capsule is the same shape; inflate, flatten and
//! smooth build up with every dab and keep their count.

use petunia_math::Vec3;

use crate::{Stroke, StrokeMode, axis_distance};

/// The strokes after merging chains. The result is never longer than the input.
pub fn consolidate(strokes: &[Stroke], tolerance: f32) -> Vec<Stroke> {
    let mut merged: Vec<Stroke> = Vec::with_capacity(strokes.len());
    let mut index = 0;
    while let Some(first) = strokes.get(index) {
        index += 1;
        if !matches!(first.mode, StrokeMode::Add | StrokeMode::Carve) {
            merged.push(*first);
            continue;
        }
        let mut path = vec![first.start, first.end];
        while let Some(next) = strokes.get(index) {
            let continues = next.mode == first.mode
                && next.radius == first.radius
                && next.smooth == first.smooth
                && path
                    .last()
                    .is_some_and(|last| (next.start - *last).length() < 1.0e-4);
            if !continues {
                break;
            }
            path.push(next.end);
            index += 1;
        }
        let kept = simplify_path(&path, tolerance);
        if kept.len() < 2 {
            merged.push(*first);
            continue;
        }
        for pair in kept.windows(2) {
            if let (Some(start), Some(end)) = (pair.first(), pair.get(1)) {
                merged.push(Stroke {
                    start: *start,
                    end: *end,
                    ..*first
                });
            }
        }
    }
    merged
}

/// Douglas-Peucker without recursion: keeps the end points and every point farther than
/// `tolerance` from the line between the points kept around it.
fn simplify_path(path: &[Vec3], tolerance: f32) -> Vec<Vec3> {
    let mut keep = vec![false; path.len()];
    if let Some(first) = keep.first_mut() {
        *first = true;
    }
    if let Some(last) = keep.last_mut() {
        *last = true;
    }
    let mut pending = vec![(0usize, path.len().saturating_sub(1))];
    while let Some((from, to)) = pending.pop() {
        let (Some(start), Some(end)) = (path.get(from), path.get(to)) else {
            continue;
        };
        let mut farthest = (0.0f32, None);
        for between in from + 1..to {
            if let Some(point) = path.get(between) {
                let distance = axis_distance(*point, *start, *end);
                if distance > farthest.0 {
                    farthest = (distance, Some(between));
                }
            }
        }
        if let (distance, Some(split)) = farthest
            && distance > tolerance
        {
            if let Some(flag) = keep.get_mut(split) {
                *flag = true;
            }
            pending.push((from, split));
            pending.push((split, to));
        }
    }
    path.iter()
        .zip(&keep)
        .filter(|(_, kept)| **kept)
        .map(|(point, _)| *point)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Base, Volume};

    fn chain(count: usize, bend: f32) -> Vec<Stroke> {
        (0..count)
            .map(|index| {
                let at =
                    |step: usize| Vec3::new(step as f32, (step as f32 * bend).sin() * 0.02, 0.0);
                Stroke::capsule(StrokeMode::Carve, at(index), at(index + 1), 1.0, 0.4)
            })
            .collect()
    }

    #[test]
    fn a_straight_drag_becomes_one_capsule_and_keeps_its_shape() {
        let strokes = chain(30, 1.0);
        let merged = consolidate(&strokes, 0.1);
        assert_eq!(merged.len(), 1);
        let volume = |strokes: Vec<Stroke>| Volume {
            base: Base::Solid,
            size: Vec3::new(40.0, 8.0, 8.0),
            cell_size: 0.5,
            strokes,
            stamps: Vec::new(),
        };
        let (before, after) = (volume(strokes), volume(merged));
        for step in 0..30 {
            let point = Vec3::new(step as f32 + 0.5, 0.0, 0.7);
            assert!((before.distance(point) - after.distance(point)).abs() < 0.15);
        }
    }

    #[test]
    fn a_real_corner_is_kept_and_other_modes_are_left_alone() {
        let mut strokes = chain(10, 0.0);
        let corner = strokes.last().expect("chain").end;
        for step in 1..=10 {
            let start = corner + Vec3::new(0.0, 0.0, (step - 1) as f32);
            strokes.push(Stroke::capsule(
                StrokeMode::Carve,
                start,
                start + Vec3::new(0.0, 0.0, 1.0),
                1.0,
                0.4,
            ));
        }
        strokes.push(Stroke {
            strength: 0.5,
            ..Stroke::capsule(StrokeMode::Inflate, Vec3::ZERO, Vec3::ZERO, 1.0, 0.0)
        });
        let merged = consolidate(&strokes, 0.1);
        assert_eq!(merged.len(), 3, "two straight runs and the inflate");
        assert!((merged[0].end - corner).length() < 1.0e-4);
        assert_eq!(
            merged.last().map(|stroke| stroke.mode),
            Some(StrokeMode::Inflate)
        );
    }
}
