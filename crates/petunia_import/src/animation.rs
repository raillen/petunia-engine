//! Reading skeletons and clips from a glTF document (phase 4).
//!
//! The file is treated as hostile, like everything else `petunia_import` reads: every accessor is
//! checked for bounds and finiteness, joints must be a tree, and a clip that names something this
//! crate cannot play is dropped with a warning instead of panicking. What cannot be represented is
//! **refused by name** (cubic-spline tangents), never approximated in silence.
//!
//! `gltf` 1.4 exposes no generic accessor reader, so this module reads the raw little-endian
//! floats itself, honouring the buffer view's stride.

use std::collections::BTreeMap;

use petunia_anim::{
    Channel, ChannelValues, Clip, Interpolation, Joint, MAX_JOINTS, MAX_KEYS, Mat4, Quat, QuatKey,
    Skeleton, Vec3Key,
};
use petunia_math::Vec3;

use crate::ImportedModel;

/// Reads the first skin and every animation into `model`.
pub(crate) fn read_animation(
    document: &gltf::Document,
    buffers: &[Vec<u8>],
    model: &mut ImportedModel,
) {
    let Some((skeleton, joint_of_node)) = read_skeleton(document, buffers, model) else {
        if document.animations().count() > 0 {
            model.warnings.push(
                "o arquivo tem animações, mas nenhum esqueleto utilizável: elas foram ignoradas"
                    .to_string(),
            );
        }
        return;
    };
    for animation in document.animations() {
        let name = animation
            .name()
            .map(str::to_string)
            .unwrap_or_else(|| format!("clipe {}", animation.index()));
        match read_clip(&animation, buffers, &joint_of_node, &name) {
            Ok(clip) if clip.channels.is_empty() => model.warnings.push(format!(
                "o clipe `{name}` não tem canais que este motor saiba tocar e foi ignorado"
            )),
            Ok(clip) => model.clips.push(clip),
            Err(reason) => model.warnings.push(reason),
        }
    }
    model.skeleton = Some(skeleton);
}

/// The first skin, as a skeleton plus the map from glTF node index to joint index.
fn read_skeleton(
    document: &gltf::Document,
    buffers: &[Vec<u8>],
    model: &mut ImportedModel,
) -> Option<(Skeleton, BTreeMap<usize, usize>)> {
    let skin = document.skins().next()?;
    if document.skins().count() > 1 {
        model
            .warnings
            .push("o arquivo tem mais de um esqueleto; só o primeiro foi lido".to_string());
    }
    let nodes: Vec<gltf::Node> = skin.joints().collect();
    if nodes.len() > MAX_JOINTS {
        model.warnings.push(format!(
            "o esqueleto tem mais de {MAX_JOINTS} juntas e foi ignorado"
        ));
        return None;
    }
    let mut joint_of_node: BTreeMap<usize, usize> = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        joint_of_node.insert(node.index(), index);
    }
    // `gltf` 1.4 has no `parent()` on a node: a joint's parent is the joint listing it as a child.
    let mut parent_of_node: BTreeMap<usize, usize> = BTreeMap::new();
    for node in &nodes {
        for child in node.children() {
            parent_of_node.insert(child.index(), node.index());
        }
    }
    let inverse_binds = read_inverse_binds(&skin, buffers, model);
    let mut joints: Vec<Joint> = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let parent = parent_of_node
            .get(&node.index())
            .and_then(|owner| joint_of_node.get(owner).copied())
            // A parent that comes later would break "parents first", which is what lets the pose be
            // one pass; it becomes a root instead of refusing the whole file.
            .filter(|parent| *parent < index);
        joints.push(Joint {
            parent,
            rest: matrix_of(&node.transform()),
            inverse_bind: inverse_binds.get(index).copied().unwrap_or(Mat4::IDENTITY),
        });
    }
    match Skeleton::new(joints) {
        Ok(skeleton) => Some((skeleton, joint_of_node)),
        Err(error) => {
            model
                .warnings
                .push(format!("o esqueleto foi ignorado: {error}"));
            None
        }
    }
}

