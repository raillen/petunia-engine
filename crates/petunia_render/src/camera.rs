// Fixed-size matrix math: every index is in range by construction.
#![allow(clippy::indexing_slicing)]

use petunia_math::{Transform, Vec3};

/// 4x4 matrix, column-major (OpenGL layout): element `[column * 4 + row]`.
pub type Mat4 = [f32; 16];

/// Perspective camera. Right-handed, Y up, looking down -Z at yaw 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            fov_degrees: 70.0,
            near: 0.05,
            far: 500.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let yaw = self.yaw_degrees.to_radians();
        let pitch = self.pitch_degrees.to_radians();
        Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        )
    }

    pub fn right(&self) -> Vec3 {
        let yaw = self.yaw_degrees.to_radians();
        Vec3::new(yaw.cos(), 0.0, yaw.sin())
    }

    pub fn view_matrix(&self) -> Mat4 {
        let forward = self.forward();
        let side = forward.cross(Vec3::UP).normalized_or_zero();
        let up = side.cross(forward);
        let eye = self.position;
        [
            side.x,
            up.x,
            -forward.x,
            0.0,
            side.y,
            up.y,
            -forward.y,
            0.0,
            side.z,
            up.z,
            -forward.z,
            0.0,
            -side.dot(eye),
            -up.dot(eye),
            forward.dot(eye),
            1.0,
        ]
    }

    pub fn projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        let focal = 1.0 / (self.fov_degrees.to_radians() * 0.5).tan();
        let range = self.near - self.far;
        [
            focal / aspect_ratio,
            0.0,
            0.0,
            0.0,
            0.0,
            focal,
            0.0,
            0.0,
            0.0,
            0.0,
            (self.far + self.near) / range,
            -1.0,
            0.0,
            0.0,
            2.0 * self.far * self.near / range,
            0.0,
        ]
    }

    pub fn view_projection(&self, aspect_ratio: f32) -> Mat4 {
        multiply(&self.projection_matrix(aspect_ratio), &self.view_matrix())
    }
}

/// Matrix product `a * b` (column-major).
pub fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[column * 4 + k];
            }
            out[column * 4 + row] = sum;
        }
    }
    out
}

fn rotate_z(v: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

fn rotate_x(v: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    Vec3::new(v.x, v.y * cos - v.z * sin, v.y * sin + v.z * cos)
}

fn rotate_y(v: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    Vec3::new(v.x * cos + v.z * sin, v.y, -v.x * sin + v.z * cos)
}

/// Model matrix `translation * rotation * scale`. Rotation is applied roll (z), then pitch (x),
/// then yaw (y), from `rotation_degrees`.
pub fn model_matrix(transform: &Transform) -> Mat4 {
    let rotation = transform.rotation_degrees;
    let (pitch, yaw, roll) = (
        rotation.x.to_radians(),
        rotation.y.to_radians(),
        rotation.z.to_radians(),
    );
    let rotate = |v: Vec3| rotate_y(rotate_x(rotate_z(v, roll), pitch), yaw);
    let scale = transform.scale;
    let column_x = rotate(Vec3::new(1.0, 0.0, 0.0)) * scale.x;
    let column_y = rotate(Vec3::new(0.0, 1.0, 0.0)) * scale.y;
    let column_z = rotate(Vec3::new(0.0, 0.0, 1.0)) * scale.z;
    let p = transform.position;
    [
        column_x.x, column_x.y, column_x.z, 0.0, column_y.x, column_y.y, column_y.z, 0.0,
        column_z.x, column_z.y, column_z.z, 0.0, p.x, p.y, p.z, 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform_point(matrix: &Mat4, point: Vec3) -> Vec3 {
        Vec3::new(
            matrix[0] * point.x + matrix[4] * point.y + matrix[8] * point.z + matrix[12],
            matrix[1] * point.x + matrix[5] * point.y + matrix[9] * point.z + matrix[13],
            matrix[2] * point.x + matrix[6] * point.y + matrix[10] * point.z + matrix[14],
        )
    }

    #[test]
    fn camera_at_origin_looks_down_negative_z() {
        let camera = Camera::default();
        let forward = camera.forward();
        assert!((forward.z + 1.0).abs() < 1.0e-6);
        assert!((camera.right().x - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn view_matrix_moves_the_world_opposite_to_the_camera() {
        let camera = Camera {
            position: Vec3::new(0.0, 0.0, 5.0),
            ..Camera::default()
        };
        let view = camera.view_matrix();
        let origin_in_view = transform_point(&view, Vec3::ZERO);
        assert!(
            (origin_in_view.z + 5.0).abs() < 1.0e-5,
            "origin is 5 m in front of the camera"
        );
    }

    #[test]
    fn model_matrix_applies_scale_then_translation() {
        let transform = Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            scale: Vec3::new(2.0, 2.0, 2.0),
            ..Transform::IDENTITY
        };
        let moved = transform_point(&model_matrix(&transform), Vec3::new(1.0, 0.0, 0.0));
        assert!((moved.x - 3.0).abs() < 1.0e-6 && (moved.y - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn transform_directions_match_model_matrix_axes() {
        let transform = Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation_degrees: Vec3::new(15.0, 45.0, -30.0),
            scale: Vec3::ONE,
        };
        let model = model_matrix(&transform);
        let col_x = Vec3::new(model[0], model[1], model[2]);
        let col_y = Vec3::new(model[4], model[5], model[6]);
        let col_z = Vec3::new(model[8], model[9], model[10]);

        let right = transform.right();
        let up = transform.up();
        let forward = transform.forward();

        assert!((right.x - col_x.x).abs() < 1e-6);
        assert!((right.y - col_x.y).abs() < 1e-6);
        assert!((right.z - col_x.z).abs() < 1e-6);

        assert!((up.x - col_y.x).abs() < 1e-6);
        assert!((up.y - col_y.y).abs() < 1e-6);
        assert!((up.z - col_y.z).abs() < 1e-6);

        assert!((forward.x - (-col_z.x)).abs() < 1e-6);
        assert!((forward.y - (-col_z.y)).abs() < 1e-6);
        assert!((forward.z - (-col_z.z)).abs() < 1e-6);
    }
}
