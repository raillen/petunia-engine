//! Rivers as gameplay water: current, buoyancy and swimming.
//!
//! A river is a chain of samples along its spline. A point is inside the volume when it is within
//! the half width of a segment (in the XZ plane) and between the bed and the surface.

use petunia_math::{CubicBezierSpline, Vec3};

/// Most samples a river contributes to the water volume.
const MAX_WATER_SAMPLES: usize = 256;

/// One cross-section of a river.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSample {
    pub center: Vec3,
    pub half_width: f32,
    pub depth: f32,
}

/// A river volume with its gameplay forces.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterVolume {
    pub entity: u64,
    pub samples: Vec<WaterSample>,
    pub flow_speed: f32,
    pub float_force: f32,
    pub swim_force: f32,
}

/// What the water does at a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterMoment {
    /// Direction and speed of the current (meters per second).
    pub flow: Vec3,
    pub surface_y: f32,
    pub depth: f32,
    pub float_force: f32,
    pub swim_force: f32,
}

impl WaterVolume {
    /// Build a volume from a river spline. `None` when the spline has no usable segment.
    pub fn from_spline(
        entity: u64,
        spline: &CubicBezierSpline,
        flow_speed: f32,
        float_force: f32,
        swim_force: f32,
    ) -> Option<Self> {
        let frames = spline.sample_with_frames(2.0);
        if frames.len() < 2 {
            return None;
        }
        let stride = (frames.len() / MAX_WATER_SAMPLES).max(1);
        let samples = frames
            .iter()
            .step_by(stride)
            .filter_map(|frame| {
                let (width, depth, _extra) = spline.evaluate_parameters(frame.parameter)?;
                if !width.is_finite() || !depth.is_finite() {
                    return None;
                }
                Some(WaterSample {
                    center: frame.position,
                    half_width: (width * 0.5).max(0.05),
                    depth: depth.max(0.05),
                })
            })
            .collect::<Vec<_>>();
        if samples.len() < 2 {
            return None;
        }
        Some(Self {
            entity,
            samples,
            flow_speed: if flow_speed.is_finite() && flow_speed >= 0.0 {
                flow_speed
            } else {
                0.0
            },
            float_force: if float_force.is_finite() && float_force >= 0.0 {
                float_force
            } else {
                9.8
            },
            swim_force: if swim_force.is_finite() && swim_force > 0.0 {
                swim_force
            } else {
                4.0
            },
        })
    }

    /// The water moment at `position`, or `None` when the point is dry.
    pub fn moment_at(&self, position: Vec3) -> Option<WaterMoment> {
        if !position.is_finite() {
            return None;
        }
        let mut best: Option<(f32, WaterMoment)> = None;
        for pair in self.samples.windows(2) {
            let (Some(start), Some(end)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let segment = Vec3::new(
                end.center.x - start.center.x,
                0.0,
                end.center.z - start.center.z,
            );
            let length_squared = segment.length_squared();
            let blend = if length_squared > 1.0e-9 {
                ((position.x - start.center.x) * segment.x
                    + (position.z - start.center.z) * segment.z)
                    / length_squared
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let closest_x = start.center.x + segment.x * blend;
            let closest_z = start.center.z + segment.z * blend;
            let delta_x = position.x - closest_x;
            let delta_z = position.z - closest_z;
            let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
            let half_width = start.half_width + (end.half_width - start.half_width) * blend;
            let surface_y = start.center.y + (end.center.y - start.center.y) * blend;
            let depth = start.depth + (end.depth - start.depth) * blend;
            if distance > half_width
                || position.y > surface_y + 0.25
                || position.y < surface_y - depth - 0.5
            {
                continue;
            }
            let direction = segment.normalized_or_zero();
            let moment = WaterMoment {
                flow: direction * self.flow_speed,
                surface_y,
                depth,
                float_force: self.float_force,
                swim_force: self.swim_force,
            };
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, moment));
            }
        }
        best.map(|(_, moment)| moment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::{SplinePoint, Vec3};

    fn straight_river() -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::new(0.0, 1.0, 0.0),
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(3.0, 0.0, 0.0),
            width: 8.0,
            depth: 2.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(20.0, 0.0, 0.0),
            handle_in: Vec3::new(-3.0, 0.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 8.0,
            depth: 2.0,
            extra: 0.0,
        });
        spline
    }

    #[test]
    fn a_point_inside_gets_current_and_surface() {
        let volume = WaterVolume::from_spline(7, &straight_river(), 2.0, 9.8, 4.0)
            .unwrap_or_else(|| panic!());
        let moment = volume
            .moment_at(Vec3::new(10.0, 0.5, 1.0))
            .unwrap_or_else(|| panic!("inside"));
        assert!(moment.flow.x > 1.0);
        assert!(moment.flow.z.abs() < 1.0e-4);
        assert!((moment.surface_y - 0.5).abs() < 0.2);
        assert!((moment.depth - 2.0).abs() < 0.1);
    }

    #[test]
    fn dry_points_have_no_moment() {
        let volume = WaterVolume::from_spline(7, &straight_river(), 2.0, 9.8, 4.0)
            .unwrap_or_else(|| panic!());
        assert!(volume.moment_at(Vec3::new(10.0, 0.5, 20.0)).is_none());
        assert!(volume.moment_at(Vec3::new(10.0, 10.0, 0.0)).is_none());
    }
}
