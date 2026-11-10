//! Closed-solid booleans for brushes and imported meshes.
//!
//! Modeling operations (inset, extrude, loop cut, bevel, knife, subdivide) were removed (DR-51).
//! What stays is union, subtract and intersect, plus turning brush triangles into a polygon mesh.
//!
//! The geometry kernel is Petunia3D's (`legacy`, commit `dec467127a7f`, see `ATTRIBUTION.md`).
//! `glam` stays inside this crate.

#![allow(clippy::indexing_slicing)]

pub mod legacy;

use std::collections::BTreeSet;

use petunia_math::{MAX_MESH_FACES, MAX_MESH_VERTICES, PolygonMesh, Vec3};

/// Faces whose normals differ by less than this (degrees) merge after a brush is converted.
const COPLANAR_DEGREES: f32 = 0.5;

/// Why an operation did not change the mesh. `code` is stable (tests, scripts, the GUI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshEditError {
    pub code: &'static str,
    pub message: String,
    pub suggestion: String,
}

impl MeshEditError {
    fn new(code: &'static str, message: &str, suggestion: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            suggestion: suggestion.to_string(),
        }
    }
}

impl std::fmt::Display for MeshEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for MeshEditError {}

type MeshResult = Result<PolygonMesh, MeshEditError>;

/// The three solid operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOperation {
    /// Both volumes together ("fundir").
    Union,
    /// The first minus the second ("cortar").
    Subtract,
    /// Only where they overlap ("interseção").
    Intersect,
}

impl BooleanOperation {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "union" => Some(Self::Union),
            "subtract" => Some(Self::Subtract),
            "intersect" => Some(Self::Intersect),
            _ => None,
        }
    }
}

fn invalid_mesh(reason: &str) -> MeshEditError {
    MeshEditError::new(
        "MESH_INVALID",
        &format!("a malha está inválida: {reason}"),
        "Desfaça a última operação ou recrie o objeto.",
    )
}

fn check_mesh(mesh: &PolygonMesh) -> Result<(), MeshEditError> {
    mesh.validate().map_err(|error| invalid_mesh(&error.0))?;
    if mesh.faces.is_empty() {
        return Err(MeshEditError::new(
            "MESH_EMPTY",
            "a malha não tem faces",
            "Desfaça a operação que a esvaziou.",
        ));
    }
    Ok(())
}

fn unchanged(what: &str, suggestion: &str) -> MeshEditError {
    MeshEditError::new(
        "MESH_UNCHANGED",
        &format!("{what}: a malha não mudou"),
        suggestion,
    )
}

// ---- conversion at the boundary ---------------------------------------------------------------

fn to_legacy(mesh: &PolygonMesh) -> legacy::Mesh {
    let mut converted = legacy::Mesh::default();
    for point in &mesh.positions {
        converted
            .verts
            .push(legacy::Vertex::new(point.x, point.y, point.z));
    }
    for face in &mesh.faces {
        converted.push_face(legacy::Face::new(face.clone()));
    }
    converted
}

