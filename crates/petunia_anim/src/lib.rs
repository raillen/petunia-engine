//! Skeletons, clips and keyframe sampling (phase 4: skeletal animation playback).
//!
//! What this crate does: hold a **skeleton** (a tree of joints with rest transforms and inverse
//! bind matrices), hold **clips** (channels of keyframes per joint), and answer one question —
//! "where is every joint at time `t`?" — plus the skinning matrices that go with it.
//!
//! What it does not do: authoring (no rig editor, no auto-rig: `DR-23` says import from GLB), and
//! reading files (the glTF reader lives in `petunia_import`).
//!
//! Determinism matters here (`AGENTS.md` §2): sampling uses integer-indexed keys and an explicit
//! interpolation per segment, so the same clip and the same time give the same pose on every
//! machine. There is no dependency and no floating-point tie-break.

pub mod math;

pub use crate::math::{Mat4, Quat};
use petunia_math::Vec3;

/// Most joints a skeleton may have. A hostile file must not allocate without limit.
pub const MAX_JOINTS: usize = 2_048;
/// Most keyframes a channel may have.
pub const MAX_KEYS: usize = 65_536;
/// Most channels a clip may have.
pub const MAX_CHANNELS: usize = 8_192;

/// How a channel interpolates between two keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Interpolation {
    /// Hold the previous key until the next one. The safest default: no invented motion.
    #[default]
    Step,
    /// Straight line between keys.
    Linear,
    /// Straight line across the whole channel, ignoring intermediate keys (glTF `CUBICSPLINE`
    /// carries extra tangents this crate does not use yet; it is refused at import, not guessed).
    LinearAll,
}

/// One keyframe of a translation or scale channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3Key {
    pub time: f32,
    pub value: Vec3,
}

/// One keyframe of a rotation channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuatKey {
    pub time: f32,
    pub value: Quat,
}

/// What one channel drives.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelValues {
    Translation(Vec<Vec3Key>),
    Rotation(Vec<QuatKey>),
    Scale(Vec<Vec3Key>),
}

impl ChannelValues {
    #[must_use]
    pub fn key_count(&self) -> usize {
        match self {
            Self::Translation(keys) | Self::Scale(keys) => keys.len(),
            Self::Rotation(keys) => keys.len(),
        }
    }

    fn first_time(&self) -> f32 {
        match self {
            Self::Translation(keys) | Self::Scale(keys) => {
                keys.first().map(|key| key.time).unwrap_or(0.0)
            }
            Self::Rotation(keys) => keys.first().map(|key| key.time).unwrap_or(0.0),
        }
    }

    fn last_time(&self) -> f32 {
        match self {
            Self::Translation(keys) | Self::Scale(keys) => {
                keys.last().map(|key| key.time).unwrap_or(0.0)
            }
            Self::Rotation(keys) => keys.last().map(|key| key.time).unwrap_or(0.0),
        }
    }
}

/// One channel: which joint, what it drives, and its keys.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub joint: usize,
    pub values: ChannelValues,
    pub interpolation: Interpolation,
}

/// A named clip over a skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    pub channels: Vec<Channel>,
}

impl Clip {
    /// The length of the clip: the latest key across its channels.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.channels
            .iter()
            .map(|channel| channel.values.last_time())
            .fold(0.0, f32::max)
    }

    /// The time of the first key: a clip does not have to start at zero.
    #[must_use]
    pub fn start(&self) -> f32 {
        self.channels
            .iter()
            .map(|channel| channel.values.first_time())
            .fold(f32::MAX, f32::min)
    }
}

/// One joint: its parent, its rest transform and its inverse bind matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    /// Parent index, or `None` for a root. A joint may only point at a **lower** index, so the tree
    /// cannot contain a cycle by construction.
    pub parent: Option<usize>,
    pub rest: Mat4,
    pub inverse_bind: Mat4,
}

/// A skeleton: joints in parents-before-children order.
#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    joints: Vec<Joint>,
}

/// Why a skeleton or a clip was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimError {
    /// More joints, keys or channels than the limits allow.
    TooLarge,
    /// A joint points at itself or at a later joint (the tree would not be a tree).
    BadParent(usize),
    /// A channel names a joint the skeleton does not have.
    UnknownJoint(usize),
    /// A key list is empty, or its times do not increase.
    BadKeys(usize),
}