/// The inverse bind matrices, when the file carries them.
fn read_inverse_binds(
    skin: &gltf::Skin,
    buffers: &[Vec<u8>],
    model: &mut ImportedModel,
) -> Vec<Mat4> {
    let Some(accessor) = skin.inverse_bind_matrices() else {
        return Vec::new();
    };
    match read_mat4(&accessor, buffers) {
        Some(matrices) => matrices,
        None => {
            model.warnings.push(
                "não consegui ler as matrizes de bind; o esqueleto usa a identidade".to_string(),
            );
            Vec::new()
        }
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// A node transform as a matrix.
fn matrix_of(transform: &gltf::scene::Transform) -> Mat4 {
    match transform {
        gltf::scene::Transform::Matrix { matrix } => {
            // `matrix` is [[f32; 4]; 4] in column-major order; flatten it without indexing.
            let mut flat = [0.0f32; 16];
            for (column, cells) in matrix.iter().enumerate() {
                for (row, value) in cells.iter().enumerate() {
                    if let Some(slot) = flat.get_mut(column * 4 + row) {
                        *slot = finite(*value);
                    }
                }
            }
            Mat4(flat)
        }
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Mat4::from_trs(
            Vec3::new(
                finite(translation[0]),
                finite(translation[1]),
                finite(translation[2]),
            ),
            Quat::from_xyzw(
                finite(rotation[0]),
                finite(rotation[1]),
                finite(rotation[2]),
                finite(rotation[3]),
            ),
            Vec3::new(finite(scale[0]), finite(scale[1]), finite(scale[2])),
        ),
    }
}

/// One animation as a clip, or a sentence saying why it was left out.
fn read_clip(
    animation: &gltf::Animation,
    buffers: &[Vec<u8>],
    joint_of_node: &BTreeMap<usize, usize>,
    name: &str,
) -> Result<Clip, String> {
    let mut channels = Vec::new();
    let mut refused_cubic = false;
    for channel in animation.channels() {
        let target = channel.target();
        if target.property() == gltf::animation::Property::MorphTargetWeights {
            continue;
        }
        let Some(joint) = joint_of_node.get(&target.node().index()).copied() else {
            // A channel for something that is not a joint (a camera, a light) is not ours.
            continue;
        };
        let interpolation = match channel.sampler().interpolation() {
            gltf::animation::Interpolation::Linear => Interpolation::Linear,
            gltf::animation::Interpolation::Step => Interpolation::Step,
            gltf::animation::Interpolation::CubicSpline => {
                refused_cubic = true;
                continue;
            }
        };
        match read_channel(&channel, buffers, joint, interpolation) {
            Ok(Some(channel)) => channels.push(channel),
            Ok(None) => {}
            Err(reason) => return Err(format!("o clipe `{name}` foi ignorado: {reason}")),
        }
    }
    if refused_cubic {
        return Err(format!(
            "o clipe `{name}` usa interpolação cúbica, que ainda não é suportada, e foi ignorado"
        ));
    }
    Ok(Clip {
        name: name.to_string(),
        channels,
    })
}

/// One channel with its keys, or `None` when it cannot be read safely.
fn read_channel(
    channel: &gltf::animation::Channel<'_>,
    buffers: &[Vec<u8>],
    joint: usize,
    interpolation: Interpolation,
) -> Result<Option<Channel>, String> {
    let sampler = channel.sampler();
    let Some(times) = read_f32(&sampler.input(), buffers) else {
        return Ok(None);
    };
    if times.is_empty() || times.len() > MAX_KEYS {
        return Ok(None);
    }
    if times
        .windows(2)
        .any(|pair| match (pair.first(), pair.get(1)) {
            (Some(first), Some(second)) => second < first,
            _ => false,
        })
    {
        return Ok(None);
    }
    let values = match channel.target().property() {
        gltf::animation::Property::Translation => {
            let Some(raw) = read_vec3(&sampler.output(), buffers) else {
                return Ok(None);
            };
            ChannelValues::Translation(map_vec3(&raw, &times))
        }
        gltf::animation::Property::Scale => {
            let Some(raw) = read_vec3(&sampler.output(), buffers) else {
                return Ok(None);
            };
            ChannelValues::Scale(map_vec3(&raw, &times))
        }
        gltf::animation::Property::Rotation => {
            let Some(raw) = read_vec4(&sampler.output(), buffers) else {
                return Ok(None);
            };
            let keys: Vec<QuatKey> = raw
                .iter()
                .zip(times.iter().copied())
                .map(|(value, time)| QuatKey {
                    time,
                    // glTF stores (x, y, z, w).
                    value: Quat::from_xyzw(
                        finite(value[0]),
                        finite(value[1]),
                        finite(value[2]),
                        finite(value[3]),
                    ),
                })
                .collect();
            ChannelValues::Rotation(keys)
        }
        gltf::animation::Property::MorphTargetWeights => return Ok(None),
    };
    Ok(Some(Channel {
        joint,
        values,
        interpolation,
    }))
}

/// The readable bytes of an accessor: `(data, stride, count)`.
fn accessor_bytes<'a>(
    accessor: &gltf::Accessor<'_>,
    buffers: &'a [Vec<u8>],
) -> Option<(&'a [u8], usize, usize)> {
    let view = accessor.view()?;
    let buffer = buffers.get(view.buffer().index())?;
    let start = view.offset() + accessor.offset();
    let stride = view.stride().unwrap_or_else(|| accessor.size());
    let count = accessor.count();
    if stride == 0 || start > buffer.len() {
        return None;
    }
    // The last element plus one element of size must fit, so a truncated file is refused instead
    // of read past its end.
    let last = start.checked_add(stride.checked_mul(count.saturating_sub(1))?)?;
    let end = last.checked_add(accessor.size())?;
    if end > buffer.len() {
        return None;
    }
    let data = buffer.get(start..)?;
    Some((data, stride, count))
}

fn component(bytes: &[u8], offset: usize) -> Option<f32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(f32::from_le_bytes(slice.try_into().ok()?))
}

