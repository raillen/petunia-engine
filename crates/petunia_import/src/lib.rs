//! Reads a GLB or glTF file into editable polygon meshes (DR-29).
//!
//! Only what the editor can use comes through: one mesh per node that has geometry, with the
//! node's place in the scene as a position, a rotation in the engine's Euler degrees and a scale,
//! and the base color of its first material. Textures, animations, skins and cameras are left
//! out and reported in `warnings`. The file is untrusted: sizes, depth and counts are limited
//! before anything is built, and an external file is only read through the caller's `resolve`.

mod animation;
pub mod skin;

use std::collections::HashMap;

use gltf::mesh::Mode;
use petunia_math::{Color, MAX_MESH_VERTICES, PolygonMesh, Vec3};

mod image;

pub use image::{
    ImportedImage, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS, MAX_IMAGE_SIDE, decode_png, read_png_file,
};

/// Largest file the caller should hand over (a limit it applies before reading).
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Nodes visited in one file.
pub const MAX_NODES: usize = 4_096;
/// Depth of the node tree.
pub const MAX_DEPTH: usize = 64;
/// Vertices of all meshes together.
pub const MAX_TOTAL_VERTICES: usize = 2_000_000;
/// Vertices closer than this (meters) are one vertex.
const WELD_STEP: f32 = 1.0e-5;

/// One node of the file that has a mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedNode {
    pub name: String,
    pub position: Vec3,
    /// Euler degrees in the engine's convention (see `petunia_math::Transform`).
    pub rotation_degrees: Vec3,
    pub scale: Vec3,
    /// In the node's own coordinates (before the node's transform).
    pub mesh: PolygonMesh,
    /// Per-vertex joint weights, in `mesh.positions` order, when the file skins this mesh
    /// (`DR-23`, phase 4). `None` means the mesh is not skinned.
    pub skin: Option<crate::skin::ImportSkin>,
    pub color: Color,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedModel {
    pub nodes: Vec<ImportedNode>,
    /// The skeleton the file carries, when it has one (phase 4, `DR-23`).
    pub skeleton: Option<petunia_anim::Skeleton>,
    /// The clips of the file, ready to play.
    pub clips: Vec<petunia_anim::Clip>,
    /// What was left out or changed, one sentence each (for the person to read).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImportError {
    #[error("o arquivo não é um glTF/GLB válido: {0}")]
    Invalid(String),
    #[error("{0}")]
    Refused(String),
}

/// Reads a GLB or glTF document. `resolve` returns the bytes of an external file named by the
/// document (a `.bin`), or `None` when the file may not be read.
pub fn import_gltf(
    bytes: &[u8],
    resolve: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<ImportedModel, ImportError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ImportError::Refused(format!(
            "o arquivo tem mais de {} MB",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let parsed =
        gltf::Gltf::from_slice(bytes).map_err(|error| ImportError::Invalid(error.to_string()))?;
    let document = &parsed.document;
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for buffer in document.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => parsed.blob.clone().ok_or_else(|| {
                ImportError::Invalid("o arquivo usa o bloco binário, mas não tem um".to_string())
            })?,
            gltf::buffer::Source::Uri(uri) => {
                if let Some(encoded) = uri.strip_prefix("data:") {
                    decode_data_uri(encoded)?
                } else {
                    resolve(uri).ok_or_else(|| {
                        ImportError::Refused(format!(
                            "o arquivo precisa de `{uri}`, que não está na mesma pasta ou não pode ser lido"
                        ))
                    })?
                }
            }
        };
        if data.len() < buffer.length() {
            return Err(ImportError::Invalid(format!(
                "o bloco de dados {} é menor do que diz ser",
                buffer.index()
            )));
        }
        buffers.push(data);
    }

    let mut model = ImportedModel::default();
    let mut visited = 0usize;
    let mut total_vertices = 0usize;
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| ImportError::Invalid("o arquivo não tem cena".to_string()))?;
    let identity = identity_matrix();
    for node in scene.nodes() {
        walk(
            &node,
            &identity,
            0,
            &buffers,
            &mut model,
            &mut visited,
            &mut total_vertices,
        )?;
    }
    if document.textures().count() > 0 {
        model.warnings.push(
            "as texturas foram ignoradas: só a cor base de cada material é usada".to_string(),
        );
    }
    // Skeleton and clips (phase 4). A file without them keeps working exactly as before.
    if document.skins().count() > 0 || document.animations().count() > 0 {
        crate::animation::read_animation(&parsed.document, &buffers, &mut model);
    }
    if model.nodes.is_empty() {
        return Err(ImportError::Refused(
            "o arquivo não tem nenhuma malha de triângulos para importar".to_string(),
        ));
    }
    Ok(model)
}

