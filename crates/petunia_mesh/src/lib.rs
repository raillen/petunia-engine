//! Editable polygon meshes: the modeling operations of Petunia3D behind a small, validated API.
//!
//! Every operation takes a [`PolygonMesh`] (the value a `Mesh` row stores) and returns a new one,
//! or a [`MeshEditError`] that says why nothing changed. Nothing here touches the world, the
//! editor or the GPU: the editor turns each result into one undoable `world.set`.
//!
//! The geometry itself is Petunia3D's (`legacy`, copied from commit `dec467127a7f`, see
//! `ATTRIBUTION.md`). Its types (`legacy::Mesh`, and `glam` inside it) never leave this crate:
//! the functions below convert at the boundary, check the input before the legacy code sees it,
//! and check the output before anyone else does.
//!
//! | Operation | Function | Needs |
//! |---|---|---|
//! | Union, subtract, intersect | [`boolean`] | two closed solids |
//! | Inset faces | [`inset`] | faces |
//! | Extrude faces | [`extrude`] | faces |
//! | Loop cut | [`loop_cut`] | one edge of a ring of quads |
//! | Bevel edges | [`bevel`] | edges |
//! | Subdivide | [`subdivide`] | triangles or quads |
//! | Dissolve edges | [`dissolve_edges`] | edges between two faces |
//! | Merge vertices (at center) | [`merge_vertices`] | two or more vertices |
//! | Merge by distance | [`merge_by_distance`] | — |
//! | Delete faces | [`delete_faces`] | faces |
//! | Flip faces | [`flip_faces`] | faces (none = all) |
//! | Knife | [`knife`] | two points on edges of the same face |
//! | Brush → mesh | [`from_triangles`] | triangles |
#![allow(clippy::indexing_slicing)] // the legacy module indexes validated meshes (migrate when touched)

pub mod legacy;

use std::collections::BTreeSet;

use petunia_math::{MAX_MESH_FACES, MAX_MESH_VERTICES, PolygonMesh, Vec3};

/// Largest distance an operation accepts (meters): bigger is a typo, not a level.
pub const MAX_DISTANCE: f32 = 1_000.0;
/// Most cuts of one loop cut or subdivision.
pub const MAX_CUTS: u32 = 32;
/// Most segments of a rounded bevel.
pub const MAX_BEVEL_SEGMENTS: u32 = legacy::MAX_BEVEL_SEGMENTS;
/// Faces whose normals differ by less than this (degrees) merge back into one polygon after a
/// brush is converted or a boolean is cleaned up.
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

/// How several selected faces move when extruded or inset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    /// Neighboring faces move as one region and share their new walls.
    Region,
    /// Every face on its own.
    Individual,
}

impl Grouping {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "region" => Some(Self::Region),
            "individual" => Some(Self::Individual),
            _ => None,
        }
    }
}

/// A point on an edge for the knife: `fraction` 0 is at `edge[0]`, 1 at `edge[1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePoint {
    pub edge: [u32; 2],
    pub fraction: f32,
}

// ---- checks ---------------------------------------------------------------------------------

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

fn check_distance(name: &str, value: f32, allow_zero: bool) -> Result<(), MeshEditError> {
    if !value.is_finite() || value.abs() > MAX_DISTANCE || (!allow_zero && value == 0.0) {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            &format!(
                "`{name}` precisa ser um número finito entre -{MAX_DISTANCE} e {MAX_DISTANCE} m, diferente de zero"
            ),
            "Digite uma distância em metros, por exemplo 0,25.",
        ));
    }
    Ok(())
}

fn check_count(name: &str, value: u32, maximum: u32) -> Result<(), MeshEditError> {
    if value == 0 || value > maximum {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            &format!("`{name}` precisa ficar entre 1 e {maximum}"),
            "Use um número inteiro pequeno, por exemplo 1 ou 2.",
        ));
    }
    Ok(())
}

/// The faces as a sorted set, refusing unknown numbers and empty lists.
fn check_faces(mesh: &PolygonMesh, faces: &[u32]) -> Result<Vec<usize>, MeshEditError> {
    if faces.is_empty() {
        return Err(MeshEditError::new(
            "MESH_NO_FACES",
            "nenhuma face foi escolhida",
            "Escolha uma ou mais faces da malha.",
        ));
    }
    if faces.len() > MAX_MESH_FACES {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "faces demais na lista",
            "Escolha menos faces.",
        ));
    }
    let mut chosen = BTreeSet::new();
    for face in faces {
        if *face as usize >= mesh.faces.len() {
            return Err(MeshEditError::new(
                "MESH_UNKNOWN_FACE",
                &format!(
                    "a face {face} não existe (a malha tem {})",
                    mesh.faces.len()
                ),
                "Escolha a face de novo: a malha mudou.",
            ));
        }
        chosen.insert(*face as usize);
    }
    Ok(chosen.into_iter().collect())
}

