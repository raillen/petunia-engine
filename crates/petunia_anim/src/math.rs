//! Matrices and quaternions for skeletal animation.
//!
//! `petunia_math` has the value types the engine uses everywhere (`Vec3`, `Transform`); skinning
//! needs a 4x4 matrix and a rotation that interpolates well, and those live here so the graphics
//! crate does not have to depend on the animation crate's internals.

use petunia_math::Vec3;
use serde::{Deserialize, Serialize};

/// A column-major 4x4 matrix, like OpenGL and glTF use.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mat4(pub [f32; 16]);

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Self = Self([
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);

    #[must_use]
    pub fn translation(offset: Vec3) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.0[12] = offset.x;
        matrix.0[13] = offset.y;
        matrix.0[14] = offset.z;
        matrix
    }

    #[must_use]
    pub fn scaling(scale: Vec3) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.0[0] = scale.x;
        matrix.0[5] = scale.y;
        matrix.0[10] = scale.z;
        matrix
    }

    /// A rotation from a unit quaternion.
    #[must_use]
    pub fn from_quaternion(rotation: Quat) -> Self {
        let [x, y, z, w] = rotation.normalized().0;
        Self([
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ])
    }

    /// Translation, then rotation, then scale — the order glTF defines for a node's transform.
    #[must_use]
    pub fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self::multiply(
            &Self::multiply(
                &Self::translation(translation),
                &Self::from_quaternion(rotation),
            ),
            &Self::scaling(scale),
        )
    }

    /// `self * other`, column-major.
    ///
    /// Written with iterators over fixed-size arrays, so there is no index that could be out of
    /// bounds and no panic path in a hot function.
    #[must_use]
    pub fn multiply(&self, other: &Self) -> Self {
        let mut out = [0.0f32; 16];
        for (index, slot) in out.iter_mut().enumerate() {
            let (column, row) = (index / 4, index % 4);
            let mut sum = 0.0f32;
            for inner in 0..4 {
                let left = self.0.get(inner * 4 + row).copied().unwrap_or(0.0);
                let right = other.0.get(column * 4 + inner).copied().unwrap_or(0.0);
                sum += left * right;
            }
            *slot = sum;
        }
        Self(out)
    }

    /// The inverse of a rigid + scale matrix, or identity when it cannot be inverted.
    ///
    /// Skeletons only ever hold TRS matrices, so this is the fast path: invert the 3x3 part and
    /// negate the translation. A singular matrix (a zero scale, which a hostile file may contain)
    /// returns identity instead of a NaN-filled matrix that would poison every vertex.
    #[must_use]
    pub fn inverse_rigid(&self) -> Self {
        let m = self.0;
        let at = |index: usize| m.get(index).copied().unwrap_or(0.0);
        let a = [
            [at(0), at(4), at(8)],
            [at(1), at(5), at(9)],
            [at(2), at(6), at(10)],
        ];
        let determinant = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        if !determinant.is_finite() || determinant.abs() < 1e-12 {
            return Self::IDENTITY;
        }
        let inv_det = 1.0 / determinant;
        let mut inverse = Self::IDENTITY;
        for row in 0..3 {
            for column in 0..3 {
                // Cofactor of (row, column), transposed for the inverse.
                let (r0, r1) = ((row + 1) % 3, (row + 2) % 3);
                let (c0, c1) = ((column + 1) % 3, (column + 2) % 3);
                let read = |row_index: usize, column_index: usize| {
                    a.get(row_index)
                        .and_then(|line| line.get(column_index))
                        .copied()
                        .unwrap_or(0.0)
                };
                let cofactor = read(r0, c0) * read(r1, c1) - read(r0, c1) * read(r1, c0);
                if let Some(slot) = inverse.0.get_mut(column * 4 + row) {
                    *slot = cofactor * inv_det;
                }
            }
        }
        let translation = Vec3::new(at(12), at(13), at(14));
        let inverse_at = |index: usize| inverse.0.get(index).copied().unwrap_or(0.0);
        let inverse_translation = Vec3::new(
            -(inverse_at(0) * translation.x
                + inverse_at(4) * translation.y
                + inverse_at(8) * translation.z),
            -(inverse_at(1) * translation.x
                + inverse_at(5) * translation.y
                + inverse_at(9) * translation.z),
            -(inverse_at(2) * translation.x
                + inverse_at(6) * translation.y
                + inverse_at(10) * translation.z),
        );
        if let Some(slot) = inverse.0.get_mut(12) {
            *slot = inverse_translation.x;
        }
        if let Some(slot) = inverse.0.get_mut(13) {
            *slot = inverse_translation.y;
        }
        if let Some(slot) = inverse.0.get_mut(14) {
            *slot = inverse_translation.z;
        }
        inverse
    }

    /// Transforms a point (the translation column counts).
    #[must_use]
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let at = |index: usize| self.0.get(index).copied().unwrap_or(0.0);
        Vec3::new(
            at(0) * point.x + at(4) * point.y + at(8) * point.z + at(12),
            at(1) * point.x + at(5) * point.y + at(9) * point.z + at(13),
            at(2) * point.x + at(6) * point.y + at(10) * point.z + at(14),
        )
    }
}