type Matrix = [[f32; 4]; 4];

fn identity_matrix() -> Matrix {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `parent * child`, both column-major (`matrix[column][row]`).
fn multiply(parent: &Matrix, child: &Matrix) -> Matrix {
    let mut out = [[0.0f32; 4]; 4];
    for (out_column, child_column) in out.iter_mut().zip(child.iter()) {
        for (row, cell) in out_column.iter_mut().enumerate() {
            *cell = parent
                .iter()
                .zip(child_column.iter())
                .map(|(parent_column, child_value)| {
                    parent_column.get(row).copied().unwrap_or(0.0) * child_value
                })
                .sum();
        }
    }
    out
}

fn walk(
    node: &gltf::Node<'_>,
    parent: &Matrix,
    depth: usize,
    buffers: &[Vec<u8>],
    model: &mut ImportedModel,
    visited: &mut usize,
    total_vertices: &mut usize,
) -> Result<(), ImportError> {
    *visited += 1;
    if *visited > MAX_NODES || depth > MAX_DEPTH {
        return Err(ImportError::Refused(format!(
            "o arquivo tem nós demais (limite: {MAX_NODES} nós, {MAX_DEPTH} níveis)"
        )));
    }
    let world = multiply(parent, &node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        let name = node
            .name()
            .or_else(|| mesh.name())
            .map_or_else(|| format!("Nó {}", node.index()), str::to_string);
        match build_mesh(&mesh, buffers, total_vertices) {
            Ok(Some((polygon, color, skin_weights, note))) => {
                let (position, rotation_degrees, scale) = decompose(&world);
                if let Some(note) = note {
                    model.warnings.push(format!("{name}: {note}"));
                }
                let skin = skin_weights.map(|weights| crate::skin::ImportSkin {
                    skeleton: node.skin().map(|skin| skin.index()).unwrap_or(0),
                    weights,
                });
                model.nodes.push(ImportedNode {
                    name,
                    position,
                    rotation_degrees,
                    scale,
                    mesh: polygon,
                    skin,
                    color,
                });
            }
            Ok(None) => model
                .warnings
                .push(format!("{name}: não tem triângulos; foi ignorado")),
            Err(ImportError::Refused(reason)) => model
                .warnings
                .push(format!("{name}: {reason}; foi ignorado")),
            Err(other) => return Err(other),
        }
    }
    for child in node.children() {
        walk(
            &child,
            &world,
            depth + 1,
            buffers,
            model,
            visited,
            total_vertices,
        )?;
    }
    Ok(())
}

/// All triangle primitives of a mesh, joined and welded into one polygon mesh.
#[allow(clippy::type_complexity)]
/// What one primitive contributes: the welded polygon, its colour, the skin weights per welded
/// vertex (when the primitive is skinned), and a sentence to show when something was left out.
type BuiltMesh = (
    PolygonMesh,
    Color,
    Option<Vec<crate::skin::VertexWeights>>,
    Option<String>,
);

fn build_mesh(
    mesh: &gltf::Mesh<'_>,
    buffers: &[Vec<u8>],
    total_vertices: &mut usize,
) -> Result<Option<BuiltMesh>, ImportError> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let mut lookup: HashMap<[i64; 3], u32> = HashMap::new();
    // Skin weights, one per welded vertex, carried beside the mesh (`DR-23`, phase 4).
    let mut weights: Vec<crate::skin::VertexWeights> = Vec::new();
    let mut skinned = false;
    let mut seam_note = false;
    let mut color = None;
    let mut materials = 0usize;
    for primitive in mesh.primitives() {
        let mode = primitive.mode();
        if !matches!(
            mode,
            Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
        ) {
            continue;
        }
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
        let Some(read) = reader.read_positions() else {
            continue;
        };
        let source: Vec<[f32; 3]> = read.collect();
        let indices: Vec<u32> = match reader.read_indices() {
            Some(indices) => indices.into_u32().collect(),
            None => (0..u32::try_from(source.len())
                .map_err(|_| ImportError::Refused("vértices demais".to_string()))?)
                .collect(),
        };
        if source
            .iter()
            .any(|point| point.iter().any(|value| !value.is_finite()))
        {
            return Err(ImportError::Refused(
                "tem pontos que não são números".to_string(),
            ));
        }
        if indices.iter().any(|index| *index as usize >= source.len()) {
            return Err(ImportError::Invalid(
                "um triângulo aponta para um vértice que não existe".to_string(),
            ));
        }
        // Skin weights of this primitive, when it has any. `None` means "not a skinned primitive".
        let primitive_weights: Option<Vec<crate::skin::VertexWeights>> =
            if let (Some(joints), Some(weights_read)) =
                (reader.read_joints(0), reader.read_weights(0))
            {
                let joints: Vec<[u16; 4]> = joints.into_u16().collect();
                let weights_read: Vec<[f32; 4]> = weights_read.into_f32().collect();
                let count = joints.len().min(weights_read.len());
                Some(
                    (0..count)
                        .map(|index| {
                            crate::skin::VertexWeights::read(
                                joints.get(index).copied().unwrap_or([0; 4]),
                                weights_read.get(index).copied().unwrap_or([0.0; 4]),
                            )
                            .unwrap_or_default()
                        })
                        .collect(),
                )
            } else {
                None
            };
        if primitive_weights.is_some() {
            skinned = true;
        }
        // Weld by position so editing tools see one vertex where the file split them for normals.
        let mut remap: Vec<u32> = Vec::with_capacity(source.len());
        for (source_index, point) in source.iter().enumerate() {
            let key = [
                (point[0] / WELD_STEP).round() as i64,
                (point[1] / WELD_STEP).round() as i64,
                (point[2] / WELD_STEP).round() as i64,
            ];
            let next = u32::try_from(positions.len())
                .map_err(|_| ImportError::Refused("vértices demais".to_string()))?;
            let mut fresh = false;
            let index = *lookup.entry(key).or_insert_with(|| {
                positions.push(Vec3::new(point[0], point[1], point[2]));
                fresh = true;
                next
            });
            if fresh {
                // The first vertex that owns this position keeps its weights. A second one with a
                // different weight set is a seam (two limbs meeting); merging them would bend the
                // model where the artist put a crease, so the first wins and the file says so.
                let take = primitive_weights
                    .as_ref()
                    .and_then(|list| list.get(source_index).copied())
                    .unwrap_or_default();
                weights.push(take);
            } else if let (Some(list), Some(taken)) = (
                primitive_weights.as_ref(),
                weights.get(usize::try_from(index).unwrap_or(0)),
            ) && let Some(other) = list.get(source_index)
                && (other.weights[0] - taken.weights[0]).abs() > 1e-3
            {
                seam_note = true;
            }
            remap.push(index);
        }
        let triangle = |a: u32, b: u32, c: u32, faces: &mut Vec<Vec<u32>>| {
            let (a, b, c) = (
                remap.get(a as usize).copied(),
                remap.get(b as usize).copied(),
                remap.get(c as usize).copied(),
            );
            if let (Some(a), Some(b), Some(c)) = (a, b, c)
                && a != b
                && b != c
                && a != c
            {
                faces.push(vec![a, b, c]);
            }
        };
        match mode {
            Mode::Triangles => {
                for [a, b, c] in indices.as_chunks::<3>().0 {
                    triangle(*a, *b, *c, &mut faces);
                }
            }
            Mode::TriangleStrip => {
                for (position, window) in indices.windows(3).enumerate() {
                    if let [a, b, c] = window {
                        // Every other triangle is flipped to keep the winding.
                        if position % 2 == 0 {
                            triangle(*a, *b, *c, &mut faces);
                        } else {
                            triangle(*b, *a, *c, &mut faces);
                        }
                    }
                }
            }
            _ => {
                if let Some((&first, rest)) = indices.split_first() {
                    for pair in rest.windows(2) {
                        if let [b, c] = pair {
                            triangle(first, *b, *c, &mut faces);
                        }
                    }
                }
            }
        }
        if positions.len() > MAX_MESH_VERTICES {
            return Err(ImportError::Refused(format!(
                "a malha passa do limite de {MAX_MESH_VERTICES} vértices"
            )));
        }
        let [red, green, blue, alpha] = primitive
            .material()
            .pbr_metallic_roughness()
            .base_color_factor();
        materials += 1;
        if color.is_none() {
            color = Some(Color::new(
                red.clamp(0.0, 1.0),
                green.clamp(0.0, 1.0),
                blue.clamp(0.0, 1.0),
                alpha.clamp(0.0, 1.0),
            ));
        }
    }
    if faces.is_empty() {
        return Ok(None);
    }
    *total_vertices += positions.len();
    if *total_vertices > MAX_TOTAL_VERTICES {
        return Err(ImportError::Refused(format!(
            "o arquivo passa de {MAX_TOTAL_VERTICES} vértices no total"
        )));
    }
    let polygon = PolygonMesh { positions, faces };
    polygon
        .validate()
        .map_err(|error| ImportError::Refused(error.0))?;
    let mut notes: Vec<String> = Vec::new();
    if materials > 1 {
        notes.push(format!(
            "tem {materials} materiais; a malha usa a cor do primeiro"
        ));
    }
    if seam_note {
        notes.push(
            "duas partes da malha caem na mesma posição com pesos diferentes; ficou o primeiro              (costura entre membros)"
                .to_string(),
        );
    }
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    let skin_weights = skinned.then_some(weights);
    Ok(Some((
        polygon,
        color.unwrap_or(Color::WHITE),
        skin_weights,
        note,
    )))
}