impl std::fmt::Display for AnimError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("o esqueleto ou o clipe passa do limite"),
            Self::BadParent(joint) => {
                write!(formatter, "a junta {joint} aponta para um pai inválido")
            }
            Self::UnknownJoint(joint) => {
                write!(formatter, "o canal usa a junta {joint}, que não existe")
            }
            Self::BadKeys(joint) => {
                write!(
                    formatter,
                    "as chaves da junta {joint} estão vazias ou fora de ordem"
                )
            }
        }
    }
}

impl Skeleton {
    /// Builds a skeleton, refusing a parent that is not strictly lower than the joint itself.
    pub fn new(joints: Vec<Joint>) -> Result<Self, AnimError> {
        if joints.len() > MAX_JOINTS {
            return Err(AnimError::TooLarge);
        }
        for (index, joint) in joints.iter().enumerate() {
            if let Some(parent) = joint.parent
                && parent >= index
            {
                return Err(AnimError::BadParent(index));
            }
        }
        Ok(Self { joints })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.joints.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.joints.is_empty()
    }

    #[must_use]
    pub fn joint(&self, index: usize) -> Option<&Joint> {
        self.joints.get(index)
    }

    /// The pose at a time: each joint's model matrix, local rest overridden by the clip.
    ///
    /// `blend` mixes the clip with the rest pose (1.0 is the clip alone), so a character can fade
    /// into an idle without a second skeleton.
    #[must_use]
    pub fn pose(&self, clip: &Clip, time: f32, blend: f32) -> Vec<Mat4> {
        let blend = blend.clamp(0.0, 1.0);
        // Local transform per joint: the rest matrix, replaced where the clip has a channel.
        let mut local: Vec<Mat4> = self.joints.iter().map(|joint| joint.rest).collect();
        for channel in &clip.channels {
            if let Some(slot) = local.get_mut(channel.joint) {
                *slot = channel.values.sample(*slot, time, channel.interpolation);
            }
        }
        if blend < 1.0 {
            for (slot, joint) in local.iter_mut().zip(&self.joints) {
                *slot = mix_matrix(&joint.rest, slot, blend);
            }
        }
        // World matrices: parents come first by construction, so one pass is enough.
        let mut world: Vec<Mat4> = Vec::with_capacity(self.joints.len());
        for (index, joint) in self.joints.iter().enumerate() {
            let local_matrix = local.get(index).copied().unwrap_or(Mat4::IDENTITY);
            world.push(match joint.parent {
                Some(parent) => world
                    .get(parent)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY)
                    .multiply(&local_matrix),
                None => local_matrix,
            });
        }
        world
    }

    /// The matrices a renderer uses to move a vertex: `world * inverse_bind`, one per joint.
    ///
    /// A joint whose `inverse_bind` is absent (identity) still works: it simply moves with the world.
    #[must_use]
    pub fn skinning_matrices(&self, clip: &Clip, time: f32, blend: f32) -> Vec<Mat4> {
        self.pose(clip, time, blend)
            .iter()
            .zip(&self.joints)
            .map(|(world, joint)| world.multiply(&joint.inverse_bind))
            .collect()
    }
}

impl ChannelValues {
    /// Samples the channel at `time`, using `current` as the value when the clip has no key before
    /// the start (so a clip can be played from its own first key without snapping to zero).
    fn sample(&self, current: Mat4, time: f32, interpolation: Interpolation) -> Mat4 {
        match self {
            Self::Translation(keys) => {
                let value = sample_vec3(keys, time, interpolation, translation_of(current));
                Mat4::from_trs(value, rotation_of(current), scale_of(current))
            }
            Self::Scale(keys) => {
                let value = sample_vec3(keys, time, interpolation, scale_of(current));
                Mat4::from_trs(translation_of(current), rotation_of(current), value)
            }
            Self::Rotation(keys) => {
                let value = sample_quat(keys, time, interpolation, rotation_of(current));
                Mat4::from_trs(translation_of(current), value, scale_of(current))
            }
        }
    }
}

/// The segment index for `time`: the last key at or before it. `None` when time is before all keys.
fn segment(times: impl Iterator<Item = f32> + Clone, count: usize, time: f32) -> Option<usize> {
    let mut found = None;
    for (index, key) in times.enumerate().take(count) {
        if key <= time {
            found = Some(index);
        } else {
            break;
        }
    }
    found
}

