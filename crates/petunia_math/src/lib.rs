//! Small math value types that copy by value.
//!
//! These are the engine's own types. They appear in the schema, in save files and in the
//! network protocol, so they never depend on a third-party math library. Renderer adapters
//! convert them at the boundary.

mod polygon;
mod solid;

pub use polygon::{
    MAX_FACE_CORNERS, MAX_MESH_CORNERS, MAX_MESH_FACES, MAX_MESH_VERTICES, PolygonError,
    PolygonMesh,
};
pub use solid::{MAX_SOLID_TRIANGLES, SolidShape};

use std::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// A 2D vector (screen deltas, UV coordinates).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// A 3D vector. Right-handed, Y up, meters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub const UP: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn splat(value: f32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Unit vector in the same direction, or zero when the length is (almost) zero.
    pub fn normalized_or_zero(self) -> Self {
        let length = self.length();
        if length > 1.0e-12 && length.is_finite() {
            self * (1.0 / length)
        } else {
            Self::ZERO
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
            z: self.z * factor,
        }
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

fn opaque_alpha() -> f32 {
    1.0
}

/// A linear RGBA color. Components are usually in `0..=1`; HDR values are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    #[serde(default = "opaque_alpha")]
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn is_finite(self) -> bool {
        self.r.is_finite() && self.g.is_finite() && self.b.is_finite() && self.a.is_finite()
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Position, rotation (Euler degrees: x = pitch, y = yaw, z = roll) and scale.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation_degrees: Vec3::ZERO,
        scale: Vec3::ONE,
    };

    pub fn is_finite(&self) -> bool {
        self.position.is_finite() && self.rotation_degrees.is_finite() && self.scale.is_finite()
    }

    /// Rotates a direction vector by this transform's `rotation_degrees` (extrinsic YXZ / intrinsic ZXY).
    pub fn rotation_direction(&self, direction: Vec3) -> Vec3 {
        let (pitch, yaw, roll) = (
            self.rotation_degrees.x.to_radians(),
            self.rotation_degrees.y.to_radians(),
            self.rotation_degrees.z.to_radians(),
        );
        let rotate = |v: Vec3| rotate_y(rotate_x(rotate_z(v, roll), pitch), yaw);
        rotate(direction).normalized_or_zero()
    }

    /// Forward unit vector (-Z rotated by `rotation_degrees`).
    pub fn forward(&self) -> Vec3 {
        self.rotation_direction(Vec3::new(0.0, 0.0, -1.0))
    }

    /// Right unit vector (+X rotated by `rotation_degrees`).
    pub fn right(&self) -> Vec3 {
        self.rotation_direction(Vec3::new(1.0, 0.0, 0.0))
    }

    /// Up unit vector (+Y rotated by `rotation_degrees`).
    pub fn up(&self) -> Vec3 {
        self.rotation_direction(Vec3::new(0.0, 1.0, 0.0))
    }
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

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_of_x_and_y_is_z() {
        let z = Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(z, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn normalizing_zero_gives_zero() {
        assert_eq!(Vec3::ZERO.normalized_or_zero(), Vec3::ZERO);
        let unit = Vec3::new(3.0, 0.0, 4.0).normalized_or_zero();
        assert!((unit.length() - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn missing_color_alpha_defaults_to_opaque() {
        let color: Color = serde_json::from_str(r#"{"r":0.1,"g":0.2,"b":0.3}"#).unwrap();
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn identity_transform_vectors_match_cardinal_directions() {
        let transform = Transform::IDENTITY;
        assert_eq!(transform.forward(), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(transform.right(), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(transform.up(), Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn yaw_90_rotates_forward_to_negative_x() {
        let transform = Transform {
            rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
            ..Transform::IDENTITY
        };
        let fwd = transform.forward();
        assert!((fwd.x - (-1.0)).abs() < 1e-6);
        assert!(fwd.y.abs() < 1e-6);
        assert!(fwd.z.abs() < 1e-6);

        let rgt = transform.right();
        assert!(rgt.x.abs() < 1e-6);
        assert!(rgt.y.abs() < 1e-6);
        assert!((rgt.z - (-1.0)).abs() < 1e-6);
    }
}
