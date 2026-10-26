//! Editable polygon meshes as plain data: vertex positions and faces that list vertex numbers.
//!
//! This is the value a `Mesh` row stores (DR-22, option A: the mesh itself is saved, not the
//! operations that made it). It copies by value, validates like any other hostile input, and
//! knows just enough geometry to be drawn: face normals and triangles. The editing operations
//! (booleans, inset, loop cut, bevel…) live in `petunia_mesh`.
//!
//! Text form, one line per vertex and per face so a scene file diffs well:
//! `{"positions": ["0 0 0", "1 0 0", …], "faces": ["0 1 2 3", …]}`.

use crate::Vec3;

/// Hard ceilings for one mesh. Files, scripts and network messages are hostile input.
pub const MAX_MESH_VERTICES: usize = 65_536;
pub const MAX_MESH_FACES: usize = 65_536;
/// Corners of one face (an n-gon).
pub const MAX_FACE_CORNERS: usize = 256;
/// Corners of all faces together.
pub const MAX_MESH_CORNERS: usize = 262_144;

/// A polygon mesh. Faces go around counter-clockwise seen from outside (their front side).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PolygonMesh {
    pub positions: Vec<Vec3>,
    pub faces: Vec<Vec<u32>>,
}

/// Why a mesh is not acceptable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolygonError(pub String);

impl std::fmt::Display for PolygonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PolygonError {}

fn error<T>(message: String) -> Result<T, PolygonError> {
    Err(PolygonError(message))
}

