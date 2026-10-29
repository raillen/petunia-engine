//! Generate meshes and terrain modifications along river/road paths.
//!
//! Pure functions operating on splines and heightfields. No trait hierarchies. Cross-sections
//! are normalized: `offset_x` is a multiple of the half width and `offset_y` a multiple of the
//! depth, so the same profile follows the per-point width/depth of the spline.

use petunia_math::{CubicBezierSpline, Vec3};
use std::fmt;

/// A swept profile never has more points than this.
pub const MAX_PROFILE_POINTS: usize = 64;

/// A swept path mesh never has more vertices than this.
pub const MAX_PATH_VERTICES: usize = 65_536;

/// Why a path mesh could not be produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMeshError {
    /// The spline has fewer than two valid points.
    EmptySpline,
    /// The profile has fewer than two points.
    EmptyProfile,
    /// The profile has more than [`MAX_PROFILE_POINTS`] points.
    ProfileTooLarge,
    /// The polygon is not finite or has fewer than three points.
    InvalidPolygon,
    /// Frames times profile points would exceed [`MAX_PATH_VERTICES`].
    VertexBudgetExceeded,
}

impl fmt::Display for PathMeshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySpline => formatter.write_str("the spline has no usable segment"),
            Self::EmptyProfile => formatter.write_str("the profile needs at least two points"),
            Self::ProfileTooLarge => formatter.write_str("the profile has too many points"),
            Self::InvalidPolygon => {
                formatter.write_str("the polygon is not finite or is too small")
            }
            Self::VertexBudgetExceeded => {
                formatter.write_str("the path mesh vertex budget is exceeded")
            }
        }
    }
}

impl std::error::Error for PathMeshError {}

/// Mesh vertex produced by sweeping a profile along a curve.
#[derive(Clone, Debug, PartialEq)]
pub struct PathVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    /// Water flow vector: the tangent scaled by the local edge speed (zero on dry ground).
    pub flow: Vec3,
    /// Margin/foam mask for water edges, `0` at the center and `1` at the edge.
    pub margin_mask: f32,
}

/// Mesh produced by sweeping.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathMesh {
    pub vertices: Vec<PathVertex>,
    pub indices: Vec<u32>,
}

impl PathMesh {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

/// A point of a cross-section, in multiples of the spline half width/depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePoint {
    pub offset_x: f32,
    pub offset_y: f32,
}

impl ProfilePoint {
    pub const fn new(offset_x: f32, offset_y: f32) -> Self {
        Self { offset_x, offset_y }
    }
}

/// Sweep a profile along a spline, generating a mesh with flow vectors on vertices.
pub fn sweep_profile(
    spline: &CubicBezierSpline,
    profile: &[ProfilePoint],
    spacing: f32,
) -> Result<PathMesh, PathMeshError> {
    let frames = spline.sample_with_frames(spacing);
    if frames.is_empty() {
        return Err(PathMeshError::EmptySpline);
    }
    if profile.len() < 2 {
        return Err(PathMeshError::EmptyProfile);
    }
    if profile.len() > MAX_PROFILE_POINTS {
        return Err(PathMeshError::ProfileTooLarge);
    }
    let vertex_count = frames.len().saturating_mul(profile.len());
    if vertex_count > MAX_PATH_VERTICES {
        return Err(PathMeshError::VertexBudgetExceeded);
    }
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(frames.len().saturating_sub(1) * profile.len() * 6);
    let frame_count = frames.len();
    let profile_count = profile.len();
    for frame in &frames {
        let (width, depth, _extra) = spline
            .evaluate_parameters(frame.parameter)
            .unwrap_or((2.0, 1.0, 0.0));
        let half_width = if width.is_finite() && width > 0.0 {
            width * 0.5
        } else {
            1.0
        };
        let channel_depth = if depth.is_finite() && depth > 0.0 {
            depth
        } else {
            1.0
        };
        for (profile_index, point) in profile.iter().enumerate() {
            let position = frame.position
                + frame.normal * (point.offset_x * half_width)
                + frame.binormal * (point.offset_y * channel_depth);
            let across = point.offset_x.clamp(-1.0, 1.0);
            let edge_factor = (1.0 - across.abs()).max(0.0).powf(0.5);
            let flow = frame.tangent * edge_factor.max(0.25);
            let across_fraction = profile_index as f32 / (profile_count - 1) as f32;
            vertices.push(PathVertex {
                position,
                normal: frame.binormal,
                uv: [frame.parameter, across_fraction],
                flow,
                margin_mask: across.abs(),
            });
        }
    }
    append_quad_strip_indices(&mut indices, profile_count, 0, frame_count - 1);
    if spline.closed && frame_count > 1 {
        append_seam_indices(&mut indices, frame_count, profile_count);
    }
    Ok(PathMesh { vertices, indices })
}