fn fraction(start: f32, end: f32, time: f32) -> f32 {
    let span = end - start;
    if !span.is_finite() || span.abs() < 1e-9 {
        0.0
    } else {
        ((time - start) / span).clamp(0.0, 1.0)
    }
}

fn sample_vec3(keys: &[Vec3Key], time: f32, interpolation: Interpolation, fallback: Vec3) -> Vec3 {
    let Some(index) = segment(keys.iter().map(|key| key.time), keys.len(), time) else {
        return keys.first().map(|key| key.value).unwrap_or(fallback);
    };
    let Some(key) = keys.get(index) else {
        return fallback;
    };
    if interpolation == Interpolation::Step || index + 1 >= keys.len() {
        return key.value;
    }
    let Some(next) = keys.get(index + 1) else {
        return key.value;
    };
    let amount = fraction(key.time, next.time, time);
    let start = key.value;
    let finish = next.value;
    Vec3::new(
        start.x + (finish.x - start.x) * amount,
        start.y + (finish.y - start.y) * amount,
        start.z + (finish.z - start.z) * amount,
    )
}

fn sample_quat(keys: &[QuatKey], time: f32, interpolation: Interpolation, fallback: Quat) -> Quat {
    let Some(index) = segment(keys.iter().map(|key| key.time), keys.len(), time) else {
        return keys.first().map(|key| key.value).unwrap_or(fallback);
    };
    let Some(key) = keys.get(index) else {
        return fallback;
    };
    if interpolation == Interpolation::Step || index + 1 >= keys.len() {
        return key.value;
    }
    let Some(next) = keys.get(index + 1) else {
        return key.value;
    };
    let amount = fraction(key.time, next.time, time);
    key.value.slerp(next.value, amount)
}

/// Straight mix of two matrices, used to fade a clip into the rest pose.
fn mix_matrix(rest: &Mat4, pose: &Mat4, amount: f32) -> Mat4 {
    let mut out = [0.0f32; 16];
    for ((slot, start), finish) in out.iter_mut().zip(rest.0.iter()).zip(pose.0.iter()) {
        *slot = start + (finish - start) * amount;
    }
    Mat4(out)
}

fn translation_of(matrix: Mat4) -> Vec3 {
    let at = |index: usize| matrix.0.get(index).copied().unwrap_or(0.0);
    Vec3::new(at(12), at(13), at(14))
}

fn scale_of(matrix: Mat4) -> Vec3 {
    let at = |index: usize| matrix.0.get(index).copied().unwrap_or(0.0);
    let column = |offset: usize| Vec3::new(at(offset), at(offset + 1), at(offset + 2)).length();
    Vec3::new(column(0), column(4), column(8))
}

fn rotation_of(matrix: Mat4) -> Quat {
    // Rebuild the rotation from the normalised basis, so a scaled rest matrix still yields a
    // rotation. A degenerate matrix gives identity.
    let at = |index: usize| matrix.0.get(index).copied().unwrap_or(0.0);
    let axis = |offset: usize| {
        let vector = Vec3::new(at(offset), at(offset + 1), at(offset + 2));
        vector.normalized_or_zero()
    };
    let (x, y, z) = (axis(0), axis(4), axis(8));
    if x.length_squared() == 0.0 || y.length_squared() == 0.0 || z.length_squared() == 0.0 {
        return Quat::IDENTITY;
    }
    // Shepperd's method: pick the largest diagonal term for numerical stability.
    let trace = x.x + y.y + z.z;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quat::from_xyzw((y.z - z.y) / s, (z.x - x.z) / s, (x.y - y.x) / s, 0.25 * s)
    } else if x.x > y.y && x.x > z.z {
        let s = (1.0 + x.x - y.y - z.z).sqrt() * 2.0;
        Quat::from_xyzw(0.25 * s, (y.x + x.y) / s, (z.x + x.z) / s, (y.z - z.y) / s)
    } else if y.y > z.z {
        let s = (1.0 + y.y - x.x - z.z).sqrt() * 2.0;
        Quat::from_xyzw((y.x + x.y) / s, 0.25 * s, (z.y + y.z) / s, (z.x - x.z) / s)
    } else {
        let s = (1.0 + z.z - x.x - y.y).sqrt() * 2.0;
        Quat::from_xyzw((z.x + x.z) / s, (z.y + y.z) / s, 0.25 * s, (x.y - y.x) / s)
    }
}

