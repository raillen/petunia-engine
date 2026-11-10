//! Petunia3D — domínio de malha low-poly.
//!
//! Malha própria: vértices + faces (tris/quads) + UV por vértice-de-face +
//! arestas selecionáveis. Operações retornam `Result` em input externo e
//! mantêm o invariante `face.uv.len() == face.verts.len()`.

use std::collections::{HashMap, HashSet};

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub mod validation;

pub use validation::{
    MeshValidationError, RepairPolicy, RepairReport, repair_external_mesh, validate_invariants,
};

/// Vértice expandido p/ render/export: (posição, normal, cor, uv).
pub type Tri = ([f32; 3], [f32; 3], [f32; 3], [f32; 2]);

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub selected: bool,
}

impl Vertex {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            pos: [x, y, z],
            color: [0.75, 0.75, 0.78],
            selected: false,
        }
    }
    pub fn vec(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Face {
    /// 3 (tri) ou 4 (quad) índices.
    pub verts: Vec<u32>,
    /// UV por vértice-de-face; invariante: `uv.len() == verts.len()`.
    pub uv: Vec<[f32; 2]>,
    pub selected: bool,
    #[serde(default)]
    pub material_slot: Option<usize>,
}

impl Face {
    pub fn new(verts: Vec<u32>) -> Self {
        let n = verts.len();
        Self {
            verts,
            uv: vec![[0.0, 0.0]; n],
            selected: false,
            material_slot: None,
        }
    }
    pub fn with_uv(verts: Vec<u32>, uv: Vec<[f32; 2]>) -> Self {
        debug_assert_eq!(verts.len(), uv.len());
        Self {
            verts,
            uv,
            selected: false,
            material_slot: None,
        }
    }
    fn fix_uv(&mut self) {
        if self.uv.len() != self.verts.len() {
            self.uv = vec![[0.0, 0.0]; self.verts.len()];
        }
    }

    /// Constrói uma face a partir de corners (vértice + UV atomicamente ligados).
    pub fn from_corners(corners: Vec<FaceCorner>) -> Self {
        let verts = corners.iter().map(|c| c.vertex).collect();
        let uv = corners.iter().map(|c| c.texture_coordinates).collect();
        Self::with_uv(verts, uv)
    }

    /// Corner estrutural completa (vértice + UV) no índice informado.
    ///
    /// Une o que hoje vive em dois arrays paralelos, eliminando a possibilidade
    /// de dessincronia entre `verts[i]` e `uv[i]`.
    pub fn corner(&self, index: usize) -> Option<FaceCorner> {
        let vertex = *self.verts.get(index)?;
        let texture_coordinates = *self.uv.get(index)?;
        Some(FaceCorner {
            vertex,
            texture_coordinates,
        })
    }

    /// Versão mutável do corner: permite mover UV e vértice juntos.
    pub fn corner_mut(&mut self, index: usize) -> Option<(&mut u32, &mut [f32; 2])> {
        let vertex = self.verts.get_mut(index)?;
        let uv = self.uv.get_mut(index)?;
        Some((vertex, uv))
    }

    /// Itera todas as corners completas da face.
    pub fn corners(&self) -> impl Iterator<Item = FaceCorner> + '_ {
        self.verts
            .iter()
            .zip(self.uv.iter())
            .map(|(&vertex, &texture_coordinates)| FaceCorner {
                vertex,
                texture_coordinates,
            })
    }

    /// Índices de vértice da face (helper de transição).
    pub fn vertex_indices(&self) -> Vec<u32> {
        self.verts.clone()
    }

    /// Coordenadas de textura da face, na mesma ordem dos vértices (helper de transição).
    pub fn texture_coordinates(&self) -> Vec<[f32; 2]> {
        self.uv.clone()
    }

    /// Inverte a ordem das corners (winding), preservando o vínculo vértice↔UV.
    pub fn reverse_winding(&mut self) {
        self.verts.reverse();
        self.uv.reverse();
    }
}