fn append_quad_strip_indices(
    indices: &mut Vec<u32>,
    profile_count: usize,
    first_frame: usize,
    last_frame: usize,
) {
    for frame_index in first_frame..last_frame {
        for profile_index in 0..(profile_count - 1) {
            let base = (frame_index * profile_count + profile_index) as u32;
            let next_frame = ((frame_index + 1) * profile_count + profile_index) as u32;
            indices.extend_from_slice(&[
                base,
                next_frame,
                base + 1,
                next_frame,
                next_frame + 1,
                base + 1,
            ]);
        }
    }
}

fn append_seam_indices(indices: &mut Vec<u32>, frame_count: usize, profile_count: usize) {
    let last_frame = frame_count - 1;
    for profile_index in 0..(profile_count - 1) {
        let base = (last_frame * profile_count + profile_index) as u32;
        let seam = profile_index as u32;
        indices.extend_from_slice(&[base, seam, base + 1, seam, seam + 1, base + 1]);
    }
}

/// A flat water-ribbon profile, normalized to the spline width. The channel bed is carved into
/// the terrain separately; the water surface itself stays level across the width.
pub fn river_profile() -> Vec<ProfilePoint> {
    vec![
        ProfilePoint::new(-1.0, 0.0),
        ProfilePoint::new(-0.5, 0.0),
        ProfilePoint::new(0.0, 0.0),
        ProfilePoint::new(0.5, 0.0),
        ProfilePoint::new(1.0, 0.0),
    ]
}

/// A river bed profile: banks at the spline height and the bed one depth below, used to build the
/// channel mesh when the terrain is not carved.
pub fn river_bed_profile() -> Vec<ProfilePoint> {
    vec![
        ProfilePoint::new(-1.0, 0.0),
        ProfilePoint::new(-0.7, -0.3),
        ProfilePoint::new(0.0, -1.0),
        ProfilePoint::new(0.7, -0.3),
        ProfilePoint::new(1.0, 0.0),
    ]
}

/// A road profile: flat top and `shoulder_fraction` of half width on each side.
pub fn road_profile(shoulder_fraction: f32) -> Vec<ProfilePoint> {
    let shoulder = if shoulder_fraction.is_finite() {
        shoulder_fraction.clamp(0.0, 4.0)
    } else {
        0.0
    };
    vec![
        ProfilePoint::new(-1.0 - shoulder, -0.1),
        ProfilePoint::new(-1.0, 0.0),
        ProfilePoint::new(1.0, 0.0),
        ProfilePoint::new(1.0 + shoulder, -0.1),
    ]
}

/// Result of checking that a river goes downhill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverDescentReport {
    /// True when the curve never ascends more than the tolerance.
    pub descends: bool,
    /// Largest height increase between adjacent samples, in meters.
    pub maximum_ascent: f32,
}

/// Validate that a river spline generally descends from start to end.
pub fn validate_river_descent(
    spline: &CubicBezierSpline,
    sample_count: usize,
) -> RiverDescentReport {
    let segments = spline.segments();
    if segments == 0 {
        return RiverDescentReport {
            descends: false,
            maximum_ascent: 0.0,
        };
    }
    let count = if sample_count == 0 {
        32
    } else {
        sample_count.clamp(2, 4096)
    };
    let mut maximum_ascent = 0.0f32;
    let mut previous_height = spline.evaluate_position(0.0).map_or(0.0, |point| point.y);
    for index in 1..=count {
        let parameter = (index as f32 / count as f32) * segments as f32;
        if let Some(position) = spline.evaluate_position(parameter) {
            let ascent = position.y - previous_height;
            if ascent > maximum_ascent {
                maximum_ascent = ascent;
            }
            previous_height = position.y;
        }
    }
    RiverDescentReport {
        descends: maximum_ascent <= 1.0e-4,
        maximum_ascent,
    }
}

/// Lower the river bed along the spline until every point drops by the minimum amount.
pub fn lower_river_bed(spline: &mut CubicBezierSpline, minimum_drop_per_segment: f32) {
    if !spline.is_valid() {
        return;
    }
    let drop = if minimum_drop_per_segment.is_finite() && minimum_drop_per_segment > 0.0 {
        minimum_drop_per_segment
    } else {
        0.01
    };
    let Some(first_point) = spline.points.first() else {
        return;
    };
    let mut last_height = first_point.position.y;
    for point in spline.points.iter_mut().skip(1) {
        if point.position.y >= last_height - 1.0e-6 {
            point.position.y = last_height - drop;
        }
        last_height = point.position.y;
    }
    if spline.closed {
        let final_height = spline.points.last().map(|point| point.position.y);
        if let (Some(first_point), Some(final_height)) = (spline.points.first_mut(), final_height)
            && first_point.position.y >= final_height - 1.0e-6
        {
            first_point.position.y = final_height - drop;
        }
    }
}

