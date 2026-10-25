//! Ground grid and world axes, ported from Petunia3D (`render/src/scene.rs`: `grid_plane_lines`,
//! `grid_axis_lines`). Every 10th line is "major" and gets more contrast: it gives a sense of
//! scale without the grid competing with the objects. Unlike Petunia3D's dark-canvas colors,
//! lines here are black with transparency, so they read on light and dark floors alike.

use petunia_math::{Color, Vec3};

/// One colored line.
pub type GridLine = (Vec3, Vec3, Color);

/// Height of the grid above y = 0, so it is not hidden inside a floor whose top is at zero.
pub const GRID_LIFT: f32 = 0.002;
/// Most lines per side: a tiny spacing on a huge grid cannot produce millions of lines.
const MAX_STEPS_PER_SIDE: f32 = 2000.0;

const MINOR: Color = Color::new(0.0, 0.0, 0.0, 0.12);
const MAJOR: Color = Color::new(0.0, 0.0, 0.0, 0.26);
const AXIS_X: Color = Color::new(0.85, 0.27, 0.27, 0.9);
const AXIS_Z: Color = Color::new(0.19, 0.50, 0.86, 0.9);

/// Lines of the grid plane (without the axes), `extent` meters from the origin each way.
pub fn grid_lines(extent: f32, spacing: f32) -> Vec<GridLine> {
    let extent = if extent.is_finite() {
        extent.max(1.0)
    } else {
        1.0
    };
    let spacing = if spacing.is_finite() && spacing > 0.0 {
        spacing
    } else {
        1.0
    };
    let step = spacing.max(extent / MAX_STEPS_PER_SIDE).min(extent);
    let steps = (extent / step).ceil() as i32;
    let mut lines = Vec::new();
    for index in -steps..=steps {
        let offset = index as f32 * step;
        if offset.abs() > extent + 1.0e-4 || index == 0 {
            continue; // the axes are drawn instead of the zero lines
        }
        let color = if index % 10 == 0 { MAJOR } else { MINOR };
        lines.push((
            Vec3::new(offset, GRID_LIFT, -extent),
            Vec3::new(offset, GRID_LIFT, extent),
            color,
        ));
        lines.push((
            Vec3::new(-extent, GRID_LIFT, offset),
            Vec3::new(extent, GRID_LIFT, offset),
            color,
        ));
    }
    lines
}

/// The X (red) and Z (blue) world axes along the grid.
pub fn axis_lines(extent: f32) -> Vec<GridLine> {
    let extent = if extent.is_finite() {
        extent.max(1.0)
    } else {
        1.0
    };
    vec![
        (
            Vec3::new(-extent, GRID_LIFT, 0.0),
            Vec3::new(extent, GRID_LIFT, 0.0),
            AXIS_X,
        ),
        (
            Vec3::new(0.0, GRID_LIFT, -extent),
            Vec3::new(0.0, GRID_LIFT, extent),
            AXIS_Z,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_spacing_never_explodes_the_line_count() {
        assert!(grid_lines(100.0, 1.0e-9).len() <= 2 * (2 * 2000 + 1));
        assert!(!grid_lines(50.0, 1.0).is_empty());
        assert_eq!(axis_lines(50.0).len(), 2);
        assert!(grid_lines(f32::NAN, f32::NAN).len() < 10);
    }
}