/// Corner estrutural autoral: um vértice e sua coordenada UV, atomicamente ligados
/// (website/docs/03-geometry/uv-face-corners.md).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceCorner {
    /// Índice do vértice na malha.
    pub vertex: u32,
    /// Coordenada UV pertencente exclusivamente a este corner da face.
    pub texture_coordinates: [f32; 2],
}

impl FaceCorner {
    pub fn new(vertex: u32, texture_coordinates: [f32; 2]) -> Self {
        Self {
            vertex,
            texture_coordinates,
        }
    }
}

/// Malha. `selected_edges` guarda arestas como `(min, max)`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub verts: Vec<Vertex>,
    pub faces: Vec<Face>,
    pub selected_edges: HashSet<(u32, u32)>,
    /// UV0 seams: undirected edges that split charts (ch. 15).
    #[serde(default)]
    pub uv_seams: HashSet<(u32, u32)>,
    /// Vértices UV fixados (pinned): pares (face_idx, corner_idx) estáticos em projeções/unwraps.
    #[serde(default)]
    pub uv_pinned: HashSet<(usize, usize)>,
    /// Arestas Sharp (`(min, max)`): decisão autoral que delimita smoothing islands.
    /// Independente de UV seam e de feature edge (website/docs/03-geometry/normals-shading-sharp-edges.md).
    #[serde(default)]
    pub sharp_edges: HashSet<(u32, u32)>,
}

/// Índice tipado de vértice na malha (website/docs/03-geometry/architecture-decision.md §5).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct VertexIndex(pub u32);

impl From<u32> for VertexIndex {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl From<VertexIndex> for u32 {
    fn from(val: VertexIndex) -> Self {
        val.0
    }
}

impl From<VertexIndex> for usize {
    fn from(val: VertexIndex) -> Self {
        val.0 as usize
    }
}

/// Índice tipado de face na malha (website/docs/03-geometry/architecture-decision.md §5).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct FaceIndex(pub usize);

impl From<usize> for FaceIndex {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<FaceIndex> for usize {
    fn from(val: FaceIndex) -> Self {
        val.0
    }
}

/// Um canto de face (FaceCorner): a face e a posição do canto dentro dela
/// (website/docs/07-uv/architecture-decision.md, "Pins" e "UV Selection").
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FaceCornerIndex {
    pub face: usize,
    pub corner: usize,
}

impl FaceCornerIndex {
    pub const fn new(face: usize, corner: usize) -> Self {
        Self { face, corner }
    }
}

impl From<(usize, usize)> for FaceCornerIndex {
    fn from((face, corner): (usize, usize)) -> Self {
        Self { face, corner }
    }
}

impl From<FaceCornerIndex> for (usize, usize) {
    fn from(val: FaceCornerIndex) -> Self {
        (val.face, val.corner)
    }
}

/// Chave canônica não-direcionada de aresta `(min, max)` (website/docs/03-geometry/architecture-decision.md §5).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeKey(pub u32, pub u32);

impl EdgeKey {
    pub fn new(a: u32, b: u32) -> Self {
        Self(a.min(b), a.max(b))
    }

    pub fn to_pair(self) -> (u32, u32) {
        (self.0, self.1)
    }
}

impl From<(u32, u32)> for EdgeKey {
    fn from((a, b): (u32, u32)) -> Self {
        Self::new(a, b)
    }
}

impl From<EdgeKey> for (u32, u32) {
    fn from(key: EdgeKey) -> Self {
        key.to_pair()
    }
}

pub fn edge_key(a: u32, b: u32) -> (u32, u32) {
    EdgeKey::new(a, b).to_pair()
}

impl Mesh {
    pub fn push_face(&mut self, face: Face) {
        let mut f = face;
        f.fix_uv();
        self.faces.push(f);
    }