/// True when the two vertices are the ends of an edge of some face.
fn is_edge(mesh: &PolygonMesh, edge: [u32; 2]) -> bool {
    let [first, second] = edge;
    first != second
        && mesh.faces.iter().any(|face| {
            face.iter().enumerate().any(|(corner, vertex)| {
                let next = face[(corner + 1) % face.len()];
                (*vertex == first && next == second) || (*vertex == second && next == first)
            })
        })
}

fn check_edges(mesh: &PolygonMesh, edges: &[[u32; 2]]) -> Result<Vec<(u32, u32)>, MeshEditError> {
    if edges.is_empty() {
        return Err(MeshEditError::new(
            "MESH_NO_EDGES",
            "nenhuma aresta foi escolhida",
            "Escolha uma ou mais arestas da malha.",
        ));
    }
    if edges.len() > MAX_MESH_FACES {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "arestas demais na lista",
            "Escolha menos arestas.",
        ));
    }
    let mut chosen = BTreeSet::new();
    for edge in edges {
        if !is_edge(mesh, *edge) {
            return Err(MeshEditError::new(
                "MESH_UNKNOWN_EDGE",
                &format!("{} – {} não é uma aresta da malha", edge[0], edge[1]),
                "Escolha a aresta de novo: a malha mudou.",
            ));
        }
        chosen.insert(legacy::edge_key(edge[0], edge[1]));
    }
    Ok(chosen.into_iter().collect())
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

/// Runs a legacy edit on a copy and refuses a result equal to the input.
fn edit(
    mesh: &PolygonMesh,
    what: &str,
    suggestion: &str,
    change: impl FnOnce(&mut legacy::Mesh) -> Result<(), MeshEditError>,
) -> MeshResult {
    let mut working = to_legacy(mesh);
    change(&mut working)?;
    let result = from_legacy(&working)?;
    if result == *mesh {
        return Err(unchanged(what, suggestion));
    }
    if result.faces.is_empty() {
        return Err(MeshEditError::new(
            "MESH_EMPTY_RESULT",
            &format!("{what}: não sobraria nenhuma face"),
            "Para apagar o objeto, use Apagar.",
        ));
    }
    Ok(result)
}

