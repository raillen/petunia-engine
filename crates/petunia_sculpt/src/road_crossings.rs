//! Procedural road crossings: intersect splines, split them, build junction geometry.
//!
//! Pure geometric operations. No trait hierarchies. Intersections use a bounded coarse search
//! followed by local refinement, so results are deterministic and cheap.

use petunia_math::{CubicBezierSpline, SplinePoint, Vec3};

/// A detected intersection between two splines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplineIntersection {
    pub position: Vec3,
    /// Parameter on the first spline, in segment units.
    pub parameter_first: f32,
    /// Parameter on the second spline, in segment units.
    pub parameter_second: f32,
    /// Smallest distance found between the two curves, in meters.
    pub distance: f32,
}

/// A crossing junction.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadJunction {
    pub position: Vec3,
    pub connected_splines: Vec<usize>,
    pub corner_points: Vec<Vec3>,
    /// Angle between the two tangents, in degrees.
    pub angle_degrees: f32,
    pub first_parameter: f32,
    pub second_parameter: f32,
}

/// Find the closest approach between two splines by sampling and local refinement.
///
/// Returns `None` when the curves are farther apart than `maximum_distance`.
pub fn find_spline_intersection(
    first: &CubicBezierSpline,
    second: &CubicBezierSpline,
    samples: usize,
    maximum_distance: f32,
) -> Option<SplineIntersection> {
    let segments_first = first.segments();
    let segments_second = second.segments();
    if segments_first == 0 || segments_second == 0 {
        return None;
    }
    let limit = if maximum_distance.is_finite() && maximum_distance > 0.0 {
        maximum_distance
    } else {
        1.0
    };
    let sample_count = samples.clamp(4, 256);
    let step_first = segments_first as f32 / sample_count as f32;
    let step_second = segments_second as f32 / sample_count as f32;
    let mut best_parameter_first = 0.0f32;
    let mut best_parameter_second = 0.0f32;
    let mut best_distance = f32::INFINITY;
    for first_index in 0..=sample_count {
        let parameter_first = (first_index as f32 * step_first).min(segments_first as f32);
        let Some(position_first) = first.evaluate_position(parameter_first) else {
            continue;
        };
        for second_index in 0..=sample_count {
            let parameter_second = (second_index as f32 * step_second).min(segments_second as f32);
            let Some(position_second) = second.evaluate_position(parameter_second) else {
                continue;
            };
            let distance = (position_first - position_second).length();
            if distance < best_distance {
                best_distance = distance;
                best_parameter_first = parameter_first;
                best_parameter_second = parameter_second;
            }
        }
    }
    // Alternate refinement on each curve, shrinking the window each round.
    let mut window_first = step_first;
    let mut window_second = step_second;
    for _ in 0..8 {
        for candidate in [
            best_parameter_first - window_first,
            best_parameter_first + window_first,
        ] {
            if !(0.0..=segments_first as f32).contains(&candidate) {
                continue;
            }
            if let (Some(candidate_position), Some(other_position)) = (
                first.evaluate_position(candidate),
                second.evaluate_position(best_parameter_second),
            ) {
                let distance = (candidate_position - other_position).length();
                if distance < best_distance {
                    best_distance = distance;
                    best_parameter_first = candidate;
                }
            }
        }
        for candidate in [
            best_parameter_second - window_second,
            best_parameter_second + window_second,
        ] {
            if !(0.0..=segments_second as f32).contains(&candidate) {
                continue;
            }
            if let (Some(candidate_position), Some(other_position)) = (
                second.evaluate_position(candidate),
                first.evaluate_position(best_parameter_first),
            ) {
                let distance = (candidate_position - other_position).length();
                if distance < best_distance {
                    best_distance = distance;
                    best_parameter_second = candidate;
                }
            }
        }
        window_first *= 0.5;
        window_second *= 0.5;
    }
    if !best_distance.is_finite() || best_distance > limit {
        return None;
    }
    let position_first = first.evaluate_position(best_parameter_first)?;
    let position_second = second.evaluate_position(best_parameter_second)?;
    Some(SplineIntersection {
        position: (position_first + position_second) * 0.5,
        parameter_first: best_parameter_first,
        parameter_second: best_parameter_second,
        distance: best_distance,
    })
}

