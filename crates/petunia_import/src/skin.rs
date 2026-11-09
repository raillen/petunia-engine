//! Skin weights per vertex, read from a glTF primitive.
//!
//! The welded mesh the importer builds is shared with the modeling tools, so the skin data rides
//! beside it instead of inside `PolygonMesh`: `ImportSkin::weights` has one entry per vertex, in the
//! same order as `ImportedNode::mesh.positions`.
//!
//! Welding merges vertices that share a position, and two such vertices may have **different**
//! weights (a seam between two limbs). The first one wins and is recorded as a warning, so a file is
//! never silently mis-skinned.

use petunia_anim::MAX_JOINTS;
use petunia_math::Vec3;

/// How many joints may influence one vertex. glTF's `JOINTS_0`/`WEIGHTS_0` carry four.
pub const INFLUENCES: usize = 4;

/// One vertex's share of up to four joints. Weights are normalised on read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexWeights {
    pub joints: [u16; INFLUENCES],
    pub weights: [f32; INFLUENCES],
}

/// The skin of one imported mesh node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportSkin {
    /// The joint indices of the skin this node uses.
    pub skeleton: usize,
    /// One entry per **welded** vertex, in `positions` order. A vertex with no influence (all
    /// weights zero) moves with the node, which is what an unskinned part should do.
    pub weights: Vec<VertexWeights>,
}

impl VertexWeights {
    /// Reads one vertex from the primitive's `JOINTS_n`/`WEIGHTS_n`, refusing out-of-range joints.
    pub(crate) fn read(joints: [u16; INFLUENCES], weights: [f32; INFLUENCES]) -> Option<Self> {
        // An index past the joint limit cannot be honoured; the caller records it as "no file
        // export should do this" and the vertex stays with the node.
        if joints.iter().any(|joint| usize::from(*joint) >= MAX_JOINTS) {
            return None;
        }
        let mut total = 0.0f32;
        let mut clean = [0.0f32; INFLUENCES];
        for (slot, weight) in weights.iter().enumerate() {
            let value = if weight.is_finite() && *weight > 0.0 {
                *weight
            } else {
                0.0
            };
            if let Some(at) = clean.get_mut(slot) {
                *at = value;
            }
            total += value;
        }
        // A vertex whose weights do not sum to something sensible has no influence: it stays with
        // the node instead of being flung to the origin by a division by zero.
        if total <= 1e-6 {
            return Some(Self::default());
        }
        let mut normalised = [0.0f32; INFLUENCES];
        for (slot, value) in clean.iter().enumerate() {
            if let Some(at) = normalised.get_mut(slot) {
                *at = value / total;
            }
        }
        Some(Self {
            joints: [
                joints.first().copied().unwrap_or(0),
                joints.get(1).copied().unwrap_or(0),
                joints.get(2).copied().unwrap_or(0),
                joints.get(3).copied().unwrap_or(0),
            ],
            weights: normalised,
        })
    }

    /// The position of this vertex after the skin moves: a weighted sum of each joint's matrix.
    #[must_use]
    pub fn transform(&self, position: Vec3, matrices: &[petunia_anim::Mat4]) -> Vec3 {
        let mut moved = Vec3::ZERO;
        let mut total = 0.0f32;
        for (slot, weight) in self.weights.iter().enumerate() {
            if *weight <= 0.0 {
                continue;
            }
            let joint = usize::from(self.joints.get(slot).copied().unwrap_or(0));
            let Some(matrix) = matrices.get(joint) else {
                continue;
            };
            let point = matrix.transform_point(position);
            moved = Vec3::new(
                moved.x + point.x * weight,
                moved.y + point.y * weight,
                moved.z + point.z * weight,
            );
            total += weight;
        }
        if total <= 1e-6 {
            return position;
        }
        Vec3::new(moved.x / total, moved.y / total, moved.z / total)
    }
}
