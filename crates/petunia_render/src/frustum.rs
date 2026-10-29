//! The view volume as six planes: what a frame can possibly show.
//!
//! Built from a view-projection matrix (Gribb–Hartmann plane extraction), so the same camera and
//! viewport that the GPU uses decide what gets built. A test is conservative: an object reported
//! as visible may still be off screen, but one reported as hidden is really hidden.

use petunia_math::Vec3;

use crate::camera::Mat4;

/// A half-space as `(normal, distance)`: the inside is `normal · p + distance >= 0`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Plane {
    normal: Vec3,
    distance: f32,
}

/// Six planes of the view volume, in the view matrix's space (world space here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    /// Reads the planes out of a view-projection matrix (column-major). A degenerate matrix gives
    /// a volume that contains everything, so a bad matrix can never make the scene disappear.
    pub fn from_view_projection(matrix: &Mat4) -> Self {
        let at = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
        let row = |index: usize| [at(index), at(4 + index), at(8 + index), at(12 + index)];
        let (x, y, z, w) = (row(0), row(1), row(2), row(3));
        let combine = |left: [f32; 4], right: [f32; 4], sign: f32| {
            let [left_x, left_y, left_z, left_w] = left;
            let [right_x, right_y, right_z, right_w] = right;
            [
                left_x + right_x * sign,
                left_y + right_y * sign,
                left_z + right_z * sign,
                left_w + right_w * sign,
            ]
        };
        let raw = [
            combine(w, x, 1.0),  // left
            combine(w, x, -1.0), // right
            combine(w, y, 1.0),  // bottom
            combine(w, y, -1.0), // top
            combine(w, z, 1.0),  // near
            combine(w, z, -1.0), // far
        ];
        let mut planes = [Plane {
            normal: Vec3::ZERO,
            distance: 0.0,
        }; 6];
        for (slot, values) in planes.iter_mut().zip(raw.iter()) {
            let [normal_x, normal_y, normal_z, offset] = *values;
            let normal = Vec3::new(normal_x, normal_y, normal_z);
            let length = normal.length();
            if !length.is_finite() || length < 1.0e-6 {
                // A matrix without a usable plane cannot reject anything.
                return Self::everything();
            }
            *slot = Plane {
                normal: normal * (1.0 / length),
                distance: offset / length,
            };
        }
        Self { planes }
    }

    /// A volume that contains everything: no culling.
    pub fn everything() -> Self {
        Self {
            planes: [Plane {
                normal: Vec3::ZERO,
                distance: f32::INFINITY,
            }; 6],
        }
    }

    /// Whether a sphere is at least partly inside the view volume.
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        if !center.is_finite() || !radius.is_finite() {
            // Never cull what cannot be measured.
            return true;
        }
        let radius = radius.max(0.0);
        self.planes
            .iter()
            .all(|plane| plane.normal.dot(center) + plane.distance >= -radius)
    }

    /// Whether a box is at least partly inside the view volume.
    pub fn contains_box(&self, minimum: Vec3, maximum: Vec3) -> bool {
        let center = (minimum + maximum) * 0.5;
        let extent = (maximum - minimum) * 0.5;
        self.contains_sphere(center, extent.length())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, multiply};

    fn camera() -> Camera {
        Camera {
            position: Vec3::new(0.0, 0.0, 10.0),
            ..Camera::default()
        }
    }

    fn frustum() -> Frustum {
        let camera = camera();
        let view_projection =
            multiply(&camera.projection_matrix(16.0 / 9.0), &camera.view_matrix());
        Frustum::from_view_projection(&view_projection)
    }

    #[test]
    fn a_sphere_in_front_of_the_camera_is_visible() {
        assert!(frustum().contains_sphere(Vec3::ZERO, 1.0));
    }

    #[test]
    fn a_sphere_behind_the_camera_is_hidden() {
        // The camera sits at z = 10 looking down -z: z = 20 is behind it.
        assert!(!frustum().contains_sphere(Vec3::new(0.0, 0.0, 20.0), 1.0));
    }

    #[test]
    fn a_sphere_far_to_the_side_is_hidden() {
        assert!(!frustum().contains_sphere(Vec3::new(500.0, 0.0, -10.0), 2.0));
    }

    #[test]
    fn a_sphere_beyond_the_far_plane_is_hidden() {
        assert!(!frustum().contains_sphere(Vec3::new(0.0, 0.0, -1000.0), 2.0));
    }

    #[test]
    fn a_huge_sphere_around_the_camera_is_visible() {
        assert!(frustum().contains_sphere(Vec3::new(0.0, 0.0, 10.0), 400.0));
    }

    #[test]
    fn the_box_test_follows_the_sphere_test() {
        let frustum = frustum();
        assert!(frustum.contains_box(Vec3::splat(-1.0), Vec3::splat(1.0)));
        assert!(!frustum.contains_box(Vec3::new(400.0, -1.0, 400.0), Vec3::new(401.0, 1.0, 401.0)));
    }

    #[test]
    fn a_matrix_without_planes_rejects_nothing() {
        let empty = [0.0_f32; 16];
        let frustum = Frustum::from_view_projection(&empty);
        assert!(frustum.contains_sphere(Vec3::new(1.0e6, 0.0, 0.0), 1.0));
        assert!(frustum.contains_sphere(Vec3::new(f32::NAN, 0.0, 0.0), f32::NAN));
    }
}
