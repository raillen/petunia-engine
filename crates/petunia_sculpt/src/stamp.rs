//! Mesh stamps: a triangle surface imprinted on the field, applied after the strokes.
//!
//! A stamp is not a solid. It adds or removes rock on the samples within a short band around the
//! mesh, which is what makes a decorated rock face, a carved relief or a piece of a scanned model
//! cheap: the cost is the area of the mesh, not the volume of the cave. Bending a closed mesh into
//! a solid is the job of `petunia_mesh::boolean`, not of a brush.
//!
//! The side of the surface is the side its winding faces: for `add`, rock grows behind the face;
//! for `carve`, the rock behind the face becomes air. A mesh with inconsistent winding gives a
//! patchwork, so the editor refuses faces it cannot orient.

use petunia_math::Vec3;

use crate::{Stamp, StampOperation, component_max, component_min, smooth_max, smooth_min};

/// How far from the surface a stamp reads the field: its blend plus two cells, so the blend is
/// never cut by the edge of the band.
pub(crate) fn reach(stamp: &Stamp, cell: f32) -> f32 {
    stamp.smooth + 2.0 * cell
}

/// The box that holds every triangle of the stamp.
pub(crate) fn stamp_box(stamp: &Stamp) -> Option<(Vec3, Vec3)> {
    let mut low = Vec3::splat(f32::MAX);
    let mut high = Vec3::splat(f32::MIN);
    for triangle in &stamp.triangles {
        for point in triangle {
            low = component_min(low, *point);
            high = component_max(high, *point);
        }
    }
    (low.x <= high.x).then_some((low, high))
}

/// The box of one triangle, or `None` for a degenerate one.
pub(crate) fn triangle_box(triangle: &[Vec3; 3]) -> Option<(Vec3, Vec3)> {
    let [a, b, c] = *triangle;
    let normal = (b - a).cross(c - a);
    if normal.length_squared() <= 1.0e-20 {
        return None;
    }
    let low = component_min(component_min(a, b), c);
    let high = component_max(component_max(a, b), c);
    Some((low, high))
}

/// How a stamp changes one sample. `distance` is the signed distance to the triangle: positive on
/// the side the winding faces.
pub(crate) fn combine(value: f32, distance: f32, stamp: &Stamp) -> f32 {
    match stamp.operation {
        StampOperation::Add => smooth_min(value, distance, stamp.smooth),
        StampOperation::Carve => smooth_max(value, -distance, stamp.smooth),
    }
}

/// The field value at `point` after the whole stamp, by brute force over its triangles.
/// Used by [`crate::Volume::distance`], the slow reference for tests and queries.
pub(crate) fn stamped_value(value: f32, point: Vec3, stamp: &Stamp, cell: f32) -> f32 {
    let reach = reach(stamp, cell);
    let mut result = value;
    for triangle in &stamp.triangles {
        let distance = triangle_distance(point, triangle);
        if !distance.is_finite() || distance.abs() > reach {
            continue;
        }
        result = combine(result, distance, stamp);
    }
    result
}

/// Signed distance from `point` to a triangle. `f32::MAX` for a degenerate triangle, which no band
/// ever includes.
pub(crate) fn triangle_distance(point: Vec3, triangle: &[Vec3; 3]) -> f32 {
    let [first, second, third] = *triangle;
    let normal = (second - first).cross(third - first);
    if normal.length_squared() <= 1.0e-20 {
        return f32::MAX;
    }
    let normal = normal.normalized_or_zero();
    let closest = closest_point(point, first, second, third);
    let offset = point - closest;
    let distance = offset.length();
    if offset.dot(normal) < 0.0 {
        -distance
    } else {
        distance
    }
}

/// The closest point of the triangle, after Ericson's *Real-Time Collision Detection* (§5.1.5).
/// Every division is guarded: a zero here would produce a NaN sample, and a NaN sample would break
/// the mesher.
fn closest_point(point: Vec3, first: Vec3, second: Vec3, third: Vec3) -> Vec3 {
    let edge_ab = second - first;
    let edge_ac = third - first;
    let from_first = point - first;
    let along_ab = edge_ab.dot(from_first);
    let along_ac = edge_ac.dot(from_first);
    if along_ab <= 0.0 && along_ac <= 0.0 {
        return first;
    }
    let from_second = point - second;
    let second_ab = edge_ab.dot(from_second);
    let second_ac = edge_ac.dot(from_second);
    if second_ab >= 0.0 && second_ac <= second_ab {
        return second;
    }
    let region_ac = along_ab * second_ac - second_ab * along_ac;
    if region_ac <= 0.0 && along_ab >= 0.0 && second_ab <= 0.0 {
        return along_edge(first, edge_ab, along_ab, second_ab);
    }
    let from_third = point - third;
    let third_ab = edge_ab.dot(from_third);
    let third_ac = edge_ac.dot(from_third);
    if third_ac >= 0.0 && third_ab <= third_ac {
        return third;
    }
    let region_ab = third_ab * along_ac - along_ab * third_ac;
    if region_ab <= 0.0 && along_ac >= 0.0 && third_ac <= 0.0 {
        return along_edge(first, edge_ac, along_ac, third_ac);
    }
    let region_bc = second_ab * third_ac - third_ab * second_ac;
    if region_bc <= 0.0 && second_ac - second_ab >= 0.0 && third_ab - third_ac >= 0.0 {
        return along_edge(
            second,
            third - second,
            second_ac - second_ab,
            third_ab - third_ac,
        );
    }
    let total = region_ab + region_bc + region_ac;
    if total.abs() <= 1.0e-20 {
        return first;
    }
    let weight_second = region_ab / total;
    let weight_third = region_ac / total;
    first + edge_ab * weight_second + edge_ac * weight_third
}

/// A point on the edge from `from` towards `to`, clamped to the segment.
fn along_edge(from: Vec3, to: Vec3, from_weight: f32, to_weight: f32) -> Vec3 {
    let denominator = from_weight - to_weight;
    if denominator.abs() <= 1.0e-20 {
        return from;
    }
    let parameter = (from_weight / denominator).clamp(0.0, 1.0);
    from + to * parameter
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_triangle() -> [Vec3; 3] {
        [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ]
    }

    #[test]
    fn the_distance_is_positive_on_the_facing_side() {
        let triangle = unit_triangle();
        // The triangle faces +z (counter-clockwise seen from there).
        assert!((triangle_distance(Vec3::new(0.5, 0.5, 1.0), &triangle) - 1.0).abs() < 1.0e-5);
        assert!((triangle_distance(Vec3::new(0.5, 0.5, -2.0), &triangle) + 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn the_distance_is_measured_to_the_nearest_point_not_the_plane() {
        let triangle = unit_triangle();
        // Off the corner: the nearest point is the corner itself.
        let distance = triangle_distance(Vec3::new(-3.0, -4.0, 0.0), &triangle);
        assert!((distance.abs() - 5.0).abs() < 1.0e-4, "{distance}");
    }

    #[test]
    fn degenerate_triangles_are_skipped() {
        let flat = [
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        ];
        assert_eq!(triangle_distance(Vec3::ZERO, &flat), f32::MAX);
        assert!(triangle_box(&flat).is_none());
    }
}