/// Back to plain data: degenerate faces dropped, unused vertices removed (vertex numbers stay in
/// their original order), then the whole mesh validated.
fn from_legacy(mesh: &legacy::Mesh) -> MeshResult {
    let mut used = vec![false; mesh.verts.len()];
    let mut faces = Vec::new();
    for face in &mesh.faces {
        let mut corners: Vec<u32> = Vec::with_capacity(face.verts.len());
        for vertex in &face.verts {
            if *vertex as usize >= mesh.verts.len() {
                return Err(invalid_mesh(
                    "uma operação produziu um índice fora da malha",
                ));
            }
            if corners.last() != Some(vertex) {
                corners.push(*vertex);
            }
        }
        if corners.len() > 1 && corners.first() == corners.last() {
            corners.pop();
        }
        let distinct: BTreeSet<u32> = corners.iter().copied().collect();
        if distinct.len() < 3 || distinct.len() != corners.len() {
            continue;
        }
        for vertex in &corners {
            used[*vertex as usize] = true;
        }
        faces.push(corners);
    }
    let mut renumber = vec![0u32; mesh.verts.len()];
    let mut positions = Vec::new();
    for (vertex, keep) in used.iter().enumerate() {
        if *keep {
            renumber[vertex] = positions.len() as u32;
            let [x, y, z] = mesh.verts[vertex].pos;
            positions.push(Vec3::new(x, y, z));
        }
    }
    if positions.len() > MAX_MESH_VERTICES || faces.len() > MAX_MESH_FACES {
        return Err(MeshEditError::new(
            "MESH_TOO_LARGE",
            "o resultado passaria do limite de tamanho de uma malha",
            "Use menos cortes ou segmentos, ou divida o objeto.",
        ));
    }
    for face in &mut faces {
        for vertex in face.iter_mut() {
            *vertex = renumber[*vertex as usize];
        }
    }
    let result = PolygonMesh { positions, faces };
    result.validate().map_err(|error| {
        MeshEditError::new(
            "MESH_TOO_LARGE",
            &format!("o resultado não é uma malha válida: {error}"),
            "Use menos cortes ou segmentos, ou divida o objeto.",
        )
    })?;
    Ok(result)
}

/// The same positions and the same number of faces, whatever the numbering (the boolean kernel
/// renumbers vertices even when nothing was cut).
fn same_shape(first: &PolygonMesh, second: &PolygonMesh) -> bool {
    let sorted = |mesh: &PolygonMesh| {
        let mut points: Vec<[u32; 3]> = mesh
            .positions
            .iter()
            .map(|point| [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()])
            .collect();
        points.sort_unstable();
        points
    };
    first.faces.len() == second.faces.len() && sorted(first) == sorted(second)
}

/// True for a closed surface where every edge joins exactly two faces (a solid with an inside).
///
/// Checked directly: every directed edge `a → b` of every face appears exactly once, and so does
/// its reverse `b → a`. That is a closed surface whose faces agree on which side is outside. (It
/// is linear; building Petunia3D's half-edge structure for the same answer was about 200 ms on a
/// mesh of 2.6 thousand vertices in the editor's build.)
pub fn is_solid(mesh: &PolygonMesh) -> bool {
    if mesh.validate().is_err() || mesh.faces.is_empty() {
        return false;
    }
    let mut directed: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for face in &mesh.faces {
        for corner in 0..face.len() {
            let edge = (face[corner], face[(corner + 1) % face.len()]);
            *directed.entry(edge).or_default() += 1;
        }
    }
    directed
        .iter()
        .all(|(&(from, to), &uses)| uses == 1 && directed.get(&(to, from)) == Some(&1))
}

// ---- operations ---------------------------------------------------------------------------

/// A polygon mesh from triangles (a brush's drawing mesh): equal positions are welded and
/// coplanar triangles merged back into quads and n-gons.
pub fn from_triangles(positions: &[Vec3], triangles: &[[u32; 3]]) -> MeshResult {
    let mut mesh = legacy::Mesh::default();
    for point in positions {
        if !point.is_finite() {
            return Err(invalid_mesh("posição não finita"));
        }
        mesh.verts
            .push(legacy::Vertex::new(point.x, point.y, point.z));
    }
    for triangle in triangles {
        if triangle
            .iter()
            .any(|vertex| *vertex as usize >= positions.len())
        {
            return Err(invalid_mesh("triângulo com índice fora da malha"));
        }
        mesh.push_face(legacy::Face::new(triangle.to_vec()));
    }
    mesh.weld(1.0e-5);
    mesh.remove_degenerate_faces(1.0e-6);
    mesh.limited_dissolve(COPLANAR_DEGREES);
    let result = from_legacy(&mesh)?;
    check_mesh(&result)?;
    Ok(result)
}