impl PolygonMesh {
    /// Checks every rule: finite positions, limits, faces of at least three different vertices
    /// that exist. A mesh that passes can be drawn and edited without panicking.
    pub fn validate(&self) -> Result<(), PolygonError> {
        if self.positions.len() > MAX_MESH_VERTICES {
            return error(format!(
                "a malha tem {} vértices; o máximo é {MAX_MESH_VERTICES}",
                self.positions.len()
            ));
        }
        if self.faces.len() > MAX_MESH_FACES {
            return error(format!(
                "a malha tem {} faces; o máximo é {MAX_MESH_FACES}",
                self.faces.len()
            ));
        }
        if let Some(vertex) = self.positions.iter().position(|point| !point.is_finite()) {
            return error(format!("o vértice {vertex} não tem coordenadas finitas"));
        }
        let mut corners = 0usize;
        for (face_number, face) in self.faces.iter().enumerate() {
            if face.len() < 3 {
                return error(format!("a face {face_number} tem menos de 3 cantos"));
            }
            if face.len() > MAX_FACE_CORNERS {
                return error(format!(
                    "a face {face_number} tem {} cantos; o máximo é {MAX_FACE_CORNERS}",
                    face.len()
                ));
            }
            corners += face.len();
            if corners > MAX_MESH_CORNERS {
                return error(format!("as faces somam mais de {MAX_MESH_CORNERS} cantos"));
            }
            for (corner, vertex) in face.iter().enumerate() {
                if *vertex as usize >= self.positions.len() {
                    return error(format!(
                        "a face {face_number} usa o vértice {vertex}, que não existe"
                    ));
                }
                if face.iter().take(corner).any(|earlier| earlier == vertex) {
                    return error(format!(
                        "a face {face_number} passa duas vezes pelo vértice {vertex}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn position(&self, vertex: u32) -> Vec3 {
        self.positions
            .get(vertex as usize)
            .copied()
            .unwrap_or(Vec3::ZERO)
    }

    /// Normal of a face (Newell's method: works for concave and slightly bent polygons).
    /// Zero for a degenerate face.
    pub fn face_normal(&self, face: &[u32]) -> Vec3 {
        let mut normal = Vec3::ZERO;
        for (corner, vertex) in face.iter().enumerate() {
            let current = self.position(*vertex);
            let next = self.position(face.get((corner + 1) % face.len()).copied().unwrap_or(0));
            normal = normal
                + Vec3::new(
                    (current.y - next.y) * (current.z + next.z),
                    (current.z - next.z) * (current.x + next.x),
                    (current.x - next.x) * (current.y + next.y),
                );
        }
        normal.normalized_or_zero()
    }

    /// Lowest and highest corner, or `None` for a mesh without vertices.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let first = *self.positions.first()?;
        Some(
            self.positions
                .iter()
                .fold((first, first), |(low, high), point| {
                    (
                        Vec3::new(low.x.min(point.x), low.y.min(point.y), low.z.min(point.z)),
                        Vec3::new(
                            high.x.max(point.x),
                            high.y.max(point.y),
                            high.z.max(point.z),
                        ),
                    )
                }),
        )
    }

    /// Triangles of every face as vertex numbers, front side counter-clockwise. Convex faces
    /// become fans; concave ones are cut into ears in the face's own plane.
    pub fn triangles(&self) -> Vec<[u32; 3]> {
        let mut triangles = Vec::new();
        for face in &self.faces {
            self.triangulate_face(face, &mut triangles);
        }
        triangles
    }

    /// Adds the triangles of one face (nothing for a degenerate face).
    pub fn triangulate_face(&self, face: &[u32], triangles: &mut Vec<[u32; 3]>) {
        let normal = self.face_normal(face);
        if face.len() < 3 || normal.length_squared() == 0.0 {
            return;
        }
        if let [first, second, third] = face {
            triangles.push([*first, *second, *third]);
            return;
        }
        // Project onto the plane of the face, seen from its front.
        let tangent = if normal.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let axis_u = tangent.cross(normal).normalized_or_zero();
        let axis_v = normal.cross(axis_u);
        let flat: Vec<[f32; 2]> = face
            .iter()
            .map(|vertex| {
                let point = self.position(*vertex);
                [point.dot(axis_u), point.dot(axis_v)]
            })
            .collect();
        let start = triangles.len();
        if !cut_ears(&flat, face, triangles) {
            // Self-intersecting outline: a fan still covers it, and nothing panics.
            triangles.truncate(start);
            for corner in 1..face.len() - 1 {
                if let (Some(first), Some(second), Some(third)) =
                    (face.first(), face.get(corner), face.get(corner + 1))
                {
                    triangles.push([*first, *second, *third]);
                }
            }
        }
    }

    /// A number that changes when the mesh changes (positions bit for bit, then faces). Renderers
    /// key their GPU buffers by it.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(self.positions.len() as u64);
        for point in &self.positions {
            mix(u64::from(point.x.to_bits()));
            mix(u64::from(point.y.to_bits()));
            mix(u64::from(point.z.to_bits()));
        }
        mix(self.faces.len() as u64);
        for face in &self.faces {
            mix(face.len() as u64);
            for vertex in face {
                mix(u64::from(*vertex));
            }
        }
        hash
    }

    /// The text form (see the module notes).
    pub fn to_text_json(&self) -> serde_json::Value {
        let positions: Vec<serde_json::Value> = self
            .positions
            .iter()
            .map(|point| serde_json::Value::String(format!("{} {} {}", point.x, point.y, point.z)))
            .collect();
        let faces: Vec<serde_json::Value> = self
            .faces
            .iter()
            .map(|face| {
                let numbers: Vec<String> = face.iter().map(u32::to_string).collect();
                serde_json::Value::String(numbers.join(" "))
            })
            .collect();
        serde_json::json!({ "positions": positions, "faces": faces })
    }

    /// Reads the text form, checking the sizes before allocating, then validates the mesh.
    pub fn from_text_json(json: &serde_json::Value) -> Result<Self, PolygonError> {
        let Some(object) = json.as_object() else {
            return error("esperava {\"positions\": [...], \"faces\": [...]}".to_string());
        };
        if let Some(key) = object
            .keys()
            .find(|key| key.as_str() != "positions" && key.as_str() != "faces")
        {
            return error(format!("campo desconhecido `{key}` na malha"));
        }
        let lines = |key: &str, limit: usize| -> Result<Vec<&str>, PolygonError> {
            let Some(items) = object.get(key).and_then(serde_json::Value::as_array) else {
                return error(format!("a malha precisa da lista `{key}`"));
            };
            if items.len() > limit {
                return error(format!(
                    "`{key}` tem {} itens; o máximo é {limit}",
                    items.len()
                ));
            }
            items
                .iter()
                .enumerate()
                .map(|(line, item)| {
                    item.as_str().ok_or_else(|| {
                        PolygonError(format!("`{key}` item {line}: esperava um texto"))
                    })
                })
                .collect()
        };
        let mut mesh = Self::default();
        for (line, text) in lines("positions", MAX_MESH_VERTICES)?
            .into_iter()
            .enumerate()
        {
            let numbers: Vec<&str> = text.split_whitespace().collect();
            let [x, y, z] = numbers.as_slice() else {
                return error(format!("vértice {line}: esperava três números \"x y z\""));
            };
            let parse = |word: &str| {
                word.parse::<f32>()
                    .map_err(|_| PolygonError(format!("vértice {line}: `{word}` não é um número")))
            };
            mesh.positions
                .push(Vec3::new(parse(x)?, parse(y)?, parse(z)?));
        }
        for (line, text) in lines("faces", MAX_MESH_FACES)?.into_iter().enumerate() {
            let mut face = Vec::new();
            for word in text.split_whitespace() {
                if face.len() >= MAX_FACE_CORNERS {
                    return error(format!(
                        "a face {line} tem mais de {MAX_FACE_CORNERS} cantos"
                    ));
                }
                face.push(word.parse::<u32>().map_err(|_| {
                    PolygonError(format!("face {line}: `{word}` não é um número de vértice"))
                })?);
            }
            mesh.faces.push(face);
        }
        mesh.validate()?;
        Ok(mesh)
    }
}

/// Ear clipping of a simple polygon given in its plane (counter-clockwise or not). Writes
/// triangles with the original winding. `false` when the outline is not simple.
fn cut_ears(flat: &[[f32; 2]], face: &[u32], triangles: &mut Vec<[u32; 3]>) -> bool {
    let point = |corner: usize| flat.get(corner).copied().unwrap_or([0.0, 0.0]);
    let vertex = |corner: usize| face.get(corner).copied().unwrap_or(0);
    let doubled_area: f32 = (0..flat.len())
        .map(|corner| {
            let [ax, ay] = point(corner);
            let [bx, by] = point((corner + 1) % flat.len());
            ax * by - bx * ay
        })
        .sum();
    if doubled_area.abs() <= f32::EPSILON {
        return false;
    }
    let orientation = doubled_area.signum();
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * orientation
    };
    let mut remaining: Vec<usize> = (0..flat.len()).collect();
    // Each pass removes one ear; a simple polygon always has one, so the guard only stops a
    // self-intersecting outline.
    let mut guard = flat.len() * flat.len();
    while remaining.len() > 3 {
        if guard == 0 {
            return false;
        }
        guard -= 1;
        let count = remaining.len();
        let around = |position: usize| {
            let at = |offset: usize| remaining.get(offset % count).copied().unwrap_or(0);
            (at(position + count - 1), at(position), at(position + 1))
        };
        let ear = (0..count).find(|&position| {
            let (previous, current, next) = around(position);
            let (a, b, c) = (point(previous), point(current), point(next));
            if cross(a, b, c) <= 0.0 {
                return false;
            }
            remaining.iter().all(|&other| {
                if other == previous || other == current || other == next {
                    return true;
                }
                let inside = point(other);
                !(cross(a, b, inside) >= 0.0
                    && cross(b, c, inside) >= 0.0
                    && cross(c, a, inside) >= 0.0)
            })
        });
        let Some(position) = ear else {
            return false;
        };
        let (previous, current, next) = around(position);
        triangles.push([vertex(previous), vertex(current), vertex(next)]);
        remaining.remove(position);
    }
    if let [first, second, third] = remaining.as_slice() {
        triangles.push([vertex(*first), vertex(*second), vertex(*third)]);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> PolygonMesh {
        PolygonMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, -1.0),
            ],
            faces: vec![vec![0, 1, 2, 3]],
        }
    }

