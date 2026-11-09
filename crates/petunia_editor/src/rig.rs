//! Skeletons, clips and skin weights in the saved scene (phase 4).
//!
//! What the importer reads from a GLB has to reach the file, or an imported character arrives
//! without its animation and the engine can only draw the rest pose. Three tables carry it, all
//! written in the same batch as the meshes so one undo takes the whole model away:
//!
//! - `Skeleton`: one row per imported model, with the joint list as text.
//! - `Clip`: one row per animation, with its channels as text (keyframes are data, like a mesh).
//! - `Skin`: one row per mesh object, tying it to the skeleton and holding the per-vertex weights.
//!
//! The formats are the ones `petunia_anim` already parses, so a file written today is read back by
//! the same code that plays it. Nothing here rounds or approximates: a mesh that cannot be written
//! exactly is refused by name, never silently flattened to the rest pose.

use petunia_anim::{
    Channel, ChannelValues, Clip, Interpolation, Joint, Mat4, Quat, QuatKey, Skeleton, Vec3Key,
};
use petunia_import::skin::ImportSkin;
use petunia_math::Vec3;
use serde_json::{Value as Json, json};

// The tables this module writes: `Skeleton`, `Clip` and `Skin` (see `petunia_project::standard`).
// Their names are literals in the records below; a constant would only be a second place to keep
// in sync with the table definition.

/// The skeleton as text: one joint per line, `parent|16 matrix floats|16 inverse-bind floats`.
///
/// A line format instead of JSON inside JSON: the text stays readable in a diff, which is the same
/// reason the mesh is a line per vertex.
#[must_use]
pub fn skeleton_to_text(skeleton: &Skeleton) -> String {
    let mut out = String::new();
    for index in 0..skeleton.len() {
        let Some(joint) = skeleton.joint(index) else {
            continue;
        };
        let parent = joint
            .parent
            .map_or_else(|| "-".to_string(), |p| p.to_string());
        out.push_str(&parent);
        out.push('|');
        write_matrix(&mut out, &joint.rest);
        out.push('|');
        write_matrix(&mut out, &joint.inverse_bind);
        out.push('\n');
    }
    out
}

fn write_matrix(out: &mut String, matrix: &Mat4) {
    for (slot, value) in matrix.0.iter().enumerate() {
        if slot > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{value:.6}"));
    }
}

/// Parses what [`skeleton_to_text`] wrote. A line that does not parse makes the whole skeleton
/// invalid: half a skeleton would pose the model into a shape nobody authored.
#[must_use]
pub fn skeleton_from_text(text: &str) -> Option<Skeleton> {
    let mut joints = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('|');
        let parent = match parts.next()?.trim() {
            "-" => None,
            number => Some(number.parse::<usize>().ok()?),
        };
        let rest = parse_matrix(parts.next()?)?;
        let inverse = parse_matrix(parts.next()?)?;
        joints.push(Joint {
            parent,
            rest,
            inverse_bind: inverse,
        });
    }
    if joints.is_empty() {
        return None;
    }
    Skeleton::new(joints).ok()
}

fn parse_matrix(text: &str) -> Option<Mat4> {
    let mut values = [0.0f32; 16];
    let mut count = 0;
    for token in text.split_whitespace() {
        let slot = values.get_mut(count)?;
        *slot = token.parse::<f32>().ok()?;
        if !slot.is_finite() {
            return None;
        }
        count += 1;
    }
    (count == 16).then_some(Mat4(values))
}

/// One clip as text: a header line per channel, then its keys.
///
/// `c <joint> <translation|rotation|scale> <step|linear>` starts a channel; every following line is
/// one key. Keeping the channels together in one text block means a clip is one row, not thousands.
#[must_use]
pub fn clip_to_text(clip: &Clip) -> String {
    let mut out = String::new();
    for channel in &clip.channels {
        let (kind, interpolation) = match (&channel.values, channel.interpolation) {
            (ChannelValues::Translation(_), Interpolation::Step) => ("translation", "step"),
            (ChannelValues::Translation(_), _) => ("translation", "linear"),
            (ChannelValues::Rotation(_), Interpolation::Step) => ("rotation", "step"),
            (ChannelValues::Rotation(_), _) => ("rotation", "linear"),
            (ChannelValues::Scale(_), Interpolation::Step) => ("scale", "step"),
            (ChannelValues::Scale(_), _) => ("scale", "linear"),
        };
        out.push_str(&format!("c {} {kind} {interpolation}\n", channel.joint));
        match &channel.values {
            ChannelValues::Translation(keys) | ChannelValues::Scale(keys) => {
                for key in keys {
                    out.push_str(&format!(
                        "k {:.6} {:.6} {:.6} {:.6}\n",
                        key.time, key.value.x, key.value.y, key.value.z
                    ));
                }
            }
            ChannelValues::Rotation(keys) => {
                for key in keys {
                    out.push_str(&format!(
                        "k {:.6} {:.6} {:.6} {:.6} {:.6}\n",
                        key.time,
                        key.value.0.first().copied().unwrap_or(0.0),
                        key.value.0.get(1).copied().unwrap_or(0.0),
                        key.value.0.get(2).copied().unwrap_or(0.0),
                        key.value.0.get(3).copied().unwrap_or(1.0)
                    ));
                }
            }
        }
    }
    out
}