/// Position, engine Euler degrees and scale of a world matrix. A mirrored matrix gets a negative
/// x scale. Shear (a rotated parent with a stretched child) cannot be kept and is dropped.
fn decompose(matrix: &Matrix) -> (Vec3, Vec3, Vec3) {
    let column = |index: usize| {
        let [x, y, z, _] = matrix.get(index).copied().unwrap_or([0.0; 4]);
        Vec3::new(x, y, z)
    };
    let (x_axis, y_axis, z_axis) = (column(0), column(1), column(2));
    let mut scale = Vec3::new(x_axis.length(), y_axis.length(), z_axis.length());
    let safe = |value: f32| if value > 1.0e-8 { value } else { 1.0 };
    let (mut x_unit, y_unit, z_unit) = (
        x_axis * (1.0 / safe(scale.x)),
        y_axis * (1.0 / safe(scale.y)),
        z_axis * (1.0 / safe(scale.z)),
    );
    if x_unit.cross(y_unit).dot(z_unit) < 0.0 {
        scale.x = -scale.x;
        x_unit = x_unit * -1.0;
    }
    // The engine's rotation is R = Ry(yaw) * Rx(pitch) * Rz(roll): read the angles off its columns.
    let sine_pitch = (-z_unit.y).clamp(-1.0, 1.0);
    let pitch = sine_pitch.asin();
    let (yaw, roll) = if sine_pitch.abs() < 0.9999 {
        (z_unit.x.atan2(z_unit.z), x_unit.y.atan2(y_unit.y))
    } else {
        ((-x_unit.z).atan2(x_unit.x), 0.0)
    };
    (
        Vec3::new(matrix[3][0], matrix[3][1], matrix[3][2]),
        Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()),
        scale,
    )
}