    #[test]
    fn text_form_round_trips_and_keeps_one_line_per_element() {
        let mesh = square();
        let json = mesh.to_text_json();
        assert_eq!(json["positions"][1], "1 0 0");
        assert_eq!(json["faces"][0], "0 1 2 3");
        assert_eq!(PolygonMesh::from_text_json(&json), Ok(mesh));
    }

    #[test]
    fn hostile_meshes_are_refused_with_a_reason() {
        let cases = [
            serde_json::json!({ "positions": ["0 0 0"], "faces": ["0 1 2"] }),
            serde_json::json!({ "positions": ["0 0 NaN", "1 0 0", "0 1 0"], "faces": ["0 1 2"] }),
            serde_json::json!({ "positions": ["0 0 0", "1 0 0", "0 1 0"], "faces": ["0 1"] }),
            serde_json::json!({ "positions": ["0 0 0", "1 0 0", "0 1 0"], "faces": ["0 1 1"] }),
            serde_json::json!({ "positions": ["0 0"], "faces": [] }),
            serde_json::json!({ "positions": [], "faces": [], "extra": 1 }),
            serde_json::json!({ "positions": [1], "faces": [] }),
            serde_json::json!({ "positions": ["0 0 0"], "faces": ["-1 0 0"] }),
            serde_json::json!([]),
        ];
        for case in cases {
            assert!(PolygonMesh::from_text_json(&case).is_err(), "{case}");
        }
        let too_many: Vec<String> = (0..=MAX_MESH_VERTICES)
            .map(|_| "0 0 0".to_string())
            .collect();
        assert!(
            PolygonMesh::from_text_json(&serde_json::json!({ "positions": too_many, "faces": [] }))
                .is_err()
        );
    }

    #[test]
    fn concave_faces_become_triangles_that_cover_exactly_the_polygon() {
        // An L-shaped face (area 3) in the XZ plane, facing up.
        let mesh = PolygonMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -2.0),
                Vec3::new(0.0, 0.0, -2.0),
            ],
            faces: vec![vec![0, 1, 2, 3, 4, 5]],
        };
        assert!(mesh.face_normal(&mesh.faces[0]).y > 0.99);
        let triangles = mesh.triangles();
        assert_eq!(triangles.len(), 4);
        let mut area = 0.0;
        for [a, b, c] in triangles {
            let (a, b, c) = (mesh.position(a), mesh.position(b), mesh.position(c));
            let normal = (b - a).cross(c - a);
            assert!(normal.y > 0.0, "every triangle keeps the face's winding");
            area += normal.length() * 0.5;
        }
        assert!((area - 3.0).abs() < 1.0e-5, "{area}");
    }

    #[test]
    fn the_fingerprint_follows_every_change() {
        let mesh = square();
        let mut moved = mesh.clone();
        moved.positions[2].x += 1.0e-6;
        let mut rewired = mesh.clone();
        rewired.faces[0].reverse();
        assert_ne!(mesh.fingerprint(), moved.fingerprint());
        assert_ne!(mesh.fingerprint(), rewired.fingerprint());
        assert_eq!(mesh.fingerprint(), square().fingerprint());
    }
}
