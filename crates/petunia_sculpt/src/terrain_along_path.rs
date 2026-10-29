//! Apply terrain modifications along paths using heightfields.
//!
//! Carve/flatten/embank along river/road splines with falloff. Every stamp is applied to every
//! chunk that the brush radius touches, so a path that crosses a chunk border stays continuous.

use petunia_math::{CubicBezierSpline, Vec3};
use petunia_world::Heightfield;

use crate::path_network::{PathStroke, TerrainBrushAlongPath, path_brush_strokes};

fn apply_stroke(heightfield: &mut Heightfield, stroke: PathStroke, carve: bool) {
    for chunk_position in heightfield.chunks_touching(stroke.position, stroke.radius) {
        if let Some(chunk) = heightfield.get_or_create_chunk(chunk_position) {
            chunk.apply_brush_with_falloff(
                stroke.position,
                stroke.radius,
                stroke.strength,
                carve,
                stroke.falloff,
            );
        }
    }
}

/// Carve a channel along a spline into a heightfield. Returns the number of stamps applied.
pub fn carve_along_spline(
    heightfield: &mut Heightfield,
    spline: &CubicBezierSpline,
    brush: &TerrainBrushAlongPath,
    spacing: f32,
) -> usize {
    let strokes = path_brush_strokes(spline, brush, spacing);
    for stroke in &strokes {
        apply_stroke(heightfield, *stroke, brush.carve);
    }
    strokes.len()
}

/// Carve a river channel using the width and depth stored on each spline point.
///
/// The carved bed follows the spline height; call [`crate::lower_river_bed`] first when the
/// river must descend.
pub fn carve_river_channel(
    heightfield: &mut Heightfield,
    spline: &CubicBezierSpline,
    spacing: f32,
    falloff: f32,
) -> usize {
    let falloff = if falloff.is_finite() {
        falloff.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let frames = spline.sample_with_frames(spacing);
    for frame in &frames {
        let (width, depth, _extra) = spline
            .evaluate_parameters(frame.parameter)
            .unwrap_or((2.0, 1.0, 0.0));
        let radius = if width.is_finite() && width > 0.0 {
            width * 0.5
        } else {
            1.0
        };
        let strength = if depth.is_finite() && depth > 0.0 {
            depth
        } else {
            0.5
        };
        apply_stroke(
            heightfield,
            PathStroke {
                position: frame.position,
                radius,
                strength,
                falloff,
            },
            true,
        );
    }
    frames.len()
}

/// Flatten terrain along a road spline to the spline height (road bed).
pub fn flatten_along_spline(
    heightfield: &mut Heightfield,
    spline: &CubicBezierSpline,
    radius: f32,
    spacing: f32,
    strength: f32,
) -> usize {
    if !radius.is_finite() || radius <= 0.0 {
        return 0;
    }
    let frames = spline.sample_with_frames(spacing);
    for frame in &frames {
        for chunk_position in heightfield.chunks_touching(frame.position, radius) {
            if let Some(chunk) = heightfield.get_or_create_chunk(chunk_position) {
                chunk.flatten_region(frame.position, radius, frame.position.y, strength);
            }
        }
    }
    frames.len()
}

/// Embank/cut along a path: add terrain when `build_up`, carve when not.
pub fn embank_along_spline(
    heightfield: &mut Heightfield,
    spline: &CubicBezierSpline,
    radius: f32,
    spacing: f32,
    strength: f32,
    build_up: bool,
) -> usize {
    if !radius.is_finite() || radius <= 0.0 {
        return 0;
    }
    let frames = spline.sample_with_frames(spacing);
    for frame in &frames {
        let stroke = PathStroke {
            position: frame.position,
            radius,
            strength,
            falloff: 0.5,
        };
        apply_stroke(heightfield, stroke, !build_up);
    }
    frames.len()
}

/// Sample the terrain height under a path, or `None` when no chunk covers that point.
pub fn sample_along_spline(
    heightfield: &Heightfield,
    spline: &CubicBezierSpline,
    spacing: f32,
) -> Vec<Option<f32>> {
    spline
        .sample_with_frames(spacing)
        .into_iter()
        .map(|frame| heightfield.sample_world(frame.position.x, frame.position.z))
        .collect()
}

/// A road span that needs a bridge or a culvert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadStructureKind {
    /// The road surface is well above the terrain: build a bridge.
    Bridge,
    /// The terrain is well above the road surface: cut a passage or build a culvert.
    Culvert,
}

