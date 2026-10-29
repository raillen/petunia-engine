//! Lakes, confluences and waterfalls.
//!
//! Pure data and mesh generation helpers.

use crate::path_network::{PathMesh, PathMeshError, PathVertex};
use petunia_math::{CubicBezierSpline, Vec3};
use petunia_world::{PersistentId, WaterVolume};

/// A lake polygon never has more vertices than this.
pub const MAX_LAKE_POINTS: usize = 4096;

/// Lake polygon in the XZ plane.
#[derive(Clone, Debug)]
pub struct LakePolygon {
    pub points: Vec<Vec3>,
    pub water_level: f32,
}

impl LakePolygon {
    fn is_valid(&self) -> bool {
        self.water_level.is_finite()
            && self.points.len() >= 3
            && self.points.len() <= MAX_LAKE_POINTS
            && self.points.iter().all(|point| point.is_finite())
    }
}

/// Generate a flat water mesh for a lake polygon (triangle fan from the center).
pub fn generate_lake_mesh(lake: &LakePolygon) -> Result<PathMesh, PathMeshError> {
    if !lake.is_valid() {
        return Err(PathMeshError::InvalidPolygon);
    }
    let mut vertices = Vec::with_capacity(lake.points.len() + 1);
    let mut indices = Vec::with_capacity(lake.points.len() * 3);
    let center = lake
        .points
        .iter()
        .fold(Vec3::ZERO, |sum, point| sum + *point)
        * (1.0 / lake.points.len() as f32);
    vertices.push(PathVertex {
        position: Vec3::new(center.x, lake.water_level, center.z),
        normal: Vec3::UP,
        uv: [0.5, 0.5],
        flow: Vec3::ZERO,
        margin_mask: 0.0,
    });
    for point in &lake.points {
        vertices.push(PathVertex {
            position: Vec3::new(point.x, lake.water_level, point.z),
            normal: Vec3::UP,
            uv: [
                0.5 + (point.x - center.x) * 0.5,
                0.5 + (point.z - center.z) * 0.5,
            ],
            flow: Vec3::ZERO,
            margin_mask: 1.0,
        });
    }
    for index in 1..=lake.points.len() {
        let next = if index == lake.points.len() {
            1
        } else {
            index + 1
        };
        indices.extend_from_slice(&[0, index as u32, next as u32]);
    }
    Ok(PathMesh { vertices, indices })
}

/// Generate a vertical waterfall ribbon along a steep drop.
pub fn generate_waterfall_mesh(
    spline: &CubicBezierSpline,
    drop_height: f32,
) -> Result<PathMesh, PathMeshError> {
    let frames = spline.sample_with_frames(0.5);
    if frames.is_empty() {
        return Err(PathMeshError::EmptySpline);
    }
    let drop = if drop_height.is_finite() && drop_height > 0.0 {
        drop_height
    } else {
        3.0
    };
    let frame_count = frames.len();
    let mut vertices = Vec::with_capacity(frame_count * 2);
    let mut indices = Vec::with_capacity((frame_count - 1) * 6);
    for (index, frame) in frames.iter().enumerate() {
        let progress = index as f32 / (frame_count - 1).max(1) as f32;
        let (width, _depth, _extra) = spline
            .evaluate_parameters(frame.parameter)
            .unwrap_or((4.0, 1.0, 0.0));
        let half_width = if width.is_finite() && width > 0.0 {
            width * 0.5
        } else {
            2.0
        };
        let vertical_offset = Vec3::new(0.0, -drop * progress, 0.0);
        let flow = Vec3::new(0.0, -1.0, 0.0) * (1.5 + progress * 2.5);
        for (side, sign) in [(-1.0f32, -1.0f32), (1.0, 1.0)] {
            vertices.push(PathVertex {
                position: frame.position + frame.normal * (half_width * sign) + vertical_offset,
                normal: frame.normal,
                uv: [progress, (side + 1.0) * 0.5],
                flow,
                margin_mask: 0.8,
            });
        }
    }
    for index in 0..(frame_count - 1) {
        let base = (index * 2) as u32;
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }
    Ok(PathMesh { vertices, indices })
}