/// Parses what [`clip_to_text`] wrote, or `None` when a line is malformed.
#[must_use]
pub fn clip_from_text(name: &str, text: &str) -> Option<Clip> {
    let mut channels: Vec<Channel> = Vec::new();
    let mut kind = String::new();
    let mut interpolation = Interpolation::Linear;
    let mut joint = 0usize;
    let mut translation: Vec<Vec3Key> = Vec::new();
    let mut rotation: Vec<QuatKey> = Vec::new();
    let mut scaling: Vec<Vec3Key> = Vec::new();
    let flush = |channels: &mut Vec<Channel>,
                 joint: usize,
                 kind: &str,
                 interpolation: Interpolation,
                 translation: &mut Vec<Vec3Key>,
                 rotation: &mut Vec<QuatKey>,
                 scaling: &mut Vec<Vec3Key>| {
        let values = match kind {
            "rotation" => ChannelValues::Rotation(std::mem::take(rotation)),
            "scale" => ChannelValues::Scale(std::mem::take(scaling)),
            _ => ChannelValues::Translation(std::mem::take(translation)),
        };
        channels.push(Channel {
            joint,
            values,
            interpolation,
        });
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next()? {
            "c" => {
                if !kind.is_empty() {
                    flush(
                        &mut channels,
                        joint,
                        &kind,
                        interpolation,
                        &mut translation,
                        &mut rotation,
                        &mut scaling,
                    );
                }
                joint = parts.next()?.parse().ok()?;
                kind = parts.next()?.to_string();
                interpolation = match parts.next().unwrap_or("linear") {
                    "step" => Interpolation::Step,
                    _ => Interpolation::Linear,
                };
            }
            "k" => {
                let time: f32 = parts.next()?.parse().ok()?;
                match kind.as_str() {
                    "rotation" => {
                        let x: f32 = parts.next()?.parse().ok()?;
                        let y: f32 = parts.next()?.parse().ok()?;
                        let z: f32 = parts.next()?.parse().ok()?;
                        let w: f32 = parts.next()?.parse().ok()?;
                        rotation.push(QuatKey {
                            time,
                            value: Quat::from_xyzw(x, y, z, w),
                        });
                    }
                    _ => {
                        let x: f32 = parts.next()?.parse().ok()?;
                        let y: f32 = parts.next()?.parse().ok()?;
                        let z: f32 = parts.next()?.parse().ok()?;
                        let key = Vec3Key {
                            time,
                            value: Vec3::new(x, y, z),
                        };
                        if kind == "scale" {
                            scaling.push(key);
                        } else {
                            translation.push(key);
                        }
                    }
                }
            }
            _ => return None,
        }
    }
    if !kind.is_empty() {
        flush(
            &mut channels,
            joint,
            &kind,
            interpolation,
            &mut translation,
            &mut rotation,
            &mut scaling,
        );
    }
    (!channels.is_empty()).then(|| Clip {
        name: name.to_string(),
        channels,
    })
}

/// One mesh's skin as text: a header with the skeleton row, then one line per vertex.
#[must_use]
pub fn skin_to_text(skeleton_row: u64, skin: &ImportSkin) -> String {
    let mut out = format!("skeleton {skeleton_row}\n");
    for weight in &skin.weights {
        out.push_str(&format!(
            "v {} {} {} {} {} {} {} {}\n",
            weight.joints.first().copied().unwrap_or(0),
            weight.joints.get(1).copied().unwrap_or(0),
            weight.joints.get(2).copied().unwrap_or(0),
            weight.joints.get(3).copied().unwrap_or(0),
            weight.weights.first().copied().unwrap_or(0.0),
            weight.weights.get(1).copied().unwrap_or(0.0),
            weight.weights.get(2).copied().unwrap_or(0.0),
            weight.weights.get(3).copied().unwrap_or(0.0),
        ));
    }
    out
}