fn select_faces(mesh: &mut legacy::Mesh, faces: &[usize]) {
    for face in &mut mesh.faces {
        face.selected = false;
    }
    for face in faces {
        if let Some(chosen) = mesh.faces.get_mut(*face) {
            chosen.selected = true;
        }
    }
    mesh.sync_vert_selection_from_faces();
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

/// Inserts vertices that lie on another face's edge into that edge, so neighbors share them
/// (an operation that splits one face's edges otherwise leaves its neighbors with a crack, a
/// "T-junction", and the solid is no longer closed).
///
/// Kept cheap two ways: only vertices that are not in `before` are candidates, and only edges
/// whose two ends were already in `before` can receive them (an edge that was split has a new
/// end). The candidates sit in a grid, so an edge looks only at the cells it crosses.
fn join_t_junctions(mesh: &mut PolygonMesh, before: &PolygonMesh) {
    let bits = |point: Vec3| [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()];
    let old: std::collections::HashSet<[u32; 3]> =
        before.positions.iter().map(|point| bits(*point)).collect();
    let is_new: Vec<bool> = mesh
        .positions
        .iter()
        .map(|point| !old.contains(&bits(*point)))
        .collect();
    let candidates: Vec<u32> = (0..mesh.positions.len() as u32)
        .filter(|vertex| is_new[*vertex as usize])
        .collect();
    let Some((low, high)) = mesh.bounds() else {
        return;
    };
    if candidates.is_empty() {
        return;
    }
    const CELLS_PER_SIDE: f32 = 64.0;
    let extent = high - low;
    let cell = (extent.x.max(extent.y).max(extent.z) / CELLS_PER_SIDE).max(1.0e-6);
    let cell_of = |point: Vec3| {
        [
            ((point.x - low.x) / cell).floor() as i64,
            ((point.y - low.y) / cell).floor() as i64,
            ((point.z - low.z) / cell).floor() as i64,
        ]
    };
    let mut grid: std::collections::HashMap<[i64; 3], Vec<u32>> = std::collections::HashMap::new();
    for vertex in &candidates {
        grid.entry(cell_of(mesh.positions[*vertex as usize]))
            .or_default()
            .push(*vertex);
    }
    let positions = mesh.positions.clone();
    let mut nearby = Vec::new();
    for face in &mut mesh.faces {
        let mut rebuilt = Vec::with_capacity(face.len());
        for corner in 0..face.len() {
            let (start, end) = (face[corner], face[(corner + 1) % face.len()]);
            rebuilt.push(start);
            if is_new[start as usize] || is_new[end as usize] {
                continue;
            }
            let (from, to) = (positions[start as usize], positions[end as usize]);
            let along = to - from;
            let length_squared = along.length_squared();
            if length_squared <= f32::EPSILON {
                continue;
            }
            let tolerance = 1.0e-5 * length_squared.sqrt().max(1.0);
            let margin = Vec3::splat(tolerance);
            let first =
                cell_of(Vec3::new(from.x.min(to.x), from.y.min(to.y), from.z.min(to.z)) - margin);
            let last =
                cell_of(Vec3::new(from.x.max(to.x), from.y.max(to.y), from.z.max(to.z)) + margin);
            let span = (0..3)
                .map(|axis| (last[axis] - first[axis] + 1).max(1))
                .product::<i64>();
            nearby.clear();
            if span as usize > candidates.len() {
                nearby.extend_from_slice(&candidates);
            } else {
                for x in first[0]..=last[0] {
                    for y in first[1]..=last[1] {
                        for z in first[2]..=last[2] {
                            if let Some(found) = grid.get(&[x, y, z]) {
                                nearby.extend_from_slice(found);
                            }
                        }
                    }
                }
            }
            let mut inside: Vec<(f32, u32)> = nearby
                .iter()
                .filter(|vertex| !face.contains(vertex))
                .filter_map(|vertex| {
                    let point = positions[*vertex as usize];
                    let parameter = (point - from).dot(along) / length_squared;
                    let closest = from + along * parameter;
                    (parameter > 0.0 && parameter < 1.0 && (point - closest).length() <= tolerance)
                        .then_some((parameter, *vertex))
                })
                .collect();
            inside.sort_by(|first, second| first.0.total_cmp(&second.0));
            inside.dedup_by_key(|(_, vertex)| *vertex);
            rebuilt.extend(inside.into_iter().map(|(_, vertex)| vertex));
        }
        *face = rebuilt;
    }
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

/// Shrinks the faces inward by `distance` and joins them to their old outline with a ring of
/// new faces. Returns the mesh and the inner faces (the useful selection afterwards).
pub fn inset(
    mesh: &PolygonMesh,
    faces: &[u32],
    distance: f32,
    grouping: Grouping,
) -> Result<(PolygonMesh, Vec<u32>), MeshEditError> {
    check_mesh(mesh)?;
    let chosen = check_faces(mesh, faces)?;
    check_distance("distance", distance, false)?;
    if distance < 0.0 {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "o inset precisa de uma distância positiva",
            "Use um número maior que zero.",
        ));
    }
    let mode = match grouping {
        Grouping::Region => legacy::InsetMode::Region,
        Grouping::Individual => legacy::InsetMode::Individual,
    };
    let mut inner = Vec::new();
    let result = edit(
        mesh,
        "inset",
        "Use uma distância menor que metade da face.",
        |working| {
            let report = working.inset_faces(&chosen, distance, mode);
            inner = report.inner_faces.iter().map(|face| *face as u32).collect();
            Ok(())
        },
    )?;
    inner.retain(|face| (*face as usize) < result.faces.len());
    Ok((result, inner))
}

/// Pushes the faces out along their normal by `distance` (negative digs in) and builds walls.
pub fn extrude(mesh: &PolygonMesh, faces: &[u32], distance: f32, grouping: Grouping) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = check_faces(mesh, faces)?;
    check_distance("distance", distance, false)?;
    edit(
        mesh,
        "extrusão",
        "Escolha faces que formem uma superfície (não arestas soltas).",
        |working| {
            select_faces(working, &chosen);
            match grouping {
                Grouping::Region => working.extrude_selected(distance),
                Grouping::Individual => working.extrude_individual_with(distance, true),
            }
            Ok(())
        },
    )
}