/// Split an open spline at a curve parameter, using de Casteljau subdivision.
///
/// The two halves share the split point and keep the per-point parameters. Closed splines are
/// refused because the seam would need a different treatment.
pub fn split_spline_at(
    spline: &CubicBezierSpline,
    parameter: f32,
) -> Option<(CubicBezierSpline, CubicBezierSpline)> {
    let segments = spline.segments();
    if segments == 0 || spline.closed || !parameter.is_finite() {
        return None;
    }
    if !(0.0..=segments as f32).contains(&parameter) {
        return None;
    }
    let clamped = parameter.clamp(0.0, segments as f32);
    // A split exactly at either end would leave one half without a segment.
    if clamped <= 1.0e-4 || clamped >= segments as f32 - 1.0e-4 {
        return None;
    }
    let segment = (clamped.floor() as usize).min(segments - 1);
    let segment_parameter = clamped - segment as f32;
    let start = spline.points.get(segment)?;
    let end = spline.points.get(segment + 1)?;
    let control_start = start.position;
    let control_start_handle = start.position + start.handle_out;
    let control_end_handle = end.position + end.handle_in;
    let control_end = end.position;
    let blend = segment_parameter;
    let first_handle = lerp(control_start, control_start_handle, blend);
    let middle = lerp(control_start_handle, control_end_handle, blend);
    let last_handle = lerp(control_end_handle, control_end, blend);
    let first_inner = lerp(first_handle, middle, blend);
    let second_inner = lerp(middle, last_handle, blend);
    let split_position = lerp(first_inner, second_inner, blend);
    let (width, depth, extra) =
        spline
            .evaluate_parameters(clamped)
            .unwrap_or((start.width, start.depth, start.extra));

    let mut left_points = spline.points.get(..=segment)?.to_vec();
    if let Some(last) = left_points.last_mut() {
        last.handle_out = first_handle - control_start;
    }
    left_points.push(SplinePoint {
        position: split_position,
        handle_in: first_inner - split_position,
        handle_out: Vec3::ZERO,
        width,
        depth,
        extra,
    });

    let mut right_points = Vec::with_capacity(spline.points.len() - segment);
    right_points.push(SplinePoint {
        position: split_position,
        handle_in: Vec3::ZERO,
        handle_out: second_inner - split_position,
        width,
        depth,
        extra,
    });
    let mut tail = spline.points.get(segment + 1..)?.to_vec();
    if let Some(first) = tail.first_mut() {
        first.handle_in = last_handle - control_end;
    }
    right_points.extend(tail);

    Some((
        CubicBezierSpline {
            points: left_points,
            closed: false,
        },
        CubicBezierSpline {
            points: right_points,
            closed: false,
        },
    ))
}

fn lerp(start: Vec3, end: Vec3, blend: f32) -> Vec3 {
    start + (end - start) * blend
}

/// Build a junction polygon around the crossing, ordered by angle.
pub fn build_junction_polygon(center: Vec3, radius: f32, directions: &[Vec3]) -> Vec<Vec3> {
    let radius = if radius.is_finite() && radius > 0.0 {
        radius.min(1000.0)
    } else {
        4.0
    };
    if !center.is_finite() || directions.is_empty() {
        return vec![
            center + Vec3::new(-radius, 0.0, -radius),
            center + Vec3::new(radius, 0.0, -radius),
            center + Vec3::new(radius, 0.0, radius),
            center + Vec3::new(-radius, 0.0, radius),
        ];
    }
    let mut points = Vec::new();
    for direction in directions {
        let horizontal = Vec3::new(direction.x, 0.0, direction.z).normalized_or_zero();
        if horizontal.length_squared() < 1.0e-12 {
            continue;
        }
        let perpendicular = Vec3::new(horizontal.z, 0.0, -horizontal.x);
        points.push(center + perpendicular * radius);
        points.push(center - perpendicular * radius);
    }
    points.sort_by(|first, second| {
        let first_angle = (first.z - center.z).atan2(first.x - center.x);
        let second_angle = (second.z - center.z).atan2(second.x - center.x);
        first_angle.total_cmp(&second_angle)
    });
    points
}

/// Crossings sharper than this are flagged: the junction polygon becomes thin and textures
/// overlap badly in the acute corner.
pub const MINIMUM_CROSSING_ANGLE_DEGREES: f32 = 30.0;

/// True when the acute angle between the roads is below `minimum_degrees`.
pub fn crossing_angle_is_too_sharp(junction: &RoadJunction, minimum_degrees: f32) -> bool {
    let threshold = if minimum_degrees.is_finite() && minimum_degrees >= 0.0 {
        minimum_degrees
    } else {
        MINIMUM_CROSSING_ANGLE_DEGREES
    };
    let acute_angle = junction.angle_degrees.min(180.0 - junction.angle_degrees);
    acute_angle < threshold
}