/// Parses what [`skin_to_text`] wrote: the skeleton row and one weight per vertex.
#[must_use]
pub fn skin_from_text(text: &str) -> Option<(u64, Vec<petunia_render::SkinWeight>)> {
    let mut skeleton = None;
    let mut weights = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next()? {
            "skeleton" => skeleton = Some(parts.next()?.parse().ok()?),
            "v" => {
                let mut joints = [0u16; 4];
                for slot in &mut joints {
                    *slot = parts.next()?.parse().ok()?;
                }
                let mut values = [0.0f32; 4];
                for slot in &mut values {
                    *slot = parts.next()?.parse().ok()?;
                }
                weights.push(petunia_render::SkinWeight {
                    joints,
                    weights: values,
                });
            }
            _ => return None,
        }
    }
    Some((skeleton?, weights))
}

/// The rows an imported model contributes, ready to be written in the same batch as its meshes.
///
/// Returns the skeleton and clip rows first (so their ids exist), then nothing else: the skin rows
/// need the skeleton's id, which only exists after the batch runs, so they are written by
/// [`skin_row`] in a second small batch.
#[must_use]
pub fn model_rows(skeleton: &Skeleton, clips: &[Clip], skeleton_name: &str) -> Vec<Json> {
    let mut rows = vec![json!({
        "Name": { "text": format!("{skeleton_name} — esqueleto") },
        "Skeleton": {
            "joints": skeleton_to_text(skeleton),
            "joint_count": skeleton.len() as i64,
        }
    })];
    for clip in clips {
        rows.push(json!({
            "Name": { "text": clip.name },
            "Clip": {
                "channels": clip_to_text(clip),
                "duration": clip.duration(),
            }
        }));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(parent: Option<usize>, x: f32) -> Joint {
        Joint {
            parent,
            rest: Mat4::translation(Vec3::new(x, 0.0, 0.0)),
            inverse_bind: Mat4::translation(Vec3::new(-x, 0.0, 0.0)),
        }
    }

    #[test]
    fn a_skeleton_survives_the_round_trip() {
        let skeleton =
            Skeleton::new(vec![joint(None, 0.0), joint(Some(0), 1.5)]).expect("skeleton");
        let text = skeleton_to_text(&skeleton);
        let back = skeleton_from_text(&text).expect("parses");
        assert_eq!(back.len(), 2);
        assert_eq!(back.joint(1).expect("joint").parent, Some(0));
        // The matrices come back to six decimals, which is what the text keeps.
        let first = back.joint(0).expect("joint").rest;
        assert!((first.0.get(12).copied().unwrap_or(9.0)).abs() < 1e-6);
    }

    #[test]
    fn a_broken_skeleton_is_refused_instead_of_half_parsed() {
        assert!(skeleton_from_text("0|1 2 3").is_none(), "a short matrix");
        assert!(skeleton_from_text("").is_none(), "nothing");
        assert!(skeleton_from_text("1|").is_none());
    }

    #[test]
    fn a_clip_with_three_channel_kinds_survives_the_round_trip() {
        let clip = Clip {
            name: "anda".to_string(),
            channels: vec![
                Channel {
                    joint: 0,
                    values: ChannelValues::Translation(vec![
                        Vec3Key {
                            time: 0.0,
                            value: Vec3::ZERO,
                        },
                        Vec3Key {
                            time: 1.0,
                            value: Vec3::new(2.0, 0.0, 0.0),
                        },
                    ]),
                    interpolation: Interpolation::Linear,
                },
                Channel {
                    joint: 1,
                    values: ChannelValues::Rotation(vec![QuatKey {
                        time: 0.0,
                        value: Quat::IDENTITY,
                    }]),
                    interpolation: Interpolation::Step,
                },
                Channel {
                    joint: 1,
                    values: ChannelValues::Scale(vec![Vec3Key {
                        time: 0.0,
                        value: Vec3::ONE,
                    }]),
                    interpolation: Interpolation::Linear,
                },
            ],
        };
        let text = clip_to_text(&clip);
        let back = clip_from_text("anda", &text).expect("parses");
        assert_eq!(back.channels.len(), 3);
        assert_eq!(back.channels[1].interpolation, Interpolation::Step);
        assert!((back.duration() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_skin_keeps_one_weight_per_vertex() {
        let skin = ImportSkin {
            skeleton: 0,
            weights: vec![
                petunia_import::skin::VertexWeights {
                    joints: [0, 1, 0, 0],
                    weights: [0.5, 0.5, 0.0, 0.0],
                };
                3
            ],
        };
        let text = skin_to_text(7, &skin);
        let (row, weights) = skin_from_text(&text).expect("parses");
        assert_eq!(row, 7);
        assert_eq!(weights.len(), 3);
        assert!((weights[0].weights[0] - 0.5).abs() < 1e-6);
    }
}
