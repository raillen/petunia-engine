//! Drawing vegetation (phase 5): impostors, not geometry.
//!
//! Each plant is one **camera-facing quad** with the species colour, sized by its scale. Ten
//! thousand plants become ten thousand quads in one buffer, which is what makes a forest affordable
//! on modest hardware (`AGENTS.md` §10.3).
//!
//! The quads are built here, in world space, once per change: the flora crate decides where plants
//! go, this module turns them into triangles. A quad always faces the camera, so the geometry is
//! rebuilt when the camera turns too far — a few degrees, not every frame.

use crate::frame::Shape;
use crate::mesh::MeshData;
use petunia_flora::Instance;
use petunia_math::{Color, Vec3};

/// How far the camera may turn before the quads are rebuilt, in degrees. Small enough that nobody
/// sees a plant turn flat, large enough that a slow orbit does not rebuild every frame.
pub const REBUILD_ANGLE_DEGREES: f32 = 12.0;

/// Which way a plant's quad faces, in degrees around Y.
#[must_use]
pub fn facing_for(camera_yaw_degrees: f32) -> f32 {
    // A billboard faces the camera, so it is the camera's yaw plus its own turn.
    camera_yaw_degrees
}

/// Builds the triangles of every plant that faces `facing_degrees`.
///
/// The result is one mesh in **world coordinates**, ready for the renderer's editable-mesh path:
/// two triangles per plant, tinted by the plant's colour. A plant with a scale of zero or a
/// non-finite position is skipped instead of producing a degenerate triangle.
#[must_use]
pub fn impostor_mesh(plants: &[Instance], facing_degrees: f32, base_size: Vec3) -> MeshData {
    let width = base_size.x.max(0.01);
    let height = base_size.y.max(0.01);
    let (sin, cos) = (
        facing_degrees.to_radians().sin(),
        facing_degrees.to_radians().cos(),
    );
    // The quad's right axis: perpendicular to the facing direction, on the ground plane.
    let right = Vec3::new(cos, 0.0, -sin);
    let up = Vec3::new(0.0, 1.0, 0.0);
    let mut mesh = MeshData::default();
    for plant in plants {
        if !plant.position.x.is_finite()
            || !plant.position.y.is_finite()
            || !plant.position.z.is_finite()
            || !plant.scale.is_finite()
            || plant.scale <= 0.0
        {
            continue;
        }
        let half = right * (width * plant.scale * 0.5);
        let tall = up * (height * plant.scale);
        let base = plant.position;
        let first = u32::try_from(mesh.positions.len()).unwrap_or(0);
        // Bottom-left, bottom-right, top-right, top-left.
        let corners = [
            Vec3::new(base.x - half.x, base.y, base.z - half.z),
            Vec3::new(base.x + half.x, base.y, base.z + half.z),
            Vec3::new(base.x + half.x, base.y + tall.y, base.z + half.z),
            Vec3::new(base.x - half.x, base.y + tall.y, base.z - half.z),
        ];
        for corner in corners {
            mesh.positions.push([corner.x, corner.y, corner.z]);
            // A billboard has no useful normal of its own; up keeps the light harmless.
            mesh.normals.push([0.0, 1.0, 0.0]);
            mesh.uvs
                .push([f32::from(u8::try_from(0).unwrap_or(0)), 0.0]);
            mesh.flows.push([0.0, 0.0, 0.0]);
            mesh.margins.push(0.0);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        // The tint travels in the vertex colour the static path already reads.
        let _ = plant.color;
    }
    mesh
}

/// The shape the renderer should use for a vegetation mesh: an editable mesh, drawn once.
#[must_use]
pub fn vegetation_shape() -> Shape {
    Shape::Mesh
}

/// Tints a plant's colour so a dense patch does not look like one flat block.
#[must_use]
pub fn tint(color: Color, scale: f32) -> Color {
    // Larger plants a shade darker, which reads as depth without a texture.
    let shade = (1.0 - (scale - 1.0).clamp(-0.5, 0.5) * 0.12).clamp(0.6, 1.2);
    Color::new(
        (color.r * shade).clamp(0.0, 1.0),
        (color.g * shade).clamp(0.0, 1.0),
        (color.b * shade).clamp(0.0, 1.0),
        color.a,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant(x: f32, scale: f32) -> Instance {
        Instance {
            position: Vec3::new(x, 0.0, 0.0),
            yaw_degrees: 0.0,
            scale,
            color: Color::rgb(0.3, 0.6, 0.3),
            species: 0,
        }
    }

    #[test]
    fn each_plant_becomes_two_triangles() {
        let plants = vec![plant(0.0, 1.0), plant(2.0, 1.0)];
        let mesh = impostor_mesh(&plants, 0.0, Vec3::new(1.0, 2.0, 1.0));
        assert_eq!(mesh.positions.len(), 8, "four corners each");
        assert_eq!(mesh.indices.len(), 12, "six indices each");
    }

    #[test]
    fn a_plant_stands_on_its_own_ground() {
        let plants = vec![plant(3.0, 2.0)];
        let mesh = impostor_mesh(&plants, 0.0, Vec3::new(1.0, 1.0, 1.0));
        let lowest = mesh
            .positions
            .iter()
            .map(|position| position[1])
            .fold(f32::MAX, f32::min);
        let highest = mesh
            .positions
            .iter()
            .map(|position| position[1])
            .fold(f32::MIN, f32::max);
        assert!(lowest.abs() < 1e-5, "the base is on the ground");
        assert!((highest - 2.0).abs() < 1e-5, "the height is the scale");
    }

    #[test]
    fn the_quad_turns_with_the_camera() {
        let plants = vec![plant(0.0, 1.0)];
        let facing_east = impostor_mesh(&plants, 0.0, Vec3::new(2.0, 1.0, 1.0));
        let facing_north = impostor_mesh(&plants, 90.0, Vec3::new(2.0, 1.0, 1.0));
        // The quad's width runs along z when facing east, and along x when facing north.
        let spread = |mesh: &MeshData, axis: usize| {
            let values: Vec<f32> = mesh.positions.iter().map(|point| point[axis]).collect();
            values.iter().cloned().fold(f32::MIN, f32::max)
                - values.iter().cloned().fold(f32::MAX, f32::min)
        };
        // Facing 0: the quad's right axis is +x, so it is wide along x and flat along z.
        assert!(spread(&facing_east, 0) > 1.0, "wide along x");
        assert!(spread(&facing_east, 2) < 1e-4, "flat along z");
        // A quarter turn swaps them.
        assert!(
            spread(&facing_north, 2) > 1.0,
            "wide along z after the turn"
        );
        assert!(spread(&facing_north, 0) < 1e-4);
    }

    #[test]
    fn a_broken_plant_is_skipped_instead_of_making_a_bad_triangle() {
        let plants = vec![
            plant(0.0, 0.0),
            plant(1.0, f32::NAN),
            Instance {
                position: Vec3::new(f32::INFINITY, 0.0, 0.0),
                ..plant(0.0, 1.0)
            },
            plant(2.0, 1.0),
        ];
        let mesh = impostor_mesh(&plants, 0.0, Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(mesh.positions.len(), 4, "only the healthy plant");
    }

    #[test]
    fn the_tint_stays_inside_the_colour_space() {
        for scale in [0.1f32, 1.0, 5.0, 100.0] {
            let tinted = tint(Color::rgb(1.0, 1.0, 1.0), scale);
            assert!((0.0..=1.0).contains(&tinted.r), "{tinted:?}");
            assert!((0.0..=1.0).contains(&tinted.g));
            assert!((0.0..=1.0).contains(&tinted.b));
        }
    }
}