/// Cuts the ring of quads that crosses `edge` with `cuts` new edge loops. `slide` (-1 to 1)
/// shifts them toward one side.
pub fn loop_cut(mesh: &PolygonMesh, edge: [u32; 2], cuts: u32, slide: f32) -> MeshResult {
    check_mesh(mesh)?;
    check_edges(mesh, &[edge])?;
    check_count("cuts", cuts, MAX_CUTS)?;
    if !slide.is_finite() || !(-1.0..=1.0).contains(&slide) {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "`slide` precisa ficar entre -1 e 1",
            "Use 0 para cortar no meio.",
        ));
    }
    edit(mesh, "corte em laço", "Escolha outra aresta.", |working| {
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
                    legacy::loop_cut::LoopCutError::TwistedRing => (
                        "MESH_LOOP_TWISTED",
                        "o anel se cruza ou está torcido",
                        "Escolha outra aresta.",
                    ),
                    _ => (
                        "MESH_LOOP_FAILED",
                        "não foi possível achar um anel a partir dessa aresta",
                        "Escolha uma aresta entre dois quadriláteros.",
                    ),
                };
                MeshEditError::new(code, message, suggestion)
            })
    })
}

/// Rounds or chamfers the edges: `width` meters, `segments` 1 (chamfer) to 16 (round).
/// All edges or none: when one cannot be beveled, the mesh stays as it was.
pub fn bevel(mesh: &PolygonMesh, edges: &[[u32; 2]], width: f32, segments: u32) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = check_edges(mesh, edges)?;
    check_distance("width", width, false)?;
    check_count("segments", segments, MAX_BEVEL_SEGMENTS)?;
    if width < 0.0 {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "o bevel precisa de uma largura positiva",
            "Use um número maior que zero.",
        ));
    }
    edit(mesh, "bevel", "Use uma largura menor.", |working| {
        let report = working.bevel_edges(&chosen, width, segments, true);
        if !report.unsupported_edges.is_empty() {
            let list: Vec<String> = report
                .unsupported_edges
                .iter()
                .map(|(first, second)| format!("{first}–{second}"))
                .collect();
            return Err(MeshEditError::new(
                "MESH_BEVEL_UNSUPPORTED",
                &format!("o bevel não funciona nestas arestas: {}", list.join(", ")),
                "O bevel precisa de arestas entre duas faces; tire as outras da seleção.",
            ));
        }
        Ok(())
    })
}

/// Splits each triangle or quad into `(cuts + 1)²` smaller ones; neighbors stay joined.
pub fn subdivide(mesh: &PolygonMesh, faces: &[u32], cuts: u32) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = check_faces(mesh, faces)?;
    check_count("cuts", cuts, MAX_CUTS)?;
    if chosen
        .iter()
        .any(|face| !matches!(mesh.faces[*face].len(), 3 | 4))
    {
        return Err(MeshEditError::new(
            "MESH_SUBDIVIDE_NGON",
            "só triângulos e quadriláteros podem ser subdivididos",
            "Use o corte em faca ou o inset em polígonos de mais de 4 lados.",
        ));
    }
    let mut result = edit(
        mesh,
        "subdividir",
        "Escolha triângulos ou quadriláteros.",
        |working| {
            select_faces(working, &chosen);
            working.subdivide_selected_cuts(cuts);
            Ok(())
        },
    )?;
    join_t_junctions(&mut result, mesh);
    result.validate().map_err(|error| invalid_mesh(&error.0))?;
    Ok(result)
}

/// Removes the edges and merges the two faces on each side into one polygon.
pub fn dissolve_edges(mesh: &PolygonMesh, edges: &[[u32; 2]]) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = check_edges(mesh, edges)?;
    edit(
        mesh,
        "dissolver",
        "Escolha arestas entre duas faces (não da borda).",
        |working| {
            select_faces(working, &[]);
            for vertex in &mut working.verts {
                vertex.selected = false;
            }
            working.selected_edges = chosen.iter().copied().collect();
            working.dissolve_selected();
            Ok(())
        },
    )
}