fn decode_data_uri(rest: &str) -> Result<Vec<u8>, ImportError> {
    let (_, payload) = rest
        .split_once(";base64,")
        .ok_or_else(|| ImportError::Invalid("um data URI sem base64".to_string()))?;
    let mut out = Vec::with_capacity(payload.len() / 4 * 3);
    let mut bits = 0u32;
    let mut count = 0u8;
    for byte in payload.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' => continue,
            _ => {
                return Err(ImportError::Invalid(
                    "base64 com caractere inválido".to_string(),
                ));
            }
        };
        bits = (bits << 6) | u32::from(value);
        count += 1;
        if count == 4 {
            out.extend_from_slice(&[(bits >> 16) as u8, (bits >> 8) as u8, bits as u8]);
            bits = 0;
            count = 0;
        }
        if out.len() > MAX_FILE_BYTES {
            return Err(ImportError::Refused(
                "um bloco de dados embutido é grande demais".to_string(),
            ));
        }
    }
    match count {
        2 => out.push((bits >> 4) as u8),
        3 => out.extend_from_slice(&[(bits >> 10) as u8, (bits >> 2) as u8]),
        _ => {}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GLB with one node: a unit cube whose faces are split per vertex (24 points), moved,
    /// turned 90 degrees around Y and stretched, with a red material.
    fn cube_glb(extra_json: &str, transform: &str) -> Vec<u8> {
        let mut points: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        // Six faces, four vertices each (so vertices repeat across faces, like a real export).
        let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ];
        for (normal, u, v) in faces {
            let base = points.len() as u16;
            for (a, b) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                points.push([
                    (normal[0] + a * u[0] + b * v[0]) * 0.5,
                    (normal[1] + a * u[1] + b * v[1]) * 0.5,
                    (normal[2] + a * u[2] + b * v[2]) * 0.5,
                ]);
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mut bin: Vec<u8> = Vec::new();
        for point in &points {
            for value in point {
                bin.extend_from_slice(&value.to_le_bytes());
            }
        }
        let index_offset = bin.len();
        for index in &indices {
            bin.extend_from_slice(&index.to_le_bytes());
        }
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
"nodes":[{{"name":"Cubo","mesh":0,{transform}}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]}}],
"materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[1.0,0.0,0.0,1.0]}}}}],
"buffers":[{{"byteLength":{}}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{}}},{{"buffer":0,"byteOffset":{index_offset},"byteLength":{}}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":{},"type":"VEC3","min":[-0.5,-0.5,-0.5],"max":[0.5,0.5,0.5]}},
{{"bufferView":1,"componentType":5123,"count":{},"type":"SCALAR"}}]{extra_json}}}"#,
            bin.len(),
            index_offset,
            indices.len() * 2,
            points.len(),
            indices.len()
        );
        let mut json_bytes = json.into_bytes();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    fn nothing(_: &str) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn a_cube_comes_in_welded_colored_and_placed() {
        // Translate (1, 2, 3), turn 90 degrees around Y (quaternion x y z w), scale (2, 1, 1).
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let transform = format!(
            r#""translation":[1.0,2.0,3.0],"rotation":[0.0,{half},0.0,{half}],"scale":[2.0,1.0,1.0]"#
        );
        let model = import_gltf(&cube_glb("", &transform), &nothing).expect("valid file");
        assert_eq!(model.nodes.len(), 1);
        let node = &model.nodes[0];
        assert_eq!(node.name, "Cubo");
        assert_eq!(
            node.mesh.positions.len(),
            8,
            "24 split points weld into the 8 corners"
        );
        assert_eq!(node.mesh.faces.len(), 12, "twelve triangles");
        node.mesh.validate().expect("an acceptable mesh");
        assert_eq!(node.color, Color::new(1.0, 0.0, 0.0, 1.0));
        assert!((node.position - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
        assert!(
            (node.rotation_degrees.y - 90.0).abs() < 1e-3,
            "{:?}",
            node.rotation_degrees
        );
        assert!(node.rotation_degrees.x.abs() < 1e-3 && node.rotation_degrees.z.abs() < 1e-3);
        assert!(
            (node.scale - Vec3::new(2.0, 1.0, 1.0)).length() < 1e-5,
            "{:?}",
            node.scale
        );
    }

    #[test]
    fn the_rotation_matches_the_engine_convention() {
        // A node turned 90 degrees around Y must send +X to -Z, like Transform::rotation_direction.
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let transform = format!(r#""rotation":[0.0,{half},0.0,{half}]"#);
        let model = import_gltf(&cube_glb("", &transform), &nothing).expect("valid file");
        let rotated = petunia_math::Transform {
            rotation_degrees: model.nodes[0].rotation_degrees,
            ..petunia_math::Transform::IDENTITY
        }
        .right();
        assert!(
            (rotated - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-4,
            "{rotated:?}"
        );
    }

    #[test]
    fn what_is_left_out_is_said() {
        let extra = r#","textures":[{"source":0}],"images":[{"uri":"x.png"}],"animations":[]"#;
        let model = import_gltf(&cube_glb(extra, r#""scale":[1.0,1.0,1.0]"#), &nothing)
            .expect("valid file");
        assert!(
            model
                .warnings
                .iter()
                .any(|warning| warning.contains("texturas")),
            "{:?}",
            model.warnings
        );
    }

    #[test]
    fn hostile_files_are_refused_not_trusted() {
        assert!(import_gltf(b"not a glb", &nothing).is_err());
        assert!(import_gltf(&[], &nothing).is_err());
        let mut truncated = cube_glb("", r#""scale":[1.0,1.0,1.0]"#);
        truncated.truncate(truncated.len() - 40);
        assert!(import_gltf(&truncated, &nothing).is_err(), "a cut file");
        // A document that names an external file the caller will not give.
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}],"buffers":[{"uri":"../../etc/passwd","byteLength":4}]}"#;
        let refused = import_gltf(json, &|_| None);
        assert!(
            matches!(refused, Err(ImportError::Refused(_))),
            "{refused:?}"
        );
        // The same document, with a resolver that only gives files from the model's folder.
        assert!(
            import_gltf(json, &|name| (!name.contains("..")).then(|| vec![0u8; 4])).is_err(),
            "no meshes in it"
        );
        // A file that says it is bigger than it is.
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}],"buffers":[{"uri":"data:application/octet-stream;base64,AAAA","byteLength":99}]}"#;
        assert!(matches!(
            import_gltf(json, &nothing),
            Err(ImportError::Invalid(_))
        ));
        // A pile of nodes.
        let nodes: Vec<String> = (0..MAX_NODES + 5).map(|_| "{}".to_string()).collect();
        let list: Vec<String> = (0..MAX_NODES + 5).map(|index| index.to_string()).collect();
        let many = format!(
            r#"{{"asset":{{"version":"2.0"}},"scenes":[{{"nodes":[{}]}}],"nodes":[{}]}}"#,
            list.join(","),
            nodes.join(",")
        );
        assert!(matches!(
            import_gltf(many.as_bytes(), &nothing),
            Err(ImportError::Refused(_))
        ));
    }

    #[test]
    fn a_data_uri_buffer_works() {
        assert_eq!(
            decode_data_uri("application/octet-stream;base64,AQIDBA==").expect("valid"),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            decode_data_uri("x;base64,AQID").expect("valid"),
            vec![1, 2, 3]
        );
        assert!(decode_data_uri("x;base64,A$ID").is_err());
        assert!(decode_data_uri("no base64 here").is_err());
    }

    #[test]
    fn the_same_file_gives_the_same_model() {
        let file = cube_glb("", r#""translation":[0.5,0.5,0.5]"#);
        assert_eq!(import_gltf(&file, &nothing), import_gltf(&file, &nothing));
    }

    /// A tiny GLB with one skinned triangle: 2 joints, an animation that moves the root.
    ///
    /// Built byte by byte because the crate has no fixture and a real model would be a large binary
    /// in the repository; the bytes are the smallest document that exercises skin + clip reading.
    fn skinned_glb() -> Vec<u8> {
        // One skinned triangle: 3 vertices, 2 joints, and a clip that moves the root.
        //
        // The float block, in order: positions (3 x vec3), weights (3 x vec4), inverse binds
        // (2 x mat4), clip times (2), clip translations (2 x vec3). The joints are `u16` and live
        // in their own region after the floats, because glTF requires an integer accessor there.
        let mut floats: Vec<f32> = Vec::new();
        // Triangle on the ground.
        floats.extend([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        let positions_len = floats.len() * 4;
        // Every vertex: joint 0, full weight.
        for _ in 0..3 {
            floats.extend([1.0, 0.0, 0.0, 0.0]);
        }
        let weights_at = floats.len() * 4 - 48;
        let weights_len = 48;
        // Inverse binds: identity for both joints.
        for _ in 0..2 {
            floats.extend([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]);
        }
        let binds_at = weights_at + weights_len;
        let binds_len = floats.len() * 4 - binds_at;
        let times_at = floats.len() * 4;
        floats.extend([0.0, 1.0]);
        let translations_at = floats.len() * 4;
        floats.extend([0.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
        let translations_len = floats.len() * 4 - translations_at;
        // Joints: u16, four per vertex, all zero.
        let joints_at = floats.len() * 4;
        let joints_len = 3 * 4 * 2;
        let indices_at = joints_at + joints_len + 2; // keep the block 4-aligned below
        let indices_len = 6;

        let mut binary = Vec::new();
        for value in &floats {
            binary.extend(value.to_le_bytes());
        }
        while binary.len() < joints_at {
            binary.push(0);
        }
        for _ in 0..3 {
            for _ in 0..4 {
                binary.extend(0u16.to_le_bytes());
            }
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        for index in [0u16, 1, 2] {
            binary.extend(index.to_le_bytes());
        }
        let indices_at = indices_at.min(binary.len() - indices_len);
        while binary.len() % 4 != 0 {
            binary.push(0);
        }

        let json = format!(
            r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [{{ "nodes": [0] }}],
  "nodes": [
    {{ "name": "raiz", "children": [1, 2], "translation": [0, 0, 0] }},
    {{ "name": "filho", "translation": [0, 1, 0] }},
    {{ "name": "malha", "mesh": 0, "skin": 0 }}
  ],
  "skins": [{{ "joints": [0, 1], "inverseBindMatrices": 2 }}],
  "meshes": [{{
    "primitives": [{{
      "attributes": {{ "POSITION": 0, "JOINTS_0": 5, "WEIGHTS_0": 1 }},
      "indices": 6,
      "mode": 4
    }}]
  }}],
  "animations": [{{
    "name": "anda",
    "channels": [
      {{ "sampler": 0, "target": {{ "node": 0, "path": "translation" }} }}
    ],
    "samplers": [{{ "input": 3, "output": 4, "interpolation": "LINEAR" }}]
  }}],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": {positions_len} }},
    {{ "buffer": 0, "byteOffset": {weights_at}, "byteLength": {weights_len} }},
    {{ "buffer": 0, "byteOffset": {binds_at}, "byteLength": {binds_len} }},
    {{ "buffer": 0, "byteOffset": {times_at}, "byteLength": 8 }},
    {{ "buffer": 0, "byteOffset": {translations_at}, "byteLength": {translations_len} }},
    {{ "buffer": 0, "byteOffset": {joints_at}, "byteLength": {joints_len} }},
    {{ "buffer": 0, "byteOffset": {indices_at}, "byteLength": {indices_len} }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
       "min": [0, 0, 0], "max": [1, 0, 1] }},
    {{ "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }},
    {{ "bufferView": 2, "componentType": 5126, "count": 2, "type": "MAT4" }},
    {{ "bufferView": 3, "componentType": 5126, "count": 2, "type": "SCALAR" }},
    {{ "bufferView": 4, "componentType": 5126, "count": 2, "type": "VEC3" }},
    {{ "bufferView": 5, "componentType": 5123, "count": 3, "type": "VEC4" }},
    {{ "bufferView": 6, "componentType": 5123, "count": 3, "type": "SCALAR" }}
  ],
  "buffers": [{{ "byteLength": {binary_len} }}]
}}"#,
            binary_len = binary.len(),
        );

        let json_bytes = json.as_bytes();
        let json_padding = (4 - json_bytes.len() % 4) % 4;
        let mut json_chunk = json_bytes.to_vec();
        json_chunk.extend(std::iter::repeat_n(b' ', json_padding));
        let mut out = Vec::new();
        out.extend(0x4654_6C67u32.to_le_bytes()); // "glTF"
        out.extend(2u32.to_le_bytes());
        let total = 12 + 8 + json_chunk.len() + 8 + binary.len();
        out.extend((total as u32).to_le_bytes());
        out.extend((json_chunk.len() as u32).to_le_bytes());
        out.extend(0x4E4F_534Au32.to_le_bytes()); // "JSON"
        out.extend(&json_chunk);
        out.extend((binary.len() as u32).to_le_bytes());
        out.extend(0x004E_4942u32.to_le_bytes()); // "BIN\0"
        out.extend(&binary);
        out
    }

    #[test]
    fn a_glb_with_a_skeleton_and_a_clip_is_read() {
        let bytes = skinned_glb();
        let model = import_gltf(&bytes, &nothing).expect("the file imports");
        let skeleton = model.skeleton.expect("the file has a skeleton");
        assert_eq!(skeleton.len(), 2, "two joints");
        assert_eq!(model.clips.len(), 1, "one clip");
        let clip = model.clips.first().expect("clip");
        assert_eq!(clip.name, "anda");
        assert!(
            (clip.duration() - 1.0).abs() < 1e-5,
            "{:?}",
            clip.duration()
        );
        // At the end the root is at x=2, and the child rides along at its own y=1.
        let pose = skeleton.pose(clip, 1.0, 1.0);
        assert!(
            (pose[0].0[12] - 2.0).abs() < 1e-5,
            "root moved: {:?}",
            pose[0]
        );
        assert!(
            (pose[1].0[12] - 2.0).abs() < 1e-5,
            "child follows: {:?}",
            pose[1]
        );
        assert!(
            (pose[1].0[13] - 1.0).abs() < 1e-5,
            "child keeps its y: {:?}",
            pose[1]
        );

        // The mesh carries its weights: every welded vertex belongs to joint 0 with full weight.
        let node = model.nodes.first().expect("mesh node");
        let skin = node.skin.as_ref().expect("the mesh is skinned");
        assert_eq!(skin.weights.len(), node.mesh.positions.len());
        let first = skin.weights.first().expect("weights");
        assert!((first.weights[0] - 1.0).abs() < 1e-5, "{first:?}");
        // With the clip's matrices, the weighted vertex follows the root to x=2.
        let matrices = skeleton.skinning_matrices(clip, 1.0, 1.0);
        let point = node.mesh.positions.first().copied().expect("vertex");
        let moved = first.transform(point, &matrices);
        assert!(
            (moved.x - 2.0).abs() < 1e-4,
            "the vertex is skinned: {moved:?}"
        );
    }

    #[test]
    fn a_file_without_a_skeleton_keeps_working() {
        // The existing cube fixture: no skin, no animation.
        let bytes = cube_glb("", "\"translation\":[0,0,0]");
        let model = import_gltf(&bytes, &|_| None).expect("imports");
        assert!(model.skeleton.is_none());
        assert!(model.clips.is_empty());
        assert_eq!(model.nodes.len(), 1, "the mesh is still there");
    }
}