/// A contiguous span of a road where the terrain height differs from the road height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadStructure {
    pub kind: RoadStructureKind,
    pub start_parameter: f32,
    pub end_parameter: f32,
    pub start_position: Vec3,
    pub end_position: Vec3,
    pub maximum_height_difference: f32,
}

/// Find bridge and culvert spans along a road by comparing the spline height to the terrain.
///
/// A sample with no terrain chunk under it is treated as "unknown" and breaks a span, so a
/// missing chunk can never invent a bridge. Spans shorter than two samples are ignored.
pub fn detect_road_structures(
    heightfield: &Heightfield,
    spline: &CubicBezierSpline,
    spacing: f32,
    minimum_height_difference: f32,
) -> Vec<RoadStructure> {
    let threshold = if minimum_height_difference.is_finite() && minimum_height_difference >= 0.0 {
        minimum_height_difference
    } else {
        2.0
    };
    let frames = spline.sample_with_frames(spacing);
    let mut structures = Vec::new();
    let mut current: Option<RoadStructure> = None;
    for frame in &frames {
        let Some(terrain_height) = heightfield.sample_world(frame.position.x, frame.position.z)
        else {
            if let Some(structure) = current.take() {
                push_structure(&mut structures, structure);
            }
            continue;
        };
        let difference = frame.position.y - terrain_height;
        let kind = if difference > threshold {
            Some(RoadStructureKind::Bridge)
        } else if difference < -threshold {
            Some(RoadStructureKind::Culvert)
        } else {
            None
        };
        match (current.as_mut(), kind) {
            (Some(structure), Some(kind)) if structure.kind == kind => {
                structure.end_parameter = frame.parameter;
                structure.end_position = frame.position;
                structure.maximum_height_difference =
                    structure.maximum_height_difference.max(difference.abs());
            }
            (_, kind) => {
                if let Some(structure) = current.take() {
                    push_structure(&mut structures, structure);
                }
                if let Some(kind) = kind {
                    current = Some(RoadStructure {
                        kind,
                        start_parameter: frame.parameter,
                        end_parameter: frame.parameter,
                        start_position: frame.position,
                        end_position: frame.position,
                        maximum_height_difference: difference.abs(),
                    });
                }
            }
        }
    }
    if let Some(structure) = current {
        push_structure(&mut structures, structure);
    }
    structures
}

