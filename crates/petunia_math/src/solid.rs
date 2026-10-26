//! The solid form of an object for collision: the points and triangles of what is drawn, and the
//! box around them. Plain data, so the renderer can produce it and the physics can use it without
//! knowing each other.

use crate::Vec3;

/// Most triangles one solid keeps; a bigger surface is refused by the code that builds it.
pub const MAX_SOLID_TRIANGLES: usize = 500_000;

/// An object's surface in its own coordinates (meters, before its `Transform`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidShape {
    pub points: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// The box around the points.
    pub low: Vec3,
    pub high: Vec3,
    /// The surface is exactly a box (a `box` brush), so a box collider is enough.
    pub is_box: bool,
}

impl SolidShape {
    /// Builds a solid from vertex positions and a triangle index list. Returns `None` for an
    /// empty or oversized surface, a broken index list or a non-finite point.
    pub fn from_triangles(positions: &[[f32; 3]], indices: &[u32], is_box: bool) -> Option<Self> {
        if positions.is_empty()
            || indices.len() < 3
            || !indices.len().is_multiple_of(3)
            || indices.len() / 3 > MAX_SOLID_TRIANGLES
        {
            return None;
        }
        let mut points = Vec::with_capacity(positions.len());
        let mut low = Vec3::splat(f32::MAX);
        let mut high = Vec3::splat(f32::MIN);
        for [x, y, z] in positions {
            let point = Vec3::new(*x, *y, *z);
            if !point.is_finite() {
                return None;
            }
            low = Vec3::new(low.x.min(point.x), low.y.min(point.y), low.z.min(point.z));
            high = Vec3::new(
                high.x.max(point.x),
                high.y.max(point.y),
                high.z.max(point.z),
            );
            points.push(point);
        }
        let count = u32::try_from(points.len()).ok()?;
        let mut triangles = Vec::with_capacity(indices.len() / 3);
        let (corners, _) = indices.as_chunks::<3>();
        for [a, b, c] in corners {
            if *a >= count || *b >= count || *c >= count {
                return None;
            }
            triangles.push([*a, *b, *c]);
        }
        Some(Self {
            points,
            triangles,
            low,
            high,
            is_box,
        })
    }

    /// Several surfaces joined into one (the chunks of a sculpted cave).
    pub fn merged(parts: &[(&[[f32; 3]], &[u32])]) -> Option<Self> {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for (part_positions, part_indices) in parts {
            let offset = u32::try_from(positions.len()).ok()?;
            positions.extend_from_slice(part_positions);
            indices.extend(part_indices.iter().map(|index| index + offset));
        }
        Self::from_triangles(&positions, &indices, false)
    }

    pub fn size(&self) -> Vec3 {
        self.high - self.low
    }

    pub fn center(&self) -> Vec3 {
        (self.low + self.high) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solid_keeps_its_box_and_refuses_broken_input() {
        let positions = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 1.0]];
        let solid = SolidShape::from_triangles(&positions, &[0, 1, 2], false).expect("valid");
        assert_eq!(solid.size(), Vec3::new(2.0, 3.0, 1.0));
        assert_eq!(solid.center(), Vec3::new(1.0, 1.5, 0.5));
        assert!(
            SolidShape::from_triangles(&positions, &[0, 1, 3], false).is_none(),
            "index out of range"
        );
        assert!(
            SolidShape::from_triangles(&positions, &[0, 1], false).is_none(),
            "not whole triangles"
        );
        assert!(SolidShape::from_triangles(&[], &[], false).is_none());
        assert!(SolidShape::from_triangles(&[[f32::NAN, 0.0, 0.0]], &[0, 0, 0], false).is_none());
    }

    #[test]
    fn merging_shifts_the_second_surface_indices() {
        let a = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let b = [[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [5.0, 1.0, 0.0]];
        let solid = SolidShape::merged(&[(&a, &[0, 1, 2]), (&b, &[0, 1, 2])]).expect("valid");
        assert_eq!(solid.triangles, vec![[0, 1, 2], [3, 4, 5]]);
        assert_eq!(solid.high.x, 6.0);
    }
}