/// Terrain modification along a path: carve or build with falloff.
#[derive(Clone, Copy, Debug)]
pub struct TerrainBrushAlongPath {
    pub radius: f32,
    /// Edge softness in `0..=1`; `0` is linear and `1` is sharp.
    pub falloff: f32,
    pub strength: f32,
    pub carve: bool,
}

impl Default for TerrainBrushAlongPath {
    fn default() -> Self {
        Self {
            radius: 2.0,
            falloff: 0.5,
            strength: 0.5,
            carve: true,
        }
    }
}

/// A single brush stamp on the terrain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStroke {
    pub position: Vec3,
    pub radius: f32,
    pub strength: f32,
    pub falloff: f32,
}

/// Sample the spline into brush stamps, evenly spaced by arc length.
pub fn path_brush_strokes(
    spline: &CubicBezierSpline,
    brush: &TerrainBrushAlongPath,
    spacing: f32,
) -> Vec<PathStroke> {
    let radius = if brush.radius.is_finite() && brush.radius > 0.0 {
        brush.radius
    } else {
        2.0
    };
    let strength = if brush.strength.is_finite() && brush.strength > 0.0 {
        brush.strength
    } else {
        0.5
    };
    let falloff = if brush.falloff.is_finite() {
        brush.falloff.clamp(0.0, 1.0)
    } else {
        0.5
    };
    spline
        .sample_with_frames(spacing)
        .into_iter()
        .map(|frame| PathStroke {
            position: frame.position,
            radius,
            strength,
            falloff,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::SplinePoint;

    fn straight_spline() -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::ZERO,
            handle_in: Vec3::ZERO,
            handle_out: Vec3::new(2.0, 0.0, 0.0),
            width: 4.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 0.0),
            handle_in: Vec3::new(-2.0, 0.0, 0.0),
            handle_out: Vec3::ZERO,
            width: 4.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline
    }

    #[test]
    fn sweep_produces_mesh_with_flow_and_margins() {
        let mesh = sweep_profile(&straight_spline(), &river_profile(), 0.5)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(!mesh.vertices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.flow.length() > 0.0)
        );
        assert!(mesh.vertices.iter().any(|vertex| vertex.margin_mask > 0.9));
        let center = mesh
            .vertices
            .iter()
            .find(|vertex| vertex.margin_mask < 0.01)
            .map(|vertex| vertex.position);
        assert!(center.is_some());
    }

    #[test]
    fn profile_uses_per_point_half_width() {
        let mut spline = straight_spline();
        spline.points[0].width = 2.0;
        spline.points[1].width = 8.0;
        let mesh =
            sweep_profile(&spline, &river_profile(), 1.0).unwrap_or_else(|error| panic!("{error}"));
        let first_edge = mesh
            .vertices
            .iter()
            .find(|vertex| vertex.margin_mask > 0.99)
            .map(|vertex| vertex.position);
        let last_edge = mesh
            .vertices
            .iter()
            .rev()
            .find(|vertex| vertex.margin_mask > 0.99)
            .map(|vertex| vertex.position);
        if let (Some(first), Some(last)) = (first_edge, last_edge) {
            let first_offset = (first.z - spline.points[0].position.z).abs();
            let last_offset = (last.z - spline.points[1].position.z).abs();
            assert!(
                (first_offset - 1.0).abs() < 0.1,
                "first width {first_offset}"
            );
            assert!((last_offset - 4.0).abs() < 0.1, "last width {last_offset}");
        }
    }

    #[test]
    fn invalid_profiles_are_refused() {
        let spline = straight_spline();
        assert_eq!(
            sweep_profile(&spline, &[], 1.0),
            Err(PathMeshError::EmptyProfile)
        );
        assert_eq!(
            sweep_profile(&CubicBezierSpline::new(), &river_profile(), 1.0),
            Err(PathMeshError::EmptySpline)
        );
    }

    #[test]
    fn river_descent_validation_and_lowering() {
        let mut spline = straight_spline();
        spline.points[0].position.y = 10.0;
        spline.points[1].position.y = 5.0;
        let report = validate_river_descent(&spline, 16);
        assert!(report.descends);

        spline.points[1].position.y = 20.0;
        let report = validate_river_descent(&spline, 16);
        assert!(!report.descends);
        assert!(report.maximum_ascent > 0.0);

        lower_river_bed(&mut spline, 0.5);
        assert!(spline.points[1].position.y < spline.points[0].position.y);
    }

    #[test]
    fn brush_strokes_follow_the_curve() {
        let brush = TerrainBrushAlongPath::default();
        let strokes = path_brush_strokes(&straight_spline(), &brush, 1.0);
        assert!(strokes.len() > 8);
        for pair in strokes.windows(2) {
            assert!((pair[1].position - pair[0].position).length() < 1.5);
        }
    }
}