/// Checks a clip against a skeleton: every channel names a real joint, every key list is ordered.
pub fn validate_clip(clip: &Clip, skeleton: &Skeleton) -> Result<(), AnimError> {
    if clip.channels.len() > MAX_CHANNELS {
        return Err(AnimError::TooLarge);
    }
    for channel in &clip.channels {
        if channel.joint >= skeleton.len() {
            return Err(AnimError::UnknownJoint(channel.joint));
        }
        if channel.values.key_count() == 0 || channel.values.key_count() > MAX_KEYS {
            return Err(AnimError::BadKeys(channel.joint));
        }
        let ordered = match &channel.values {
            ChannelValues::Translation(keys) | ChannelValues::Scale(keys) => {
                keys.windows(2)
                    .all(|pair| match (pair.first(), pair.get(1)) {
                        (Some(first), Some(second)) => first.time <= second.time,
                        _ => true,
                    })
            }
            ChannelValues::Rotation(keys) => {
                keys.windows(2)
                    .all(|pair| match (pair.first(), pair.get(1)) {
                        (Some(first), Some(second)) => first.time <= second.time,
                        _ => true,
                    })
            }
        };
        if !ordered {
            return Err(AnimError::BadKeys(channel.joint));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(parent: Option<usize>, position: Vec3) -> Joint {
        Joint {
            parent,
            rest: Mat4::translation(position),
            inverse_bind: Mat4::translation(Vec3::new(-position.x, -position.y, -position.z)),
        }
    }

    fn two_joints() -> Skeleton {
        Skeleton::new(vec![
            joint(None, Vec3::ZERO),
            joint(Some(0), Vec3::new(0.0, 1.0, 0.0)),
        ])
        .expect("skeleton")
    }

    fn translation_clip(parent_x: f32, child_x: f32, linear: bool) -> Clip {
        let interpolation = if linear {
            Interpolation::Linear
        } else {
            Interpolation::Step
        };
        Clip {
            name: "anda".to_string(),
            channels: vec![
                Channel {
                    joint: 0,
                    values: ChannelValues::Translation(vec![
                        Vec3Key {
                            time: 0.0,
                            value: Vec3::new(0.0, 0.0, 0.0),
                        },
                        Vec3Key {
                            time: 1.0,
                            value: Vec3::new(parent_x, 0.0, 0.0),
                        },
                    ]),
                    interpolation,
                },
                Channel {
                    joint: 1,
                    values: ChannelValues::Translation(vec![
                        Vec3Key {
                            time: 0.0,
                            value: Vec3::new(0.0, 1.0, 0.0),
                        },
                        Vec3Key {
                            time: 1.0,
                            value: Vec3::new(child_x, 1.0, 0.0),
                        },
                    ]),
                    interpolation,
                },
            ],
        }
    }

    #[test]
    fn a_parent_moves_its_child_with_it() {
        let skeleton = two_joints();
        let clip = translation_clip(4.0, 0.0, true);
        // At the end the root is at x=4 and the child keeps its local y=1, so it rides along.
        let world = skeleton.pose(&clip, 1.0, 1.0);
        let child = world.get(1).copied().expect("child");
        assert!(
            (child.0[12] - 4.0).abs() < 1e-5,
            "child x follows the root: {child:?}"
        );
        assert!(
            (child.0[13] - 1.0).abs() < 1e-5,
            "child y is its own: {child:?}"
        );
    }

    #[test]
    fn linear_interpolates_and_step_holds() {
        let skeleton = two_joints();
        let linear = translation_clip(4.0, 0.0, true);
        let halfway = skeleton.pose(&linear, 0.5, 1.0);
        assert!(
            (halfway[0].0[12] - 2.0).abs() < 1e-5,
            "linear reaches the middle: {:?}",
            halfway[0]
        );
        let stepped = translation_clip(4.0, 0.0, false);
        let held = skeleton.pose(&stepped, 0.5, 1.0);
        assert!(
            held[0].0[12].abs() < 1e-6,
            "step keeps the previous key: {:?}",
            held[0]
        );
    }

    #[test]
    fn the_same_clip_and_time_always_give_the_same_pose() {
        // Determinism (AGENTS.md §2): nine runs, byte-identical matrices.
        let skeleton = two_joints();
        let clip = translation_clip(3.5, -1.25, true);
        let first = skeleton.pose(&clip, 0.37, 1.0);
        for _ in 0..8 {
            assert_eq!(skeleton.pose(&clip, 0.37, 1.0), first);
        }
    }

    #[test]
    fn the_duration_is_the_latest_key() {
        let clip = translation_clip(1.0, 1.0, true);
        assert!((clip.duration() - 1.0).abs() < 1e-6);
        assert!(clip.start().abs() < 1e-6);
    }

    #[test]
    fn a_blend_of_zero_is_the_rest_pose() {
        let skeleton = two_joints();
        let clip = translation_clip(9.0, 9.0, true);
        let at_rest = skeleton.pose(&clip, 1.0, 0.0);
        let expected: Vec<Mat4> = [Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)]
            .into_iter()
            .map(Mat4::translation)
            .collect();
        assert_eq!(at_rest, expected);
    }

    #[test]
    fn skinning_puts_a_vertex_back_where_the_bind_pose_had_it() {
        let skeleton = two_joints();
        let clip = translation_clip(0.0, 0.0, true);
        // At the bind pose (time 0, same values) the skinning matrix must be identity for a joint
        // whose inverse bind cancels its rest translation.
        let skinning = skeleton.skinning_matrices(&clip, 0.0, 1.0);
        let point = Vec3::new(1.0, 2.0, 3.0);
        let moved = skinning[1].transform_point(point);
        assert!(
            (moved.x - point.x).abs() < 1e-5
                && (moved.y - point.y).abs() < 1e-5
                && (moved.z - point.z).abs() < 1e-5,
            "bind pose leaves the vertex alone: {moved:?}"
        );
    }

    #[test]
    fn a_cycle_in_the_joint_tree_is_refused() {
        let bad = Skeleton::new(vec![
            Joint {
                parent: Some(1),
                rest: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            },
            Joint {
                parent: Some(0),
                rest: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            },
        ]);
        assert_eq!(bad, Err(AnimError::BadParent(0)));
        let self_parent = Skeleton::new(vec![Joint {
            parent: Some(0),
            rest: Mat4::IDENTITY,
            inverse_bind: Mat4::IDENTITY,
        }]);
        assert_eq!(self_parent, Err(AnimError::BadParent(0)));
    }

    #[test]
    fn a_channel_for_a_joint_that_does_not_exist_is_refused() {
        let skeleton = two_joints();
        let clip = Clip {
            name: "ruim".to_string(),
            channels: vec![Channel {
                joint: 9,
                values: ChannelValues::Translation(vec![Vec3Key {
                    time: 0.0,
                    value: Vec3::ZERO,
                }]),
                interpolation: Interpolation::Linear,
            }],
        };
        assert_eq!(
            validate_clip(&clip, &skeleton),
            Err(AnimError::UnknownJoint(9))
        );
    }

    #[test]
    fn keys_out_of_order_and_empty_keys_are_refused() {
        let skeleton = two_joints();
        let unordered = Clip {
            name: "fora".to_string(),
            channels: vec![Channel {
                joint: 0,
                values: ChannelValues::Translation(vec![
                    Vec3Key {
                        time: 1.0,
                        value: Vec3::ZERO,
                    },
                    Vec3Key {
                        time: 0.5,
                        value: Vec3::ZERO,
                    },
                ]),
                interpolation: Interpolation::Linear,
            }],
        };
        assert_eq!(
            validate_clip(&unordered, &skeleton),
            Err(AnimError::BadKeys(0))
        );

        let empty = Clip {
            name: "vazio".to_string(),
            channels: vec![Channel {
                joint: 0,
                values: ChannelValues::Rotation(Vec::new()),
                interpolation: Interpolation::Step,
            }],
        };
        assert_eq!(validate_clip(&empty, &skeleton), Err(AnimError::BadKeys(0)));
    }

    #[test]
    fn a_hostile_skeleton_past_the_limit_is_refused() {
        let joints = (0..MAX_JOINTS + 1)
            .map(|index| Joint {
                parent: if index == 0 { None } else { Some(index - 1) },
                rest: Mat4::IDENTITY,
                inverse_bind: Mat4::IDENTITY,
            })
            .collect();
        assert_eq!(Skeleton::new(joints), Err(AnimError::TooLarge));
    }
}