/// Moves the vertices to their center and joins them into one.
pub fn merge_vertices(mesh: &PolygonMesh, vertices: &[u32]) -> MeshResult {
    check_mesh(mesh)?;
    let chosen: BTreeSet<u32> = vertices.iter().copied().collect();
    if chosen.len() < 2 {
        return Err(MeshEditError::new(
            "MESH_NO_VERTICES",
            "fundir precisa de pelo menos dois vértices",
            "Escolha dois ou mais vértices.",
        ));
    }
    if let Some(unknown) = chosen
        .iter()
        .find(|vertex| **vertex as usize >= mesh.positions.len())
    {
        return Err(MeshEditError::new(
            "MESH_UNKNOWN_VERTEX",
            &format!("o vértice {unknown} não existe"),
            "Escolha os vértices de novo: a malha mudou.",
        ));
    }
    edit(
        mesh,
        "fundir vértices",
        "Escolha outros vértices.",
        |working| {
            for face in &mut working.faces {
                face.selected = false;
            }
            for (number, vertex) in working.verts.iter_mut().enumerate() {
                vertex.selected = chosen.contains(&(number as u32));
            }
            working.merge_center();
            Ok(())
        },
    )
}

/// Joins vertices closer than `distance` (cleans up imported or booleaned meshes). Returns the
/// mesh and how many vertices were removed.
pub fn merge_by_distance(
    mesh: &PolygonMesh,
    distance: f32,
) -> Result<(PolygonMesh, usize), MeshEditError> {
    check_mesh(mesh)?;
    check_distance("distance", distance, false)?;
    if distance < 0.0 {
        return Err(MeshEditError::new(
            "MESH_BAD_PARAMETER",
            "a distância precisa ser positiva",
            "Use, por exemplo, 0,001.",
        ));
    }
    let result = edit(
        mesh,
        "fundir por distância",
        "Nenhum vértice está tão perto de outro; aumente a distância.",
        |working| {
            working.weld(distance);
            Ok(())
        },
    )?;
    let removed = mesh.positions.len().saturating_sub(result.positions.len());
    Ok((result, removed))
}

/// Deletes the faces; vertices no face uses any more go too. The mesh is left open.
pub fn delete_faces(mesh: &PolygonMesh, faces: &[u32]) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = check_faces(mesh, faces)?;
    let keep: Vec<Vec<u32>> = mesh
        .faces
        .iter()
        .enumerate()
        .filter(|(number, _)| chosen.binary_search(number).is_err())
        .map(|(_, face)| face.clone())
        .collect();
    let mut working = to_legacy(&PolygonMesh {
        positions: mesh.positions.clone(),
        faces: keep,
    });
    working.selected_edges.clear();
    let result = from_legacy(&working)?;
    if result.faces.is_empty() {
        return Err(MeshEditError::new(
            "MESH_EMPTY_RESULT",
            "apagar essas faces não deixaria nenhuma",
            "Para apagar o objeto, use Apagar.",
        ));
    }
    Ok(result)
}

/// Turns the faces inside out (their front becomes the back). No faces means all of them.
pub fn flip_faces(mesh: &PolygonMesh, faces: &[u32]) -> MeshResult {
    check_mesh(mesh)?;
    let chosen = if faces.is_empty() {
        (0..mesh.faces.len()).collect()
    } else {
        check_faces(mesh, faces)?
    };
    let mut result = mesh.clone();
    for face in chosen {
        result.faces[face].reverse();
    }
    Ok(result)
}

/// Cuts one face in two along the straight line between two points on its edges.
pub fn knife(mesh: &PolygonMesh, start: EdgePoint, end: EdgePoint) -> MeshResult {
    check_mesh(mesh)?;
    check_edges(mesh, &[start.edge, end.edge])?;
    for point in [start, end] {
        if !point.fraction.is_finite() || !(0.0..=1.0).contains(&point.fraction) {
            return Err(MeshEditError::new(
                "MESH_BAD_PARAMETER",
                "o ponto da faca precisa ficar sobre a aresta (fração de 0 a 1)",
                "Clique sobre uma aresta.",
            ));
        }
    }
    let on_edge = |point: EdgePoint| {
        let first = mesh.positions[point.edge[0] as usize];
        let second = mesh.positions[point.edge[1] as usize];
        let position = first + (second - first) * point.fraction;
        legacy::knife::EdgePoint {
            edge: (point.edge[0], point.edge[1]),
            position: glam::Vec3::new(position.x, position.y, position.z),
        }
    };
    let cut = legacy::knife::cut_face(&to_legacy(mesh), on_edge(start), on_edge(end)).map_err(
        |reason| {
            MeshEditError::new(
                "MESH_KNIFE_FAILED",
                &format!("o corte em faca falhou: {reason}"),
                "Os dois pontos precisam estar em arestas diferentes da mesma face.",
            )
        },
    )?;
    let result = from_legacy(&cut)?;
    if result == *mesh {
        return Err(unchanged(
            "faca",
            "Escolha dois pontos em arestas diferentes da mesma face.",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