/// Create a junction between two splines that cross.
pub fn create_road_junction(
    first: &CubicBezierSpline,
    second: &CubicBezierSpline,
) -> Option<RoadJunction> {
    let intersection = find_spline_intersection(first, second, 48, 1.5)?;
    let tangent_first = first
        .evaluate_tangent(intersection.parameter_first)
        .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
    let tangent_second = second
        .evaluate_tangent(intersection.parameter_second)
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    let dot = tangent_first.dot(tangent_second).clamp(-1.0, 1.0);
    let angle_degrees = dot.acos().to_degrees();
    let corners =
        build_junction_polygon(intersection.position, 3.0, &[tangent_first, tangent_second]);
    Some(RoadJunction {
        position: intersection.position,
        connected_splines: vec![0, 1],
        corner_points: corners,
        angle_degrees,
        first_parameter: intersection.parameter_first,
        second_parameter: intersection.parameter_second,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: Vec3, to: Vec3) -> CubicBezierSpline {
        let direction = (to - from) * (1.0 / 3.0);
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: from,
            handle_in: Vec3::ZERO,
            handle_out: direction,
            width: 4.0,
            depth: 0.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: to,
            handle_in: -direction,
            handle_out: Vec3::ZERO,
            width: 4.0,
            depth: 0.0,
            extra: 0.0,
        });
        spline
    }

    #[test]
    fn find_perpendicular_intersection() {
        let first = line(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        let second = line(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0));
        let intersection = find_spline_intersection(&first, &second, 32, 1.0);
        assert!(intersection.is_some());
        if let Some(intersection) = intersection {
            assert!(intersection.position.x.abs() < 0.5);
            assert!(intersection.position.z.abs() < 0.5);
            assert!(intersection.distance < 1.0);
        }
    }

    #[test]
    fn parallel_splines_have_no_intersection() {
        let first = line(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0));
        let second = line(Vec3::new(0.0, 0.0, 5.0), Vec3::new(10.0, 0.0, 5.0));
        assert!(find_spline_intersection(&first, &second, 32, 1.0).is_none());
    }

    #[test]
    fn split_preserves_the_curve_and_the_split_point() {
        let spline = line(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0));
        let expected = spline.evaluate_position(0.5);
        let (left, right) = split_spline_at(&spline, 0.5).unwrap_or_else(|| panic!("split"));
        assert!(left.is_valid());
        assert!(right.is_valid());
        let left_end = left.evaluate_position(left.segments() as f32);
        let right_start = right.evaluate_position(0.0);
        assert_eq!(left_end, right_start);
        assert_eq!(left_end, expected);
        assert!((left.points.last().map_or(0.0, |point| point.width) - 4.0).abs() < 1.0e-4);
        // The left half is the original curve restricted to [0, 0.5]; its midpoint is at
        // original parameter 0.25.
        let middle = left.evaluate_position(0.5);
        let original = spline.evaluate_position(0.25);
        if let (Some(middle), Some(original)) = (middle, original) {
            assert!((middle - original).length() < 1.0e-4);
        }
    }

    #[test]
    fn closed_splines_are_not_split() {
        let mut spline = line(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        spline.closed = true;
        assert!(split_spline_at(&spline, 0.5).is_none());
    }

    #[test]
    fn sharp_crossings_are_flagged() {
        let perpendicular_first = line(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        let perpendicular_second = line(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0));
        if let Some(junction) = create_road_junction(&perpendicular_first, &perpendicular_second) {
            assert!(!crossing_angle_is_too_sharp(&junction, 30.0));
        }
        let diagonal_first = line(Vec3::new(-5.0, 0.0, -1.0), Vec3::new(5.0, 0.0, 1.0));
        let diagonal_second = line(Vec3::new(-5.0, 0.0, 1.0), Vec3::new(5.0, 0.0, -1.0));
        if let Some(junction) = create_road_junction(&diagonal_first, &diagonal_second) {
            assert!(crossing_angle_is_too_sharp(&junction, 30.0));
        }
    }

    #[test]
    fn junction_has_corners_and_angle() {
        let first = line(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
        let second = line(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0));
        let junction = create_road_junction(&first, &second);
        assert!(junction.is_some());
        if let Some(junction) = junction {
            assert!(!junction.corner_points.is_empty());
            assert!((junction.angle_degrees - 90.0).abs() < 5.0);
        }
    }
}