fn push_structure(structures: &mut Vec<RoadStructure>, structure: RoadStructure) {
    if (structure.end_position - structure.start_position).length_squared() > 0.0 {
        structures.push(structure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_network::TerrainBrushAlongPath;
    use petunia_math::{CubicBezierSpline, SplinePoint};
    use petunia_world::HEIGHT_CHUNK_SIZE;

    fn flat_spline(from: Vec3, to: Vec3) -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        let direction = (to - from) * 0.25;
        spline.points.push(SplinePoint {
            position: from,
            handle_in: Vec3::ZERO,
            handle_out: direction,
            width: 4.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline.points.push(SplinePoint {
            position: to,
            handle_in: -direction,
            handle_out: Vec3::ZERO,
            width: 4.0,
            depth: 1.0,
            extra: 0.0,
        });
        spline
    }

    #[test]
    fn carve_along_spline_lowers_terrain_near_the_path() {
        let mut heightfield = Heightfield::new(1.0);
        let spline = flat_spline(Vec3::new(32.0, 0.0, 32.0), Vec3::new(40.0, 0.0, 32.0));
        let brush = TerrainBrushAlongPath {
            radius: 3.0,
            falloff: 0.5,
            strength: 0.5,
            carve: true,
        };
        let stamps = carve_along_spline(&mut heightfield, &spline, &brush, 1.0);
        assert!(stamps > 0);
        let on_path = heightfield.sample_world(36.0, 32.0).unwrap_or(0.0);
        let away = heightfield.sample_world(36.0, 50.0).unwrap_or(0.0);
        assert!(on_path < -0.1, "path height {on_path}");
        assert!(away.abs() < 1.0e-6, "away height {away}");
    }

    #[test]
    fn carve_crosses_chunk_borders() {
        let mut heightfield = Heightfield::new(1.0);
        let border = (HEIGHT_CHUNK_SIZE as f32) - 0.5;
        let spline = flat_spline(
            Vec3::new(border - 1.0, 0.0, 10.0),
            Vec3::new(border + 1.0, 0.0, 10.0),
        );
        let brush = TerrainBrushAlongPath {
            radius: 2.0,
            falloff: 0.5,
            strength: 0.5,
            carve: true,
        };
        carve_along_spline(&mut heightfield, &spline, &brush, 0.5);
        assert!(heightfield.chunk([0, 0]).is_some());
        assert!(heightfield.chunk([1, 0]).is_some());
        let left = heightfield
            .chunk([0, 0])
            .map(|chunk| chunk.get(HEIGHT_CHUNK_SIZE - 1, 10));
        let right = heightfield.chunk([1, 0]).map(|chunk| chunk.get(0, 10));
        assert!(left.unwrap_or(0.0) < 0.0);
        assert!(right.unwrap_or(0.0) < 0.0);
    }

    #[test]
    fn flatten_along_spline_uses_the_spline_height() {
        let mut heightfield = Heightfield::new(1.0);
        let spline = flat_spline(Vec3::new(10.0, 3.0, 10.0), Vec3::new(20.0, 3.0, 10.0));
        let stamps = flatten_along_spline(&mut heightfield, &spline, 2.0, 1.0, 1.0);
        assert!(stamps > 0);
        let height = heightfield.sample_world(15.0, 10.0).unwrap_or(0.0);
        assert!((height - 3.0).abs() < 1.0e-3, "height {height}");
    }

    #[test]
    fn embank_builds_up_and_cuts_down() {
        let spline = flat_spline(Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 10.0));
        let mut raised = Heightfield::new(1.0);
        embank_along_spline(&mut raised, &spline, 2.0, 1.0, 0.5, true);
        let raised_height = raised.sample_world(15.0, 10.0).unwrap_or(0.0);
        assert!(raised_height > 0.0);
        let mut cut = Heightfield::new(1.0);
        embank_along_spline(&mut cut, &spline, 2.0, 1.0, 0.5, false);
        let cut_height = cut.sample_world(15.0, 10.0).unwrap_or(0.0);
        assert!(cut_height < 0.0);
    }

    #[test]
    fn river_channel_follows_per_point_depth() {
        let mut heightfield = Heightfield::new(1.0);
        let mut spline = flat_spline(Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 10.0));
        spline.points[0].depth = 0.5;
        spline.points[1].depth = 3.0;
        carve_river_channel(&mut heightfield, &spline, 1.0, 0.5);
        let shallow = heightfield.sample_world(11.0, 10.0).unwrap_or(0.0);
        let deep = heightfield.sample_world(19.0, 10.0).unwrap_or(0.0);
        assert!(deep < shallow, "deep {deep} shallow {shallow}");
    }

    #[test]
    fn bridges_and_culverts_are_detected() {
        let mut heightfield = Heightfield::new(1.0);
        let chunk = heightfield
            .get_or_create_chunk([0, 0])
            .unwrap_or_else(|| panic!("chunk"));
        for z in 0..HEIGHT_CHUNK_SIZE {
            for x in 0..HEIGHT_CHUNK_SIZE {
                chunk.set(x, z, 0.0);
            }
        }
        for x in 12..=16 {
            chunk.set(x, 10, -8.0);
        }
        for x in 25..=28 {
            chunk.set(x, 10, 8.0);
        }
        let spline = flat_spline(Vec3::new(8.0, 0.0, 10.0), Vec3::new(32.0, 0.0, 10.0));
        let structures = detect_road_structures(&heightfield, &spline, 0.5, 2.0);
        assert!(
            structures
                .iter()
                .any(|structure| structure.kind == RoadStructureKind::Bridge)
        );
        assert!(
            structures
                .iter()
                .any(|structure| structure.kind == RoadStructureKind::Culvert)
        );
    }

    #[test]
    fn missing_terrain_never_invents_a_bridge() {
        let heightfield = Heightfield::new(1.0);
        let spline = flat_spline(Vec3::new(8.0, 5.0, 10.0), Vec3::new(32.0, 5.0, 10.0));
        assert!(detect_road_structures(&heightfield, &spline, 0.5, 2.0).is_empty());
    }

    #[test]
    fn hostile_arguments_are_ignored() {
        let mut heightfield = Heightfield::new(1.0);
        let spline = flat_spline(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(
            flatten_along_spline(&mut heightfield, &spline, 0.0, 1.0, 1.0),
            0
        );
        assert_eq!(
            embank_along_spline(&mut heightfield, &spline, -1.0, 1.0, 1.0, true),
            0
        );
        assert!(heightfield.chunks.is_empty());
    }
}