fn read_f32(accessor: &gltf::Accessor<'_>, buffers: &[Vec<u8>]) -> Option<Vec<f32>> {
    let (data, stride, count) = accessor_bytes(accessor, buffers)?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(component(data, index.checked_mul(stride)?)?);
    }
    Some(values)
}

fn read_vec3(accessor: &gltf::Accessor<'_>, buffers: &[Vec<u8>]) -> Option<Vec<[f32; 3]>> {
    let (data, stride, count) = accessor_bytes(accessor, buffers)?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let base = index.checked_mul(stride)?;
        values.push([
            component(data, base)?,
            component(data, base.checked_add(4)?)?,
            component(data, base.checked_add(8)?)?,
        ]);
    }
    Some(values)
}

fn read_vec4(accessor: &gltf::Accessor<'_>, buffers: &[Vec<u8>]) -> Option<Vec<[f32; 4]>> {
    let (data, stride, count) = accessor_bytes(accessor, buffers)?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let base = index.checked_mul(stride)?;
        values.push([
            component(data, base)?,
            component(data, base.checked_add(4)?)?,
            component(data, base.checked_add(8)?)?,
            component(data, base.checked_add(12)?)?,
        ]);
    }
    Some(values)
}

fn read_mat4(accessor: &gltf::Accessor<'_>, buffers: &[Vec<u8>]) -> Option<Vec<Mat4>> {
    let (data, stride, count) = accessor_bytes(accessor, buffers)?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let base = index.checked_mul(stride)?;
        let mut matrix = [0.0f32; 16];
        for (slot, cell) in matrix.iter_mut().enumerate() {
            *cell = finite(component(data, base.checked_add(slot * 4)?)?);
        }
        values.push(Mat4(matrix));
    }
    Some(values)
}

fn map_vec3(raw: &[[f32; 3]], times: &[f32]) -> Vec<Vec3Key> {
    raw.iter()
        .zip(times.iter().copied())
        .map(|(value, time)| Vec3Key {
            time,
            value: Vec3::new(finite(value[0]), finite(value[1]), finite(value[2])),
        })
        .collect()
}
