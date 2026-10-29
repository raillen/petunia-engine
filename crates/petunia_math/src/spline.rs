//! Cubic Bezier splines with per-point width/depth and frames, shared by rivers and roads.
//!
//! All values are plain data; no trait hierarchies. Deterministic and free of panics.

use serde::{Deserialize, Serialize};

use crate::Vec3;

/// A curve is never sampled into more frames than this.
pub const MAX_CURVE_SAMPLES: usize = 4096;

/// A curve never has more control points than this (external data guard).
pub const MAX_SPLINE_POINTS: usize = 4096;

/// How many straight pieces approximate one Bezier segment when measuring length.
const LENGTH_SAMPLES_PER_SEGMENT: usize = 24;

/// A frame at a point on a curve (orthonormal basis for sweeping a profile).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveFrame {
    pub position: Vec3,
    pub tangent: Vec3,
    pub normal: Vec3,
    pub binormal: Vec3,
    /// Global curve parameter in segment units (`0..=segment_count`) at this frame.
    pub parameter: f32,
}

/// Per-point parameters along a spline.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplinePoint {
    pub position: Vec3,
    pub handle_in: Vec3,
    pub handle_out: Vec3,
    /// Width in meters (used by rivers/roads).
    pub width: f32,
    /// Depth/channel depth in meters (used by rivers).
    pub depth: f32,
    /// Optional extra parameter for the road or river profile.
    pub extra: f32,
}

impl Default for SplinePoint {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            handle_in: Vec3::ZERO,
            handle_out: Vec3::ZERO,
            width: 4.0,
            depth: 1.0,
            extra: 0.0,
        }
    }
}

impl SplinePoint {
    fn is_finite(&self) -> bool {
        self.position.is_finite()
            && self.handle_in.is_finite()
            && self.handle_out.is_finite()
            && self.width.is_finite()
            && self.depth.is_finite()
            && self.extra.is_finite()
    }
}

/// A cubic Bezier spline built from control points with handles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CubicBezierSpline {
    pub points: Vec<SplinePoint>,
    /// If true, the spline is closed (last connects to first).
    pub closed: bool,
}