    /// Verificação estrita de invariantes sem mutação de dados.
    ///
    /// O antigo `Mesh::validate()` mutante foi removido (decisão 5 de
    /// website/docs/03-geometry/validation-repair-cleanup.md): reparo só existe
    /// na fronteira externa, explícito e com relatório — use
    /// `repair_external_mesh(&mut mesh, policy)`.
    pub fn validate_invariants(&self) -> Result<(), MeshValidationError> {
        validate_invariants(self)
    }

    pub fn vert_count(&self) -> usize {
        self.verts.len()
    }
    #[allow(dead_code)]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
    /// tris equivalentes (quads = 2 tris).
    pub fn tri_count(&self) -> usize {
        self.faces
            .iter()
            .map(|f| f.verts.len().saturating_sub(2))
            .sum()
    }

    /// Arestas únicas da malha.
    pub fn edges_unique(&self) -> Vec<(u32, u32)> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for f in &self.faces {
            let m = f.verts.len();
            for k in 0..m {
                let key = edge_key(f.verts[k], f.verts[(k + 1) % m]);
                if seen.insert(key) {
                    out.push(key);
                }
            }
        }
        out
    }

    /// Faces adjacentes a uma aresta (índices).
    pub fn edge_faces(&self, a: u32, b: u32) -> Vec<usize> {
        self.faces
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                let m = f.verts.len();
                (0..m).any(|k| {
                    let x = f.verts[k];
                    let y = f.verts[(k + 1) % m];
                    (x == a && y == b) || (x == b && y == a)
                })
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn face_centroid(&self, fi: usize) -> Vec3 {
        let f = &self.faces[fi];
        let mut c = Vec3::ZERO;
        for &vi in &f.verts {
            c += self.verts[vi as usize].vec();
        }
        c / f.verts.len().max(1) as f32
    }

    /// Raycast against the render triangulation, including concave n-gons.
    /// Retorna `(face, t, ponto)` mais próximo, se houver.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3) -> Option<(usize, f32, Vec3)> {
        let mut best: Option<(usize, f32)> = None;
        for (fi, f) in self.faces.iter().enumerate() {
            for corners in self.face_triangle_corners(fi) {
                let [a, b, c] = corners.map(|corner| f.verts[corner]);
                if let Some(t) = triangulate::ray_tri(
                    origin,
                    dir,
                    self.verts[a as usize].vec(),
                    self.verts[b as usize].vec(),
                    self.verts[c as usize].vec(),
                ) && t > 1e-6
                    && best.map(|(_, bt)| t < bt).unwrap_or(true)
                {
                    best = Some((fi, t));
                }
            }
        }
        best.map(|(fi, t)| (fi, t, origin + dir * t))
    }

    pub fn face_normal(&self, fi: usize) -> Vec3 {
        triangulate::face_normal_of(&self.verts, &self.faces[fi].verts)
    }

    /// Calcula o centroide médio dos vértices da malha.
    pub fn center(&self) -> Vec3 {
        if self.verts.is_empty() {
            return Vec3::ZERO;
        }
        let sum: Vec3 = self.verts.iter().map(|v| v.vec()).sum();
        sum / (self.verts.len() as f32)
    }
}

impl Mesh {
    /// Fuses vertices closer than `eps` (brush triangles share a corner per normal).
    #[allow(clippy::needless_range_loop)]
    pub fn weld(&mut self, eps: f32) {
        let n = self.verts.len();
        let mut rep: Vec<u32> = (0..n as u32).collect();
        for i in 0..n {
            if rep[i] != i as u32 {
                continue;
            }
            for j in (i + 1)..n {
                if rep[j] != j as u32 {
                    continue;
                }
                let distance = (self.verts[i].vec() - self.verts[j].vec()).length();
                if distance <= eps {
                    rep[j] = i as u32;
                }
            }
        }
        let mut new_id = vec![u32::MAX; n];
        let mut compacted = Vec::new();
        for i in 0..n {
            if rep[i] == i as u32 {
                new_id[i] = compacted.len() as u32;
                compacted.push(self.verts[i].clone());
            }
        }
        for i in 0..n {
            if rep[i] != i as u32 {
                new_id[i] = new_id[rep[i] as usize];
            }
        }
        for face in &mut self.faces {
            for vertex in &mut face.verts {
                *vertex = new_id[*vertex as usize];
            }
        }
        self.faces.retain(|face| {
            let mut unique = face.verts.clone();
            unique.sort_unstable();
            unique.dedup();
            unique.len() >= 3
        });
        self.verts = compacted;
        let edges: Vec<(u32, u32)> = self.selected_edges.iter().copied().collect();
        self.selected_edges.clear();
        for (a, b) in edges {
            let (na, nb) = (new_id[a as usize], new_id[b as usize]);
            if na != u32::MAX && nb != u32::MAX && na != nb {
                self.selected_edges.insert(edge_key(na, nb));
            }
        }
    }