/// `target` combined with `tool`; both in the same coordinates (the caller moves the tool into
/// the target's space). Both must be closed solids.
pub fn boolean(
    target: &PolygonMesh,
    tool: &PolygonMesh,
    operation: BooleanOperation,
) -> MeshResult {
    check_mesh(target)?;
    check_mesh(tool)?;
    for (label, mesh) in [("o objeto", target), ("a ferramenta", tool)] {
        if !is_solid(mesh) {
            return Err(MeshEditError::new(
                "MESH_NOT_SOLID",
                &format!("{label} não é um sólido fechado (tem buracos ou arestas soltas)"),
                "Booleanos precisam de dois sólidos fechados. Feche os buracos ou use outro objeto.",
            ));
        }
    }
    let kind = match operation {
        BooleanOperation::Union => legacy::boolean::BooleanOp::Union,
        BooleanOperation::Subtract => legacy::boolean::BooleanOp::Difference,
        BooleanOperation::Intersect => legacy::boolean::BooleanOp::Intersection,
    };
    let (raw, _report) = legacy::boolean::boolean_meshes_clean(
        &to_legacy(target),
        &to_legacy(tool),
        kind,
    )
    .map_err(|error| match error {
        legacy::boolean::BooleanError::InvalidOutput(_) => MeshEditError::new(
            "MESH_EMPTY_RESULT",
            "o resultado do booleano ficaria vazio (os objetos não se cruzam ou um apaga o outro)",
            "Encoste os dois objetos e tente de novo.",
        ),
        other => MeshEditError::new(
            "MESH_BOOLEAN_FAILED",
            &format!("o booleano falhou: {other}"),
            "Mova um pouco a ferramenta para que as faces não fiquem exatamente no mesmo plano.",
        ),
    })?;
    let result = from_legacy(&raw)?;
    check_mesh(&result).map_err(|_| {
        MeshEditError::new(
            "MESH_EMPTY_RESULT",
            "o resultado do booleano ficaria vazio",
            "Encoste os dois objetos e tente de novo.",
        )
    })?;
    if same_shape(&result, target) {
        return Err(unchanged(
            "booleano",
            "A ferramenta não encosta no objeto; mova-a para cruzar a superfície.",
        ));
    }
    Ok(result)
}

/// Inserts quad loop cuts along the quad ring or planar section containing `edge`.
pub fn loop_cut(mesh: &PolygonMesh, edge: [u32; 2], cuts: u32, slide: f32) -> MeshResult {
    check_mesh(mesh)?;
    if cuts == 0 || cuts > 32 {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "`cuts` precisa ficar entre 1 e 32",
            "Escolha um número de cortes de 1 a 32.",
        ));
    }
    if !slide.is_finite() || !(-1.0..=1.0).contains(&slide) {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "`slide` precisa ficar entre -1 e 1",
            "Use 0 para cortar no meio.",
        ));
    }
    let mut working = to_legacy(mesh);
    working
        .loop_cut((edge[0], edge[1]), cuts as usize, slide)
        .map_err(|error| {
            let (code, message, suggestion) = match error {
                legacy::loop_cut::LoopCutError::NonQuad => (
                    "MESH_LOOP_NOT_QUADS",
                    "o anel passa por um triângulo ou por um polígono de mais de 4 lados",
                    "O corte em laço só atravessa quadriláteros; escolha uma aresta de outro anel.",
                ),
                legacy::loop_cut::LoopCutError::NonManifold => (
                    "MESH_LOOP_NON_MANIFOLD",
                    "o anel passa por uma aresta compartilhada por mais de duas faces",
                    "Escolha outra aresta.",
                ),
                legacy::loop_cut::LoopCutError::InvalidEdge => (
                    "MESH_EDGE_INVALID",
                    "a aresta escolhida não existe na malha",
                    "Escolha uma aresta de uma face existente.",
                ),
                _ => (
                    "MESH_LOOP_FAILED",
                    "não foi possível realizar o corte em laço nesta aresta",
                    "Escolha outra aresta.",
                ),
            };
            MeshEditError::new(code, message, suggestion)
        })?;
    let result = from_legacy(&working)?;
    check_mesh(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