/// A rotation as `(x, y, z, w)`, unit length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quat(pub [f32; 4]);

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Self = Self([0.0, 0.0, 0.0, 1.0]);

    #[must_use]
    pub fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w]).normalized()
    }

    /// The shortest rotation from `from` to `to`, both unit length.
    #[must_use]
    pub fn from_vectors(from: Vec3, to: Vec3) -> Self {
        let dot = from.x * to.x + from.y * to.y + from.z * to.z;
        if dot < -0.999_999 {
            // Opposite: any perpendicular axis is a valid 180 degree turn.
            let axis = if from.x.abs() < 0.9 {
                Vec3::new(0.0, -from.z, from.y)
            } else {
                Vec3::new(-from.z, 0.0, from.x)
            };
            return Self::from_axis_angle(axis, std::f32::consts::PI);
        }
        let cross = Vec3::new(
            from.y * to.z - from.z * to.y,
            from.z * to.x - from.x * to.z,
            from.x * to.y - from.y * to.x,
        );
        Self::from_xyzw(cross.x, cross.y, cross.z, 1.0 + dot)
    }

    /// A rotation of `angle` radians around `axis`.
    #[must_use]
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let scaled = axis.normalized_or_zero();
        if scaled.length_squared() == 0.0 {
            return Self::IDENTITY;
        }
        let half = angle * 0.5;
        let sine = half.sin();
        Self([
            scaled.x * sine,
            scaled.y * sine,
            scaled.z * sine,
            half.cos(),
        ])
        .normalized()
    }

    /// Normalised, or identity when the input is degenerate (a hostile file may send zeros).
    #[must_use]
    pub fn normalized(self) -> Self {
        let [x, y, z, w] = self.0;
        let length = (x * x + y * y + z * z + w * w).sqrt();
        if !length.is_finite() || length < 1e-8 {
            return Self::IDENTITY;
        }
        Self([x / length, y / length, z / length, w / length])
    }

    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.0[0] * other.0[0]
            + self.0[1] * other.0[1]
            + self.0[2] * other.0[2]
            + self.0[3] * other.0[3]
    }

    /// Spherical interpolation. The shortest way around is chosen (negating one end when needed),
    /// and a near-zero angle falls back to a straight mix, so the result never divides by zero.
    #[must_use]
    pub fn slerp(self, other: Self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let mut end = other.normalized();
        let mut cosine = self.normalized().dot(end);
        if cosine < 0.0 {
            end = Self([-end.0[0], -end.0[1], -end.0[2], -end.0[3]]);
            cosine = -cosine;
        }
        if cosine > 0.999_5 {
            let start = self.0;
            let finish = end.0;
            return Self([
                start[0] + (finish[0] - start[0]) * amount,
                start[1] + (finish[1] - start[1]) * amount,
                start[2] + (finish[2] - start[2]) * amount,
                start[3] + (finish[3] - start[3]) * amount,
            ])
            .normalized();
        }
        let theta = cosine.clamp(-1.0, 1.0).acos();
        let sine = theta.sin();
        let weight_start = ((1.0 - amount) * theta).sin() / sine;
        let weight_end = (amount * theta).sin() / sine;
        let start = self.0;
        Self([
            start[0] * weight_start + end.0[0] * weight_end,
            start[1] * weight_start + end.0[1] * weight_end,
            start[2] * weight_start + end.0[2] * weight_end,
            start[3] * weight_start + end.0[3] * weight_end,
        ])
        .normalized()
    }
}