    /// Splits polygons into triangles. A face that cannot be split stays as it is.
    pub fn triangulate(&mut self) {
        let mut out = Vec::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            if face.verts.len() <= 3 {
                out.push(face.clone());
                continue;
            }
            let corners = self.face_triangle_corners(face_index);
            if corners.len() != face.verts.len() - 2 {
                out.push(face.clone());
                continue;
            }
            for triangle in corners {
                let mut split = face.clone();
                split.verts = triangle.map(|corner| face.verts[corner]).to_vec();
                split.uv = triangle
                    .map(|corner| face.uv.get(corner).copied().unwrap_or_default())
                    .to_vec();
                out.push(split);
            }
        }
        self.faces = out;
    }
}

pub mod boolean;
pub mod half_edge;
pub use half_edge::HalfEdgeMesh;
pub mod boolean_cleanup;
mod boolean_partition;
pub mod cleanup;
pub mod primitives;
pub mod topology;
pub mod triangulate;

pub use topology::{DirtyDomains, ElementRemap, TopologyResult};

impl Mesh {
    // ---------------- saída p/ render/export ----------------

    /// Triangles expressed as face-corner indices, preserving UV seams and
    /// winding. Concave polygons must use the same tessellation for drawing
    /// and picking; a fan can otherwise make empty space selectable.
    pub fn face_triangle_corners(&self, face_index: usize) -> Vec<[usize; 3]> {
        let Some(face) = self.faces.get(face_index) else {
            return Vec::new();
        };
        if face.verts.len() < 3
            || face.verts.iter().any(|&v| {
                v as usize >= self.verts.len() || !self.verts[v as usize].vec().is_finite()
            })
        {
            return Vec::new();
        }
        if face.verts.len() == 3 {
            return vec![[0, 1, 2]];
        }
        let normal = self.face_normal(face_index).abs();
        let axis = if normal.x >= normal.y && normal.x >= normal.z {
            0
        } else if normal.y >= normal.z {
            1
        } else {
            2
        };
        let points: Vec<[f32; 2]> = face
            .verts
            .iter()
            .map(|&v| {
                let p = self.verts[v as usize].pos;
                match axis {
                    0 => [p[1], p[2]],
                    1 => [p[2], p[0]],
                    _ => [p[0], p[1]],
                }
            })
            .collect();
        let area = triangulate::polygon_area(&points);
        let convex = (0..points.len()).all(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let c = points[(i + 2) % points.len()];
            ((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])) * area >= 0.0
        });
        if convex && area.abs() > 1.0e-12 {
            return (1..face.verts.len() - 1).map(|i| [0, i, i + 1]).collect();
        }
        let Ok(mut triangles) = triangulate::ear_clip(&points) else {
            return Vec::new();
        };
        if area < 0.0 {
            for tri in &mut triangles {
                tri.swap(1, 2);
            }
        }
        triangles
    }

    /// Identidade estável da conectividade e da triangulação autoral.
    /// Posições só participam quando mudam a decomposição de uma face côncava.
    pub fn topology_fingerprint(&self) -> u64 {
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        let mix = |value: u64, next: u64| {
            let value = value ^ next;
            value.wrapping_mul(0x100_0000_01b3)
        };
        fingerprint = mix(fingerprint, self.verts.len() as u64);
        fingerprint = mix(fingerprint, self.faces.len() as u64);
        for (face_index, face) in self.faces.iter().enumerate() {
            fingerprint = mix(fingerprint, face.verts.len() as u64);
            for &vertex in &face.verts {
                fingerprint = mix(fingerprint, u64::from(vertex));
            }
            let triangles = self.face_triangle_corners(face_index);
            fingerprint = mix(fingerprint, triangles.len() as u64);
            for triangle in triangles {
                for corner in triangle {
                    fingerprint = mix(fingerprint, corner as u64);
                }
            }
        }
        fingerprint
    }

    /// Marca arestas como Sharp (decisão autoral; não é seam nem feature edge).
    /// Ignora self-edges e vértices inexistentes. Devolve quantas arestas mudaram.
    /// (Petunia Engine: copied from Petunia3D `shading.rs`, the rest of that module was not.)
    pub fn mark_sharp(&mut self, edges: impl IntoIterator<Item = (u32, u32)>) -> usize {
        let mut changed = 0;
        for (a, b) in edges {
            if a == b || (a as usize) >= self.verts.len() || (b as usize) >= self.verts.len() {
                continue;
            }
            if self.sharp_edges.insert(edge_key(a, b)) {
                changed += 1;
            }
        }
        changed
    }

    /// (a, b, selecionada) p/ overlay wireframe.
    pub fn to_edges(&self) -> Vec<([f32; 3], [f32; 3], bool)> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for f in &self.faces {
            let m = f.verts.len();
            for k in 0..m {
                let a = f.verts[k];
                let b = f.verts[(k + 1) % m];
                let key = edge_key(a, b);
                if seen.insert(key) {
                    let sel = self.selected_edges.contains(&key);
                    out.push((self.verts[a as usize].pos, self.verts[b as usize].pos, sel));
                }
            }
        }
        out
    }

    /// Arestas classificadas para a aparência por modo (capítulo 05): `(a, b,
    /// selecionada, de feição)`. De feição = borda (uma face), não manifold
    /// ou dobra entre faces vizinhas acima de `crease_degrees`.
    pub fn to_classified_edges(
        &self,
        crease_degrees: f32,
    ) -> Vec<([f32; 3], [f32; 3], bool, bool)> {
        let mut faces_of: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        let mut order = Vec::new();
        for (index, face) in self.faces.iter().enumerate() {
            let m = face.verts.len();
            for k in 0..m {
                let key = edge_key(face.verts[k], face.verts[(k + 1) % m]);
                let entry = faces_of.entry(key).or_default();
                if entry.is_empty() {
                    order.push(key);
                }
                entry.push(index);
            }
        }
        let cos_limit = crease_degrees.to_radians().cos();
        let normals: Vec<Vec3> = (0..self.faces.len())
            .map(|f| self.face_normal(f).normalize_or_zero())
            .collect();
        order
            .into_iter()
            .map(|key| {
                let faces = &faces_of[&key];
                let feature = match faces.as_slice() {
                    [a, b] => normals[*a].dot(normals[*b]) < cos_limit,
                    _ => true,
                };
                (
                    self.verts[key.0 as usize].pos,
                    self.verts[key.1 as usize].pos,
                    self.selected_edges.contains(&key),
                    feature,
                )
            })
            .collect()
    }

    /// Retorna pares de pontos `(p0, p1)` representando as diagonais internas de triangulação
    /// para todos os polígonos com 4 ou mais lados da malha (para exibição em Show Triangulation).
    pub fn triangulation_wireframe(&self) -> Vec<([f32; 3], [f32; 3])> {
        let mut lines = Vec::new();
        for f in &self.faces {
            let m = f.verts.len();
            if m <= 3 {
                continue;
            }
            let idx0 = f.verts[0] as usize;
            if idx0 >= self.verts.len() {
                continue;
            }
            let p0 = self.verts[idx0].pos;
            for i in 2..(m - 1) {
                let idxi = f.verts[i] as usize;
                if idxi < self.verts.len() {
                    let pi = self.verts[idxi].pos;
                    lines.push((p0, pi));
                }
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn classified_edges_mark_borders_and_creases_only() {
        use super::{Face, Mesh, Vertex};
        // Dois quads coplanares (aresta do meio plana) e um terceiro dobrado
        // a 90° sobre a borda direita.
        let mut mesh = Mesh::default();
        for p in [
            [-2.0f32, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [2.0, -1.0, 0.0],
            [2.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [-2.0, 1.0, 0.0],
            [2.0, -1.0, -2.0],
            [2.0, 1.0, -2.0],
        ] {
            mesh.verts.push(Vertex::new(p[0], p[1], p[2]));
        }
        mesh.faces.push(Face::new(vec![0, 1, 4, 5]));
        mesh.faces.push(Face::new(vec![1, 2, 3, 4]));
        mesh.faces.push(Face::new(vec![2, 6, 7, 3]));
        let edges = mesh.to_classified_edges(30.0);
        let feature = |a: [f32; 3], b: [f32; 3]| {
            edges
                .iter()
                .find(|e| (e.0 == a && e.1 == b) || (e.0 == b && e.1 == a))
                .map(|e| e.3)
                .unwrap()
        };
        assert!(!feature([0.0, -1.0, 0.0], [0.0, 1.0, 0.0]), "plana");
        assert!(feature([2.0, -1.0, 0.0], [2.0, 1.0, 0.0]), "dobra de 90°");
        assert!(feature([-2.0, -1.0, 0.0], [-2.0, 1.0, 0.0]), "borda");
        assert_eq!(edges.len(), mesh.edges_unique().len());
    }

    use crate::legacy::{
        EdgeKey, Face, FaceIndex, Mesh, RepairPolicy, VertexIndex, edge_key, triangulate::ear_clip,
        triangulate::ray_tri_hit, validation::repair_external_mesh,
    };
    use glam::Vec3;

    #[test]
    fn cube_has_8_verts_6_faces() {
        let m = Mesh::cube(2.0);
        assert_eq!(m.verts.len(), 8);
        assert_eq!(m.faces.len(), 6);
    }

    #[test]
    fn ray_triangle_hit_reports_stable_barycentric_coordinates() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(2.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 0.0, 2.0);
        let hit = ray_tri_hit(Vec3::new(0.5, 2.0, 0.25), -Vec3::Y, a, b, c).unwrap();
        let reconstructed =
            a * hit.barycentric[0] + b * hit.barycentric[1] + c * hit.barycentric[2];
        assert!((hit.distance - 2.0).abs() < 1.0e-6);
        assert!((hit.barycentric.iter().sum::<f32>() - 1.0).abs() < 1.0e-6);
        assert!(reconstructed.distance(Vec3::new(0.5, 0.0, 0.25)) < 1.0e-6);
        assert!(ray_tri_hit(Vec3::ZERO, Vec3::NAN, a, b, c).is_none());
    }

    #[test]
    fn topology_fingerprint_ignores_attributes_but_tracks_connectivity() {
        let mut mesh = Mesh::plane(2.0);
        let fingerprint = mesh.topology_fingerprint();
        for vertex in &mut mesh.verts {
            vertex.pos[1] += 3.0;
            vertex.color = [0.1, 0.2, 0.3];
        }
        mesh.faces[0].uv.fill([0.25, 0.75]);
        mesh.faces[0].selected = true;
        assert_eq!(mesh.topology_fingerprint(), fingerprint);

        mesh.faces[0].verts.swap(1, 2);
        assert_ne!(mesh.topology_fingerprint(), fingerprint);
    }

    #[test]
    fn ear_clip_concave() {
        // seta côncava: 7 pontos
        let pts = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
            [0.5, 1.0],
        ];
        let tris = ear_clip(&pts).expect("triangula");
        assert_eq!(tris.len(), pts.len() - 2);
    }

    #[test]
    fn ear_clip_rejects_degenerate() {
        assert!(ear_clip(&[[0.0, 0.0], [1.0, 1.0]]).is_err());
    }

    #[test]
    fn validate_repairs_corrupt() {
        // Dados externos corrompidos passam por reparo explícito com relatório.
        let mut m = Mesh::cube(1.0);
        m.faces.push(Face {
            verts: vec![0, 1, 99],
            uv: vec![[0.0, 0.0]],
            selected: false,
            material_slot: None,
        });
        m.selected_edges.insert((0, 500));

        // Validação estrita rejeita sem mutar (dados de authoring inválidos = bug).
        assert!(m.validate_invariants().is_err());

        let report = repair_external_mesh(&mut m, RepairPolicy::TolerantImport);
        assert_eq!(report.faces_dropped, 1);
        assert_eq!(report.selected_edges_dropped, 1);
        assert!(m.faces.iter().all(|f| f.uv.len() == f.verts.len()));
        assert!(
            m.faces
                .iter()
                .all(|f| f.verts.iter().all(|&i| (i as usize) < m.verts.len()))
        );
        assert!(
            m.selected_edges
                .iter()
                .all(|&(a, b)| (a as usize) < m.verts.len() && (b as usize) < m.verts.len())
        );
        // Após o reparo explícito, a malha volta a satisfazer os invariantes.
        assert!(m.validate_invariants().is_ok());
    }

    #[test]
    fn face_corner_binds_vertex_and_uv_atomically() {
        let face = Face::with_uv(vec![0, 1, 2], vec![[0.1, 0.2], [0.3, 0.4], [0.5, 0.6]]);

        let corners: Vec<_> = face.corners().collect();
        assert_eq!(corners.len(), 3);
        assert_eq!(corners[1].vertex, 1);
        assert_eq!(corners[1].texture_coordinates, [0.3, 0.4]);

        // corner_mut permite alterar UV sem tocar nos vértices.
        let mut face = face;
        if let Some((vertex, uv)) = face.corner_mut(2) {
            uv[0] = 0.9;
            assert_eq!(*vertex, 2);
        }
        assert_eq!(face.corner(2).unwrap().texture_coordinates, [0.9, 0.6]);
        assert_eq!(face.verts, vec![0, 1, 2], "vértices intactos");

        // Roundtrip por corners preserva o vínculo.
        let rebuilt = Face::from_corners(face.corners().collect());
        assert_eq!(rebuilt.verts, face.verts);
        assert_eq!(rebuilt.uv, face.uv);
    }

    #[test]
    fn face_corner_reverse_winding_keeps_pairs() {
        let face = Face::with_uv(vec![0, 1, 2], vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);
        let mut reversed = face.clone();
        reversed.reverse_winding();
        assert_eq!(reversed.verts, vec![2, 1, 0]);
        // Cada vértice mantém a UV do seu corner original.
        assert_eq!(reversed.corner(0).unwrap().vertex, 2);
        assert_eq!(reversed.corner(0).unwrap().texture_coordinates, [1.0, 1.0]);
    }

    #[test]
    fn typed_indices_and_edge_key_invariants() {
        let v_idx = VertexIndex::from(42u32);
        assert_eq!(v_idx.0, 42);
        assert_eq!(u32::from(v_idx), 42);
        assert_eq!(usize::from(v_idx), 42);

        let f_idx = FaceIndex::from(10usize);
        assert_eq!(f_idx.0, 10);
        assert_eq!(usize::from(f_idx), 10);

        let key1 = EdgeKey::new(10, 5);
        let key2 = EdgeKey::new(5, 10);
        assert_eq!(key1, key2);
        assert_eq!(key1.to_pair(), (5, 10));
        assert_eq!(<(u32, u32)>::from(key1), (5, 10));
        assert_eq!(edge_key(10, 5), (5, 10));
    }

    #[test]
    fn a_cube_keeps_unselected_vertices() {
        let mesh = Mesh::cube(2.0);
        assert!(mesh.verts.iter().all(|vertex| !vertex.selected));
    }
}