impl CubicBezierSpline {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            closed: false,
        }
    }

    /// The curve can be evaluated: two or more finite points, within the point budget.
    pub fn is_valid(&self) -> bool {
        self.points.len() >= 2
            && self.points.len() <= MAX_SPLINE_POINTS
            && self.points.iter().all(SplinePoint::is_finite)
    }

    /// Number of Bezier segments, or zero when the curve cannot be evaluated.
    pub fn segments(&self) -> usize {
        if !self.is_valid() {
            return 0;
        }
        if self.closed {
            self.points.len()
        } else {
            self.points.len() - 1
        }
    }

    /// Evaluate position at global parameter `parameter` in segment units.
    pub fn evaluate_position(&self, parameter: f32) -> Option<Vec3> {
        let segments = self.segments();
        if segments == 0 || !parameter.is_finite() {
            return None;
        }
        let local_parameter = self.wrap_parameter(parameter, segments)?;
        if !self.closed && (local_parameter <= 0.0) {
            return self.points.first().map(|point| point.position);
        }
        if !self.closed && (local_parameter >= segments as f32) {
            return self.points.get(segments).map(|point| point.position);
        }
        let segment = local_parameter.floor() as usize;
        let segment_parameter = local_parameter - segment as f32;
        let (p0, p1, p2, p3) = self.segment_control_points(segment)?;
        Some(cubic_bezier(p0, p1, p2, p3, segment_parameter))
    }

    /// Evaluate the unit tangent at global parameter `parameter`.
    pub fn evaluate_tangent(&self, parameter: f32) -> Option<Vec3> {
        let segments = self.segments();
        if segments == 0 || !parameter.is_finite() {
            return None;
        }
        let local_parameter = self.wrap_parameter(parameter, segments)?;
        let clamped = local_parameter.clamp(0.0, segments as f32);
        let segment = (clamped.floor() as usize).min(segments - 1);
        let segment_parameter = (clamped - segment as f32).clamp(0.0, 1.0);
        let (p0, p1, p2, p3) = self.segment_control_points(segment)?;
        Some(cubic_bezier_derivative(p0, p1, p2, p3, segment_parameter).normalized_or_zero())
    }

    /// Linearly interpolate width/depth/extra along the curve.
    pub fn evaluate_parameters(&self, parameter: f32) -> Option<(f32, f32, f32)> {
        let segments = self.segments();
        if segments == 0 || !parameter.is_finite() {
            return None;
        }
        let local_parameter = self.wrap_parameter(parameter, segments)?;
        if !self.closed && (local_parameter <= 0.0) {
            let point = self.points.first()?;
            return Some((point.width, point.depth, point.extra));
        }
        if !self.closed && (local_parameter >= segments as f32) {
            let point = self.points.get(segments)?;
            return Some((point.width, point.depth, point.extra));
        }
        let segment = local_parameter.floor() as usize;
        let segment_parameter = local_parameter - local_parameter.floor();
        let first = self.points.get(segment)?;
        let second = self.points.get((segment + 1) % self.points.len())?;
        Some((
            first.width + (second.width - first.width) * segment_parameter,
            first.depth + (second.depth - first.depth) * segment_parameter,
            first.extra + (second.extra - first.extra) * segment_parameter,
        ))
    }

    /// Sample the curve at approximately `spacing` meters, by arc length.
    ///
    /// Frames carry the curve parameter, so callers can read per-point width/depth without
    /// guessing. A closed curve is sampled without duplicating the seam.
    pub fn sample_with_frames(&self, spacing: f32) -> Vec<CurveFrame> {
        let segments = self.segments();
        if segments == 0 {
            return Vec::new();
        }
        let step = if spacing.is_finite() && spacing > 0.0 {
            spacing
        } else {
            0.5
        };
        let table = self.arc_length_table();
        let Some(last_entry) = table.last() else {
            return Vec::new();
        };
        let total_length = last_entry.1;
        if total_length <= 1.0e-6 {
            let mut frames = Vec::with_capacity(1);
            if let Some(frame) = self.frame_at(0.0) {
                frames.push(frame);
            }
            return frames;
        }
        let sample_count = ((total_length / step).ceil() as usize).clamp(2, MAX_CURVE_SAMPLES);
        let denominator = if self.closed {
            sample_count as f32
        } else {
            (sample_count - 1) as f32
        };
        let mut frames = Vec::with_capacity(sample_count);
        for index in 0..sample_count {
            let distance = total_length * index as f32 / denominator;
            let parameter = parameter_at_distance(&table, distance);
            if let Some(frame) = self.frame_at(parameter) {
                frames.push(frame);
            }
        }
        frames
    }

    fn wrap_parameter(&self, parameter: f32, segments: usize) -> Option<f32> {
        if self.closed {
            let length = segments as f32;
            Some(((parameter % length) + length) % length)
        } else if (0.0..=segments as f32).contains(&parameter) {
            Some(parameter)
        } else {
            None
        }
    }

    fn segment_control_points(&self, segment: usize) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        let count = self.points.len();
        let first = self.points.get(segment % count)?;
        let second = self.points.get((segment + 1) % count)?;
        Some((
            first.position,
            first.position + first.handle_out,
            second.position + second.handle_in,
            second.position,
        ))
    }

    /// (parameter, cumulative arc length) pairs across the whole curve.
    fn arc_length_table(&self) -> Vec<(f32, f32)> {
        let segments = self.segments();
        let mut table = Vec::with_capacity(segments * LENGTH_SAMPLES_PER_SEGMENT + 1);
        let mut previous = self.evaluate_position(0.0).unwrap_or(Vec3::ZERO);
        let mut cumulative = 0.0f32;
        table.push((0.0, 0.0));
        for segment in 0..segments {
            for step in 1..=LENGTH_SAMPLES_PER_SEGMENT {
                let parameter = segment as f32 + step as f32 / LENGTH_SAMPLES_PER_SEGMENT as f32;
                let Some(current) = self.evaluate_position(parameter) else {
                    continue;
                };
                cumulative += (current - previous).length();
                previous = current;
                table.push((parameter, cumulative));
            }
        }
        table
    }

    fn frame_at(&self, parameter: f32) -> Option<CurveFrame> {
        let position = self.evaluate_position(parameter)?;
        let mut tangent = self.evaluate_tangent(parameter).unwrap_or(Vec3::ZERO);
        if tangent.length_squared() < 1.0e-12 {
            tangent = Vec3::new(1.0, 0.0, 0.0);
        }
        // A stable reference up: switch to +Z when the tangent is nearly vertical.
        let reference_up = if tangent.dot(Vec3::UP).abs() > 0.99 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::UP
        };
        let mut normal = reference_up.cross(tangent).normalized_or_zero();
        if normal.length_squared() < 1.0e-12 {
            normal = Vec3::new(1.0, 0.0, 0.0).cross(tangent).normalized_or_zero();
        }
        let binormal = tangent.cross(normal).normalized_or_zero();
        Some(CurveFrame {
            position,
            tangent,
            normal,
            binormal,
            parameter,
        })
    }
}