/// Build the gameplay water volume that follows a river spline.
pub fn water_volume_for_spline(
    object_id: PersistentId,
    flow_speed: f32,
    float_force: f32,
    swim_force: f32,
) -> WaterVolume {
    let speed = if flow_speed.is_finite() && flow_speed >= 0.0 {
        flow_speed
    } else {
        1.0
    };
    WaterVolume {
        object_id,
        flow_speed: speed,
        float_force: if float_force.is_finite() && float_force > 0.0 {
            float_force
        } else {
            9.8
        },
        swim_force: if swim_force.is_finite() && swim_force > 0.0 {
            swim_force
        } else {
            4.0
        },
        has_current: speed > 0.0,
    }
}

/// Find confluence points where river spline endpoints meet, within `threshold` meters.
pub fn find_confluences(splines: &[CubicBezierSpline], threshold: f32) -> Vec<Vec3> {
    let threshold = if threshold.is_finite() && threshold > 0.0 {
        threshold
    } else {
        3.0
    };
    let mut confluences = Vec::new();
    for (first_index, first) in splines.iter().enumerate() {
        let Some(first_start) = first.evaluate_position(0.0) else {
            continue;
        };
        let first_end = if first.closed {
            first_start
        } else {
            first
                .evaluate_position(first.segments() as f32)
                .unwrap_or(first_start)
        };
        for second in splines.iter().skip(first_index + 1) {
            let Some(second_start) = second.evaluate_position(0.0) else {
                continue;
            };
            let second_end = if second.closed {
                second_start
            } else {
                second
                    .evaluate_position(second.segments() as f32)
                    .unwrap_or(second_start)
            };
            let endpoints = [
                (first_start, second_start),
                (first_start, second_end),
                (first_end, second_start),
                (first_end, second_end),
            ];
            let mut closest: Option<(Vec3, f32)> = None;
            for (first_point, second_point) in endpoints {
                let distance = (first_point - second_point).length();
                if distance <= threshold
                    && closest.is_none_or(|(_, best_distance)| distance < best_distance)
                {
                    closest = Some(((first_point + second_point) * 0.5, distance));
                }
            }
            if let Some((position, _distance)) = closest {
                confluences.push(position);
            }
        }
    }
    confluences
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::SplinePoint;

    #[test]
    fn lake_mesh_generated() {
        let lake = LakePolygon {
            points: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            water_level: 1.0,
        };
        let mesh = generate_lake_mesh(&lake).unwrap_or_default();
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| (vertex.position.y - 1.0).abs() < 1.0e-6)
        );
    }

    #[test]
    fn invalid_lake_is_refused() {
        let lake = LakePolygon {
            points: vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)],
            water_level: f32::NAN,
        };
        assert_eq!(
            generate_lake_mesh(&lake),
            Err(PathMeshError::InvalidPolygon)
        );
    }

    #[test]
    fn waterfall_ribbon_uses_the_spline_width() {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::ZERO,
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(0.0, -2.0, 0.0),
            width: 6.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(0.0, -10.0, 0.0),
            handle_in: Vec3::new(0.0, 2.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 6.0,
            depth: 1.0,
            extra: 0.0,
        });
        let mesh = generate_waterfall_mesh(&spline, 10.0).unwrap_or_default();
        assert!(!mesh.vertices.is_empty());
        assert!(mesh.vertices.iter().any(|vertex| vertex.flow.y < 0.0));
        let left = mesh.vertices.first().map(|vertex| vertex.position);
        if let Some(left) = left {
            assert!(left.y <= 0.0);
        }
    }

    #[test]
    fn water_volume_refuses_hostile_speeds() {
        let volume = water_volume_for_spline(PersistentId(7), f32::NAN, -1.0, 0.0);
        assert_eq!(volume.object_id, PersistentId(7));
        assert_eq!(volume.flow_speed, 1.0);
        assert!(volume.has_current);
    }

    #[test]
    fn confluences_are_found_between_endpoints() {
        let mut first = CubicBezierSpline::new();
        first.points.push(SplinePoint {
            position: Vec3::new(0.0, 0.0, 0.0),
            ..SplinePoint::default()
        });
        first.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 0.0),
            ..SplinePoint::default()
        });
        let mut second = CubicBezierSpline::new();
        second.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 0.0),
            ..SplinePoint::default()
        });
        second.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 10.0),
            ..SplinePoint::default()
        });
        let confluences = find_confluences(&[first, second], 1.0);
        assert_eq!(confluences.len(), 1);
        if let Some(position) = confluences.first() {
            assert!((position.x - 10.0).abs() < 1.0e-4);
        }
    }
}