fn parameter_at_distance(table: &[(f32, f32)], distance: f32) -> f32 {
    if table.len() < 2 {
        return 0.0;
    }
    let upper = table.partition_point(|entry| entry.1 < distance);
    let upper = upper.clamp(1, table.len() - 1);
    let (Some(&(parameter_before, distance_before)), Some(&(parameter_after, distance_after))) =
        (table.get(upper - 1), table.get(upper))
    else {
        return 0.0;
    };
    let span = distance_after - distance_before;
    let blend = if span > 1.0e-9 {
        ((distance - distance_before) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    parameter_before + (parameter_after - parameter_before) * blend
}

#[inline]
fn cubic_bezier(
    start: Vec3,
    start_handle: Vec3,
    end_handle: Vec3,
    end: Vec3,
    segment_parameter: f32,
) -> Vec3 {
    let parameter_squared = segment_parameter * segment_parameter;
    let parameter_cubed = parameter_squared * segment_parameter;
    let inverse = 1.0 - segment_parameter;
    let inverse_squared = inverse * inverse;
    let inverse_cubed = inverse_squared * inverse;
    start * inverse_cubed
        + start_handle * (3.0 * inverse_squared * segment_parameter)
        + end_handle * (3.0 * inverse * parameter_squared)
        + end * parameter_cubed
}

#[inline]
fn cubic_bezier_derivative(
    start: Vec3,
    start_handle: Vec3,
    end_handle: Vec3,
    end: Vec3,
    segment_parameter: f32,
) -> Vec3 {
    let parameter_squared = segment_parameter * segment_parameter;
    let inverse = 1.0 - segment_parameter;
    let inverse_squared = inverse * inverse;
    start * (-3.0 * inverse_squared)
        + start_handle * (3.0 * inverse_squared - 6.0 * inverse * segment_parameter)
        + end_handle * (6.0 * inverse * segment_parameter - 3.0 * parameter_squared)
        + end * (3.0 * parameter_squared)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight_spline() -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::ZERO,
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(1.0, 0.0, 0.0),
            width: 2.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(3.0, 1.0, 0.0),
            handle_in: Vec3::new(-1.0, 0.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 4.0,
            depth: 2.0,
            extra: 1.0,
        });
        spline
    }

    #[test]
    fn cubic_bezier_evaluates_endpoints() {
        let spline = straight_spline();
        assert_eq!(spline.evaluate_position(0.0), Some(Vec3::ZERO));
        assert_eq!(
            spline.evaluate_position(1.0),
            Some(Vec3::new(3.0, 1.0, 0.0))
        );
        let (width, depth, extra) = spline.evaluate_parameters(0.5).unwrap_or_default();
        assert!((width - 3.0).abs() < 1.0e-6);
        assert!((depth - 1.5).abs() < 1.0e-6);
        assert!((extra - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn frames_are_orthonormal() {
        let mut spline = CubicBezierSpline::new();
        for index in 0..3 {
            spline.points.push(SplinePoint {
                position: Vec3::new(index as f32 * 2.0, 0.0, 0.0),
                handle_in: Vec3::new(-0.5, 0.0, 0.0),
                handle_out: Vec3::new(0.5, 0.0, 0.0),
                width: 2.0,
                depth: 1.0,
                extra: 0.0,
            });
        }
        let frames = spline.sample_with_frames(0.5);
        assert!(frames.len() >= 2);
        for frame in frames {
            assert!(frame.tangent.is_finite());
            assert!(frame.normal.is_finite());
            assert!(frame.binormal.is_finite());
            assert!((frame.tangent.length() - 1.0).abs() < 1.0e-4);
            assert!(frame.tangent.dot(frame.normal).abs() < 1.0e-4);
            assert!(frame.tangent.dot(frame.binormal).abs() < 1.0e-4);
            assert!(frame.binormal.dot(frame.normal).abs() < 1.0e-4);
        }
    }

    #[test]
    fn sampling_by_arc_length_keeps_spacing_on_uneven_handles() {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::ZERO,
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(0.2, 0.0, 0.0),
            width: 1.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 0.0),
            handle_in: Vec3::new(-8.0, 0.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 1.0,
            depth: 1.0,
            extra: 0.0,
        });
        let frames = spline.sample_with_frames(1.0);
        assert!(frames.len() > 8);
        for pair in frames.windows(2) {
            let distance = (pair[1].position - pair[0].position).length();
            assert!(
                (distance - 1.0).abs() < 0.2,
                "uneven spacing: {distance} at parameter {}",
                pair[0].parameter
            );
        }
    }

    #[test]
    fn closed_spline_does_not_duplicate_the_seam() {
        let mut spline = CubicBezierSpline::new();
        for index in 0..4 {
            let angle = index as f32 * std::f32::consts::FRAC_PI_2;
            spline.points.push(SplinePoint {
                position: Vec3::new(angle.cos() * 4.0, 0.0, angle.sin() * 4.0),
                handle_in: Vec3::new(0.0, 0.0, 0.0),
                handle_out: Vec3::new(0.0, 0.0, 0.0),
                width: 1.0,
                depth: 1.0,
                extra: 0.0,
            });
        }
        spline.closed = true;
        let frames = spline.sample_with_frames(1.0);
        assert!(frames.len() > 10);
        let first = frames.first().map(|frame| frame.position);
        let last = frames.last().map(|frame| frame.position);
        if let (Some(first), Some(last)) = (first, last) {
            assert!((first - last).length() > 0.1, "seam was duplicated");
        }
    }

    #[test]
    fn invalid_and_out_of_range_parameters_are_refused() {
        let spline = straight_spline();
        assert!(spline.evaluate_position(-0.1).is_none());
        assert!(spline.evaluate_position(1.1).is_none());
        assert!(CubicBezierSpline::new().evaluate_position(0.0).is_none());
        let mut broken = spline.clone();
        broken.points[0].width = f32::NAN;
        assert!(!broken.is_valid());
        assert!(broken.evaluate_position(0.5).is_none());
    }
}
